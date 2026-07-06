//! OVSDB client - manages connection, schema, and IDL.

use serde_json::{Value, json};
use uuid::Uuid;

use rovs_jsonrpc::Connection;
use rovs_transport::{Address, Stream};

use crate::{DbSchema, Error, Idl, IdlState, Result, Transaction};

/// Monitor protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorVersion {
    /// Original monitor (OVSDB_UPDATE)
    V1,
    /// monitor_cond (OVSDB_UPDATE2)
    V2,
    /// monitor_cond_since (OVSDB_UPDATE3)
    V3,
}

/// OVSDB client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Database name to connect to
    pub database: String,
    /// Tables to monitor (None = all tables)
    pub tables: Option<Vec<String>>,
    /// Monitor protocol version to use
    pub monitor_version: MonitorVersion,
    /// Leader-only mode for clustered OVSDB
    pub leader_only: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            database: "Open_vSwitch".to_owned(),
            tables: None,
            monitor_version: MonitorVersion::V1,
            leader_only: false,
        }
    }
}

impl ClientConfig {
    /// Create config for Open_vSwitch database.
    pub fn open_vswitch() -> Self {
        Self::default()
    }

    /// Set the database name.
    pub fn database(mut self, name: impl Into<String>) -> Self {
        self.database = name.into();
        self
    }

    /// Set specific tables to monitor.
    pub fn tables(mut self, tables: Vec<String>) -> Self {
        self.tables = Some(tables);
        self
    }

    /// Set monitor version.
    pub fn monitor_version(mut self, version: MonitorVersion) -> Self {
        self.monitor_version = version;
        self
    }
}

/// OVSDB client.
///
/// Manages the connection to an OVSDB server, fetches the schema,
/// and maintains an in-memory replica via the IDL.
pub struct Client {
    conn: Connection,
    config: ClientConfig,
    idl: Idl,
    monitor_id: Option<Uuid>,
}

impl Client {
    /// Connect to an OVSDB server.
    pub async fn connect(addr: &str) -> Result<Self> {
        Self::connect_with_config(addr, ClientConfig::default()).await
    }

    /// Connect to an OVSDB server with custom configuration.
    pub async fn connect_with_config(addr: &str, config: ClientConfig) -> Result<Self> {
        let address: Address = addr.parse()?;
        let stream = Stream::connect(&address).await?;
        let conn = Connection::new(stream);

        let mut client = Self {
            conn,
            config,
            idl: Idl::new(),
            monitor_id: None,
        };

        // Initialize: fetch schema and start monitoring
        client.initialize().await?;

        Ok(client)
    }

    /// Get the IDL (in-memory database replica).
    ///
    /// The IDL contains all monitored tables and rows. Use it to query
    /// the current state without making RPC calls.
    ///
    /// # Example
    ///
    /// ```ignore
    /// for bridge in client.idl().rows("Bridge") {
    ///     println!("Bridge: {}", bridge.get_string("name").unwrap_or("?"));
    /// }
    /// ```
    pub fn idl(&self) -> &Idl {
        &self.idl
    }

    /// Get a mutable reference to the IDL.
    ///
    /// Rarely needed; most operations use the immutable [`idl`](Self::idl).
    pub fn idl_mut(&mut self) -> &mut Idl {
        &mut self.idl
    }

    /// Get the database schema.
    ///
    /// The schema describes all tables and columns in the database.
    /// Returns `None` if the schema hasn't been loaded yet.
    pub fn schema(&self) -> Option<&DbSchema> {
        self.idl.schema()
    }

    /// Check if the client is connected and actively monitoring.
    ///
    /// Returns `true` after successful connection and monitor setup.
    pub fn is_connected(&self) -> bool {
        self.idl.state() == IdlState::Monitoring
    }

    /// Initialize the client: fetch schema and start monitoring.
    async fn initialize(&mut self) -> Result<()> {
        // Step 1: Fetch schema
        self.fetch_schema().await?;

        // Step 2: Start monitoring
        self.start_monitor().await?;

        Ok(())
    }

    /// Fetch the database schema.
    async fn fetch_schema(&mut self) -> Result<()> {
        tracing::debug!("Fetching schema for database: {}", self.config.database);

        let result = self
            .conn
            .transact("get_schema", json!([self.config.database]))
            .await?;

        let schema = DbSchema::from_json(&result)?;
        tracing::info!(
            "Loaded schema: {} v{} ({} tables)",
            schema.name,
            schema.version,
            schema.tables.len()
        );

        self.idl.set_schema(schema);
        Ok(())
    }

    /// Start monitoring the database.
    async fn start_monitor(&mut self) -> Result<()> {
        let schema = self
            .idl
            .schema()
            .ok_or_else(|| Error::Schema("schema not loaded".into()))?;

        // Build monitor request for each table
        let mut monitor_requests = serde_json::Map::new();

        let tables_to_monitor: Vec<&String> = match &self.config.tables {
            Some(tables) => tables.iter().collect(),
            None => schema.tables.keys().collect(),
        };

        for table_name in tables_to_monitor {
            if let Some(table_schema) = schema.tables.get(table_name) {
                // Request all columns for the table
                let columns: Vec<&String> = table_schema.columns.keys().collect();

                let table_monitor = json!({
                    "columns": columns,
                });

                monitor_requests.insert(table_name.clone(), table_monitor);
            }
        }

        let monitor_id = Uuid::new_v4();
        self.monitor_id = Some(monitor_id);

        // Choose monitor method based on version
        let (method, params) = match self.config.monitor_version {
            MonitorVersion::V1 => (
                "monitor",
                json!([
                    self.config.database,
                    monitor_id.to_string(),
                    monitor_requests
                ]),
            ),
            MonitorVersion::V2 => (
                "monitor_cond",
                json!([
                    self.config.database,
                    monitor_id.to_string(),
                    monitor_requests
                ]),
            ),
            MonitorVersion::V3 => (
                "monitor_cond_since",
                json!([
                    self.config.database,
                    monitor_id.to_string(),
                    monitor_requests,
                    "00000000-0000-0000-0000-000000000000" // last_txn_id
                ]),
            ),
        };

        tracing::debug!("Starting monitor with method: {}", method);

        let result = self.conn.transact(method, params).await?;

        // Process initial data
        self.process_monitor_reply(&result)?;
        self.idl.set_monitoring();

        tracing::info!("Monitoring started, {} tables", monitor_requests.len());

        Ok(())
    }

    /// Process the initial monitor reply.
    fn process_monitor_reply(&mut self, reply: &Value) -> Result<()> {
        match self.config.monitor_version {
            MonitorVersion::V1 => {
                // V1: reply is the table update directly
                self.idl.process_update(reply);
            }
            MonitorVersion::V2 | MonitorVersion::V3 => {
                // V2/V3: reply may have different structure
                // For V3, it's [found, last_txn_id, updates]
                if let Some(arr) = reply.as_array() {
                    if arr.len() >= 3 {
                        // V3 format
                        self.idl.process_update(&arr[2]);
                    } else {
                        self.idl.process_update(reply);
                    }
                } else {
                    self.idl.process_update(reply);
                }
            }
        }
        Ok(())
    }

    /// Process any pending update notifications (non-blocking).
    ///
    /// Drains buffered notifications and updates the IDL. Use this in
    /// event loops where you need to check for updates without blocking.
    ///
    /// Returns `true` if any updates were processed.
    ///
    /// For blocking behavior, use [`wait`](Self::wait) instead.
    pub async fn run(&mut self) -> Result<bool> {
        let mut updated = false;

        // Process any buffered notifications
        while let Some(notification) = self.conn.pop_notification() {
            if notification.method == "update"
                || notification.method == "update2"
                || notification.method == "update3"
            {
                if let Some(params) = notification.params.as_array() {
                    // params: [monitor_id, updates]
                    if params.len() >= 2 {
                        self.idl.process_update(&params[1]);
                        updated = true;
                    }
                }
            }
        }

        Ok(updated)
    }

    /// Wait for the next update from the server (blocking).
    ///
    /// Blocks until an OVSDB update notification is received, then updates
    /// the IDL and returns. Use this in a loop to continuously monitor changes.
    ///
    /// # Example
    ///
    /// ```ignore
    /// loop {
    ///     client.wait().await?;
    ///     println!("Update received, seqno: {}", client.idl().change_seqno());
    ///     // Process changes...
    /// }
    /// ```
    pub async fn wait(&mut self) -> Result<()> {
        use rovs_jsonrpc::Message;

        // First, check for buffered notifications from previous operations
        while let Some(notification) = self.conn.pop_notification() {
            if notification.method == "echo" {
                self.send_echo_reply(&notification.params, notification.id.as_ref())
                    .await?;
                continue;
            }
            if let Some(updated) = self.process_notification(&notification) {
                if updated {
                    return Ok(());
                }
            }
        }

        // No buffered updates, wait for new messages
        loop {
            let msg = self.conn.recv_message().await?;

            match msg {
                Message::Request(req) => {
                    if req.method == "echo" {
                        self.send_echo_reply(&req.params, req.id.as_ref()).await?;
                        continue;
                    }
                    if let Some(updated) = self.process_notification(&req) {
                        if updated {
                            return Ok(());
                        }
                    }
                }
                Message::Response(_) => {
                    tracing::warn!("Received unexpected response");
                }
            }
        }
    }

    /// Process a notification from the server.
    ///
    /// Returns `Some(true)` if this was an update notification that was processed,
    /// `Some(false)` if it was handled but not an update, or `None` if unhandled.
    fn process_notification(&mut self, req: &rovs_jsonrpc::Request) -> Option<bool> {
        match req.method.as_str() {
            "update" | "update2" | "update3" => {
                if let Some(params) = req.params.as_array() {
                    if params.len() >= 2 {
                        self.idl.process_update(&params[1]);
                        return Some(true);
                    }
                }
                Some(false)
            }
            "echo" => {
                // Server echo request - handled asynchronously
                tracing::debug!("Received echo request from server");
                Some(false)
            }
            _ => {
                tracing::debug!("Unknown notification: {}", req.method);
                None
            }
        }
    }

    /// Send echo reply to server.
    async fn send_echo_reply(
        &mut self,
        params: &Value,
        id: Option<&rovs_jsonrpc::RpcId>,
    ) -> Result<()> {
        use rovs_jsonrpc::{Message, Response, RpcId};

        let response_id = id
            .cloned()
            .unwrap_or_else(|| RpcId::String("echo".to_owned()));
        let response = Response::success(response_id, params.clone());
        self.conn.send_message(&Message::Response(response)).await?;
        Ok(())
    }

    /// Execute a raw transaction with operations as JSON.
    pub async fn transact(&mut self, operations: Value) -> Result<Value> {
        let params = if let Value::Array(mut arr) = operations {
            arr.insert(0, Value::String(self.config.database.clone()));
            Value::Array(arr)
        } else {
            json!([self.config.database, operations])
        };

        self.conn
            .transact("transact", params)
            .await
            .map_err(Into::into)
    }

    /// Commit a transaction.
    ///
    /// Sends the transaction to the server and processes the result.
    /// On success, the transaction's uuid_map will be populated with
    /// the actual UUIDs for any inserted rows.
    pub async fn commit(&mut self, txn: &mut Transaction) -> Result<bool> {
        if txn.is_empty() {
            return Ok(true);
        }

        // Build and send the transaction
        let params = txn.build();
        let result = self.conn.transact("transact", params).await?;

        // Process the result
        Ok(txn.process_result(&result))
    }

    /// Get the list of databases on the server.
    pub async fn list_dbs(&mut self) -> Result<Vec<String>> {
        let result = self.conn.transact("list_dbs", json!([])).await?;

        let dbs = result
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(dbs)
    }

    /// Cancel the current monitor and stop receiving updates.
    ///
    /// After cancellation, the IDL will no longer receive updates. You can
    /// start a new monitor by reconnecting.
    pub async fn cancel_monitor(&mut self) -> Result<()> {
        if let Some(monitor_id) = self.monitor_id.take() {
            self.conn
                .transact("monitor_cancel", json!([monitor_id.to_string()]))
                .await?;
        }
        Ok(())
    }
}
