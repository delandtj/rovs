//! OVSDB IDL (Interface Definition Language) client.
//!
//! The IDL maintains an in-memory replica of the database and handles
//! synchronization with the OVSDB server.

use std::collections::HashMap;

use uuid::Uuid;

use crate::{DbSchema, Row};

/// IDL connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdlState {
    /// Initial state, not connected
    Initial,
    /// Waiting for schema response
    SchemaRequested,
    /// Waiting for monitor response
    MonitorRequested,
    /// Actively monitoring changes
    Monitoring,
}

/// In-memory replica of an OVSDB database.
pub struct Idl {
    /// Current state
    state: IdlState,
    /// Database schema
    schema: Option<DbSchema>,
    /// Tables indexed by name, containing rows indexed by UUID
    tables: HashMap<String, HashMap<Uuid, Row>>,
    /// Change sequence number (incremented on each update)
    change_seqno: u64,
}

impl Default for Idl {
    fn default() -> Self {
        Self::new()
    }
}

impl Idl {
    /// Create a new IDL instance.
    pub fn new() -> Self {
        Self {
            state: IdlState::Initial,
            schema: None,
            tables: HashMap::new(),
            change_seqno: 0,
        }
    }

    /// Get the current state.
    pub fn state(&self) -> IdlState {
        self.state
    }

    /// Get the schema (if loaded).
    pub fn schema(&self) -> Option<&DbSchema> {
        self.schema.as_ref()
    }

    /// Get the current change sequence number.
    pub fn change_seqno(&self) -> u64 {
        self.change_seqno
    }

    /// Check if the IDL is connected and monitoring.
    pub fn is_monitoring(&self) -> bool {
        self.state == IdlState::Monitoring
    }

    /// Get all rows in a table.
    pub fn rows(&self, table: &str) -> impl Iterator<Item = &Row> {
        self.tables
            .get(table)
            .map(|t| t.values())
            .into_iter()
            .flatten()
    }

    /// Get a row by UUID.
    pub fn row(&self, table: &str, uuid: &Uuid) -> Option<&Row> {
        self.tables.get(table).and_then(|t| t.get(uuid))
    }

    /// Set the schema (called after get_schema response).
    pub fn set_schema(&mut self, schema: DbSchema) {
        // Initialize empty tables for each table in schema
        for table_name in schema.tables.keys() {
            self.tables.insert(table_name.clone(), HashMap::new());
        }
        self.schema = Some(schema);
        self.state = IdlState::MonitorRequested;
    }

    /// Process an update from the server.
    pub fn process_update(&mut self, update: &serde_json::Value) {
        if let Some(obj) = update.as_object() {
            for (table_name, table_update) in obj {
                self.process_table_update(table_name, table_update);
            }
        }
        self.change_seqno += 1;
    }

    /// Process updates for a single table.
    fn process_table_update(&mut self, table_name: &str, update: &serde_json::Value) {
        if let Some(obj) = update.as_object() {
            for (uuid_str, row_update) in obj {
                if let Ok(uuid) = uuid_str.parse::<Uuid>() {
                    self.process_row_update(table_name, uuid, row_update);
                }
            }
        }
    }

    /// Process update for a single row.
    fn process_row_update(
        &mut self,
        table_name: &str,
        uuid: Uuid,
        update: &serde_json::Value,
    ) {
        let table = self.tables.entry(table_name.to_owned()).or_default();

        if let Some(obj) = update.as_object() {
            // Check for delete
            if obj.contains_key("old") && !obj.contains_key("new") {
                table.remove(&uuid);
                return;
            }

            // Insert or update
            if let Some(new) = obj.get("new") {
                let row = table.entry(uuid).or_insert_with(|| Row::new(uuid));
                if let Some(new_obj) = new.as_object() {
                    let values: HashMap<String, serde_json::Value> = new_obj
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    row.update(&values);
                }
            }
        }
    }

    /// Mark as monitoring (called after successful monitor request).
    pub fn set_monitoring(&mut self) {
        self.state = IdlState::Monitoring;
    }
}
