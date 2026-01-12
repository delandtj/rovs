# rovs API Reference

Complete API documentation for all rovs crates.

## Table of Contents

- [rovs-transport](#rovs-transport)
- [rovs-jsonrpc](#rovs-jsonrpc)
- [rovs-ovsdb](#rovs-ovsdb)
- [rovs-types](#rovs-types)
- [rovs-openflow](#rovs-openflow)

---

## rovs-transport

Transport layer for Unix sockets, TCP, and TLS connections.

### Address

```rust
// File: rovs-transport/src/address.rs

/// Parsed connection address
pub enum Address {
    Unix(PathBuf),
    Tcp(SocketAddr),
    Ssl { addr: SocketAddr, server_name: String },
}

impl Address {
    /// Parse an address string
    /// Formats: "unix:/path", "tcp:host:port", "ssl:host:port"
    pub fn parse(s: &str) -> Result<Self, Error>;
}

impl FromStr for Address {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err>;
}
```

### Stream

```rust
// File: rovs-transport/src/stream.rs

/// Unified stream type for all transports
pub enum Stream {
    Unix(UnixStream),
    Tcp(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl Stream {
    /// Connect to an address
    pub async fn connect(addr: &Address) -> Result<Self, Error>;

    /// Connect with TLS configuration
    pub async fn connect_tls(addr: &Address, config: TlsConfig) -> Result<Self, Error>;
}

// Implements AsyncRead + AsyncWrite
impl AsyncRead for Stream { ... }
impl AsyncWrite for Stream { ... }
```

### TlsConfig

```rust
// File: rovs-transport/src/tls.rs

/// TLS configuration
pub struct TlsConfig {
    /// CA certificate for server verification
    pub ca_cert: Option<PathBuf>,
    /// Client certificate
    pub cert: Option<PathBuf>,
    /// Client private key
    pub key: Option<PathBuf>,
    /// Skip server verification (insecure)
    pub skip_verify: bool,
}

impl TlsConfig {
    pub fn new() -> Self;
    pub fn ca_cert(self, path: impl Into<PathBuf>) -> Self;
    pub fn client_cert(self, cert: impl Into<PathBuf>, key: impl Into<PathBuf>) -> Self;
    pub fn skip_verify(self) -> Self;
}
```

### Error

```rust
// File: rovs-transport/src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("Connection refused")]
    ConnectionRefused,
}
```

---

## rovs-jsonrpc

JSON-RPC 1.0 implementation for OVSDB protocol.

### Connection

```rust
// File: rovs-jsonrpc/src/connection.rs

/// JSON-RPC connection over a transport stream
pub struct Connection {
    // Internal: split stream, buffers, notification queue
}

impl Connection {
    /// Create a new connection from a stream
    pub fn new(stream: Stream) -> Self;

    /// Get the next request ID
    pub fn next_id(&self) -> u64;

    /// Send a request and wait for response
    /// Returns the result value or error
    pub async fn transact(&mut self, method: &str, params: Value) -> Result<Value>;

    /// Send a notification (no response expected)
    pub async fn notify(&mut self, method: &str, params: Value) -> Result<()>;

    /// Send a raw message
    pub async fn send_message(&mut self, msg: &Message) -> Result<()>;

    /// Receive a single message
    /// Handles OVSDB's non-newline-terminated JSON
    pub async fn recv_message(&mut self) -> Result<Message>;

    /// Check if there are pending notifications
    pub fn has_pending_notifications(&self) -> bool;

    /// Pop next pending notification
    pub fn pop_notification(&mut self) -> Option<Request>;

    /// Drain all pending notifications
    pub fn drain_notifications(&mut self) -> impl Iterator<Item = Request> + '_;
}
```

### Message Types

```rust
// File: rovs-jsonrpc/src/message.rs

/// JSON-RPC message (request or response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    Request(Request),
    Response(Response),
}

/// JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub params: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
}

impl Request {
    /// Create a new request with ID
    pub fn new(method: impl Into<String>, params: Value, id: u64) -> Self;

    /// Create a notification (no ID)
    pub fn notification(method: impl Into<String>, params: Value) -> Self;
}

/// JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}

/// JSON-RPC error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
```

### Error

```rust
// File: rovs-jsonrpc/src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("RPC error: {0:?}")]
    Rpc(RpcError),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Unexpected response ID: expected {expected}, got {got}")]
    UnexpectedId { expected: u64, got: u64 },

    #[error("Transport error: {0}")]
    Transport(#[from] rovs_transport::Error),
}
```

---

## rovs-ovsdb

OVSDB client with IDL (In-memory Database Layer) and transaction support.

### Client

```rust
// File: rovs-ovsdb/src/client.rs

/// OVSDB client
pub struct Client {
    // Internal: connection, config, idl, monitor_id
}

impl Client {
    /// Connect to an OVSDB server with default config (Open_vSwitch database)
    pub async fn connect(addr: &str) -> Result<Self>;

    /// Connect with custom configuration
    pub async fn connect_with_config(addr: &str, config: ClientConfig) -> Result<Self>;

    /// Get the IDL (in-memory database replica)
    pub fn idl(&self) -> &Idl;

    /// Get mutable IDL reference
    pub fn idl_mut(&mut self) -> &mut Idl;

    /// Get the database schema
    pub fn schema(&self) -> Option<&DbSchema>;

    /// Check if connected and monitoring
    pub fn is_connected(&self) -> bool;

    /// Run one iteration - process pending notifications
    /// Returns true if any updates were processed
    pub async fn run(&mut self) -> Result<bool>;

    /// Wait for next update from server (blocking)
    pub async fn wait(&mut self) -> Result<()>;

    /// Execute a raw transaction (JSON operations)
    pub async fn transact(&mut self, operations: Value) -> Result<Value>;

    /// Commit a Transaction object
    /// On success, populates transaction's uuid_map
    pub async fn commit(&mut self, txn: &mut Transaction) -> Result<bool>;

    /// Get list of databases on server
    pub async fn list_dbs(&mut self) -> Result<Vec<String>>;

    /// Cancel monitoring
    pub async fn cancel_monitor(&mut self) -> Result<()>;
}
```

### ClientConfig

```rust
// File: rovs-ovsdb/src/client.rs

/// Monitor protocol version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorVersion {
    V1,  // Original monitor
    V2,  // monitor_cond
    V3,  // monitor_cond_since
}

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Database name (default: "Open_vSwitch")
    pub database: String,
    /// Tables to monitor (None = all tables)
    pub tables: Option<Vec<String>>,
    /// Monitor protocol version
    pub monitor_version: MonitorVersion,
    /// Leader-only mode for clustered OVSDB
    pub leader_only: bool,
}

impl Default for ClientConfig {
    fn default() -> Self;  // Open_vSwitch, all tables, V1
}

impl ClientConfig {
    pub fn open_vswitch() -> Self;
    pub fn database(self, name: impl Into<String>) -> Self;
    pub fn tables(self, tables: Vec<String>) -> Self;
    pub fn monitor_version(self, version: MonitorVersion) -> Self;
}
```

### Idl

```rust
// File: rovs-ovsdb/src/idl.rs

/// IDL state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdlState {
    Initial,
    SchemaLoaded,
    Monitoring,
}

/// In-memory database layer
pub struct Idl {
    // Internal: schema, tables, state, change_seqno
}

impl Idl {
    /// Create a new empty IDL
    pub fn new() -> Self;

    /// Get the schema
    pub fn schema(&self) -> Option<&DbSchema>;

    /// Get IDL state
    pub fn state(&self) -> IdlState;

    /// Get change sequence number (increments on each update)
    pub fn change_seqno(&self) -> u64;

    /// Iterate over rows in a table
    pub fn rows(&self, table: &str) -> impl Iterator<Item = &Row>;

    /// Get a specific row by UUID
    pub fn get_row(&self, table: &str, uuid: &Uuid) -> Option<&Row>;

    /// Find rows matching a predicate
    pub fn find_rows<F>(&self, table: &str, predicate: F) -> Vec<&Row>
    where
        F: Fn(&Row) -> bool;

    /// Find first row matching predicate
    pub fn find_row<F>(&self, table: &str, predicate: F) -> Option<&Row>
    where
        F: Fn(&Row) -> bool;

    // Internal methods
    pub(crate) fn set_schema(&mut self, schema: DbSchema);
    pub(crate) fn set_monitoring(&mut self);
    pub(crate) fn process_update(&mut self, update: &Value);
}
```

### Row

```rust
// File: rovs-ovsdb/src/row.rs

/// A row in an OVSDB table
pub struct Row {
    // Internal: uuid, columns HashMap
}

impl Row {
    /// Get the row's UUID
    pub fn uuid(&self) -> Uuid;

    /// Get a column value
    pub fn get(&self, column: &str) -> Option<&Atom>;

    /// Get column as string
    pub fn get_string(&self, column: &str) -> Option<&str>;

    /// Get column as i64
    pub fn get_i64(&self, column: &str) -> Option<i64>;

    /// Get column as f64
    pub fn get_f64(&self, column: &str) -> Option<f64>;

    /// Get column as bool
    pub fn get_bool(&self, column: &str) -> Option<bool>;

    /// Get column as UUID
    pub fn get_uuid(&self, column: &str) -> Option<Uuid>;

    /// Get column as set (returns Vec for any column)
    pub fn get_set(&self, column: &str) -> Vec<Atom>;

    /// Get column as map
    pub fn get_map(&self, column: &str) -> Vec<(Atom, Atom)>;

    /// Get all column names
    pub fn columns(&self) -> impl Iterator<Item = &str>;
}

/// OVSDB atomic value
#[derive(Debug, Clone)]
pub enum Atom {
    String(String),
    Integer(i64),
    Real(f64),
    Boolean(bool),
    Uuid(Uuid),
}

impl Atom {
    pub fn as_str(&self) -> Option<&str>;
    pub fn as_i64(&self) -> Option<i64>;
    pub fn as_f64(&self) -> Option<f64>;
    pub fn as_bool(&self) -> Option<bool>;
    pub fn as_uuid(&self) -> Option<Uuid>;
}
```

### Transaction

```rust
// File: rovs-ovsdb/src/transaction.rs

/// Transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    Uncommitted,
    Unchanged,
    Incomplete,
    Aborted,
    Success,
    TryAgain,
    NotLocked,
    Error,
}

/// Reference to a row (existing UUID or named-uuid for inserts)
#[derive(Debug, Clone)]
pub enum RowRef {
    Uuid(Uuid),
    Named(String),
}

impl RowRef {
    /// Convert to JSON for OVSDB protocol
    /// Uuid -> ["uuid", "..."]
    /// Named -> ["named-uuid", "..."]
    pub fn to_json(&self) -> Value;
}

impl From<Uuid> for RowRef { ... }
impl From<&str> for RowRef { ... }
impl From<String> for RowRef { ... }

/// OVSDB transaction builder
pub struct Transaction {
    // Internal: db_name, operations, status, uuid_map, uuid_counter
}

impl Transaction {
    /// Create a new transaction for a database
    pub fn new(db_name: impl Into<String>) -> Self;

    /// Get transaction status
    pub fn status(&self) -> TransactionStatus;

    /// Get database name
    pub fn db_name(&self) -> &str;

    // === Basic Operations ===

    /// Insert a row, returns RowRef for use in subsequent operations
    pub fn insert(&mut self, table: &str, row: Value) -> RowRef;

    /// Insert with HashMap
    pub fn insert_raw(&mut self, table: &str, row: HashMap<String, Value>) -> RowRef;

    /// Update a row by UUID
    pub fn update(&mut self, table: &str, uuid: Uuid, columns: Value);

    /// Mutate a row (add/remove from sets or maps) by UUID
    pub fn mutate(&mut self, table: &str, uuid: Uuid, mutations: Vec<Value>);

    /// Mutate rows matching name == value
    pub fn mutate_by_name(&mut self, table: &str, name: &str, mutations: Vec<Value>);

    /// Mutate rows matching custom condition
    pub fn mutate_where(&mut self, table: &str, condition: Value, mutations: Vec<Value>);

    /// Delete row by UUID
    pub fn delete(&mut self, table: &str, uuid: Uuid);

    /// Delete rows matching name == value
    pub fn delete_by_name(&mut self, table: &str, name: &str);

    /// Delete rows matching condition
    pub fn delete_where(&mut self, table: &str, condition: Value);

    /// Add wait operation
    pub fn wait(
        &mut self,
        table: &str,
        columns: Vec<String>,
        condition: Value,
        expected: Value,
    );

    /// Add comment to transaction
    pub fn comment(&mut self, comment: impl Into<String>);

    // === High-Level Topology Operations ===

    /// Create bridge with default internal port
    /// Returns (bridge_ref, port_ref, iface_ref)
    pub fn create_bridge(&mut self, name: &str) -> (RowRef, RowRef, RowRef);

    /// Add port to existing bridge
    /// Returns (port_ref, iface_ref)
    pub fn add_port(
        &mut self,
        bridge_name: &str,
        port_name: &str,
        iface_type: &str,
    ) -> (RowRef, RowRef);

    /// Add internal port to bridge
    pub fn add_internal_port(
        &mut self,
        bridge_name: &str,
        port_name: &str,
    ) -> (RowRef, RowRef);

    /// Add VLAN port (internal type with tag)
    pub fn add_vlan_port(
        &mut self,
        bridge_name: &str,
        port_name: &str,
        vlan_id: u16,
    ) -> (RowRef, RowRef);

    /// Create patch port pair between two bridges
    /// Returns (port1_ref, iface1_ref, port2_ref, iface2_ref)
    pub fn add_patch_ports(
        &mut self,
        bridge1: &str,
        bridge2: &str,
        port1_name: Option<&str>,
        port2_name: Option<&str>,
    ) -> (RowRef, RowRef, RowRef, RowRef);

    /// Delete bridge by UUID (recommended)
    pub fn delete_bridge_uuid(
        &mut self,
        bridge_uuid: Uuid,
        port_uuids: &[Uuid],
        iface_uuids: &[Uuid],
    );

    /// Delete bridge by name (may fail due to references)
    pub fn delete_bridge(&mut self, name: &str);

    /// Delete port by UUID (recommended)
    pub fn delete_port_uuid(
        &mut self,
        bridge_uuid: Uuid,
        port_uuid: Uuid,
        iface_uuid: Uuid,
    );

    /// Delete port by name (may fail due to references)
    pub fn delete_port(&mut self, bridge_name: &str, port_name: &str);

    // === Transaction State ===

    /// Build transaction parameters for RPC
    pub fn build(&self) -> Value;

    /// Get operations (for debugging)
    pub fn operations(&self) -> &[Value];

    /// Check if transaction has operations
    pub fn is_empty(&self) -> bool;

    /// Get UUID map after commit
    pub fn uuid_map(&self) -> &HashMap<String, Uuid>;

    /// Look up actual UUID for named-uuid after commit
    pub fn get_uuid(&self, name: &str) -> Option<Uuid>;

    /// Process transaction result (called by Client::commit)
    pub fn process_result(&mut self, result: &Value) -> bool;

    // Status setters
    pub fn set_success(&mut self);
    pub fn set_error(&mut self);
    pub fn set_try_again(&mut self);
    pub fn abort(&mut self);
}
```

### Schema

```rust
// File: rovs-ovsdb/src/schema.rs

/// Database schema
#[derive(Debug, Clone)]
pub struct DbSchema {
    pub name: String,
    pub version: String,
    pub tables: HashMap<String, TableSchema>,
}

impl DbSchema {
    /// Parse schema from JSON
    pub fn from_json(value: &Value) -> Result<Self>;
}

/// Table schema
#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub columns: HashMap<String, ColumnSchema>,
    pub max_rows: Option<u64>,
    pub is_root: bool,
    pub indexes: Vec<Vec<String>>,
}

/// Column schema
#[derive(Debug, Clone)]
pub struct ColumnSchema {
    pub name: String,
    pub type_info: ColumnType,
    pub mutable: bool,
    pub ephemeral: bool,
}

/// Column type information
#[derive(Debug, Clone)]
pub struct ColumnType {
    pub key: BaseType,
    pub value: Option<BaseType>,
    pub min: u64,
    pub max: MaxValue,
}

#[derive(Debug, Clone)]
pub enum MaxValue {
    Exactly(u64),
    Unlimited,
}

#[derive(Debug, Clone)]
pub enum BaseType {
    Integer { min: Option<i64>, max: Option<i64> },
    Real { min: Option<f64>, max: Option<f64> },
    Boolean,
    String { min_length: Option<u64>, max_length: Option<u64> },
    Uuid { ref_table: Option<String>, ref_type: RefType },
}

#[derive(Debug, Clone, Copy)]
pub enum RefType {
    Strong,
    Weak,
}
```

### Error

```rust
// File: rovs-ovsdb/src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Transport error: {0}")]
    Transport(#[from] rovs_transport::Error),

    #[error("JSON-RPC error: {0}")]
    JsonRpc(#[from] rovs_jsonrpc::Error),

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Not connected")]
    NotConnected,
}
```

---

## rovs-types

Shared types across crates.

```rust
// File: rovs-types/src/lib.rs

// Re-exports commonly used types
pub use uuid::Uuid;
pub use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

// MAC address type (if needed)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddr([u8; 6]);

impl MacAddr {
    pub fn new(bytes: [u8; 6]) -> Self;
    pub fn from_slice(slice: &[u8]) -> Option<Self>;
    pub fn as_bytes(&self) -> &[u8; 6];
    pub fn is_broadcast(&self) -> bool;
    pub fn is_multicast(&self) -> bool;
}

impl FromStr for MacAddr { ... }
impl Display for MacAddr { ... }
```

---

## rovs-openflow

OpenFlow protocol implementation (in progress).

### Current Implementation

```rust
// File: rovs-openflow/src/oxm.rs

/// OXM (OpenFlow Extensible Match) class
#[repr(u16)]
pub enum OxmClass {
    Nxm0 = 0x0000,           // Nicira extended match
    Nxm1 = 0x0001,           // Nicira extended match
    OpenflowBasic = 0x8000,  // Standard OpenFlow match fields
}

/// OXM field types (OpenFlow 1.3)
#[repr(u8)]
pub enum OxmField {
    InPort = 0,
    InPhyPort = 1,
    Metadata = 2,
    EthDst = 3,
    EthSrc = 4,
    EthType = 5,
    VlanVid = 6,
    VlanPcp = 7,
    IpDscp = 8,
    IpEcn = 9,
    IpProto = 10,
    Ipv4Src = 11,
    Ipv4Dst = 12,
    TcpSrc = 13,
    TcpDst = 14,
    UdpSrc = 15,
    UdpDst = 16,
    SctpSrc = 17,
    SctpDst = 18,
    Icmpv4Type = 19,
    Icmpv4Code = 20,
    ArpOp = 21,
    ArpSpa = 22,
    ArpTpa = 23,
    ArpSha = 24,
    ArpTha = 25,
    Ipv6Src = 26,
    Ipv6Dst = 27,
    Ipv6Flabel = 28,
    Icmpv6Type = 29,
    Icmpv6Code = 30,
    Ipv6NdTarget = 31,
    Ipv6NdSll = 32,
    Ipv6NdTll = 33,
    MplsLabel = 34,
    MplsTc = 35,
    MplsBos = 36,
    PbbIsid = 37,
    TunnelId = 38,
    Ipv6Exthdr = 39,
}

/// Build an OXM header word
pub fn oxm_header(
    class: OxmClass,
    field: OxmField,
    has_mask: bool,
    length: u8,
) -> u32;
```

### Planned API (see openflow-planning.md)

```rust
// Future: rovs-openflow/src/flow.rs

pub struct Flow {
    pub table_id: u8,
    pub priority: u16,
    pub cookie: u64,
    pub idle_timeout: u16,
    pub hard_timeout: u16,
    pub match_fields: Vec<Match>,
    pub instructions: Vec<Instruction>,
}

impl Flow {
    pub fn builder() -> FlowBuilder;
}

pub struct FlowBuilder { ... }

impl FlowBuilder {
    pub fn table(self, id: u8) -> Self;
    pub fn priority(self, priority: u16) -> Self;
    pub fn cookie(self, cookie: u64) -> Self;
    pub fn idle_timeout(self, seconds: u16) -> Self;
    pub fn hard_timeout(self, seconds: u16) -> Self;

    // Match fields
    pub fn match_in_port(self, port: u32) -> Self;
    pub fn match_eth_dst(self, mac: MacAddr) -> Self;
    pub fn match_eth_src(self, mac: MacAddr) -> Self;
    pub fn match_eth_type(self, ethertype: u16) -> Self;
    pub fn match_vlan_vid(self, vid: u16) -> Self;
    pub fn match_ipv4_src(self, addr: Ipv4Addr) -> Self;
    pub fn match_ipv4_src_masked(self, addr: Ipv4Addr, mask: Ipv4Addr) -> Self;
    pub fn match_ipv4_dst(self, addr: Ipv4Addr) -> Self;
    pub fn match_tcp_dst(self, port: u16) -> Self;
    // ... more match methods

    // Actions
    pub fn action_output(self, port: u32) -> Self;
    pub fn action_drop(self) -> Self;
    pub fn action_set_field(self, field: Match) -> Self;
    pub fn action_push_vlan(self, ethertype: u16) -> Self;
    pub fn action_pop_vlan(self) -> Self;
    pub fn action_goto_table(self, table_id: u8) -> Self;
    // ... more action methods

    pub fn build(self) -> Flow;
}
```

---

## Usage Examples

### Complete OVSDB Workflow

```rust
use rovs_ovsdb::{Client, ClientConfig, Transaction, MonitorVersion};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect with custom config
    let config = ClientConfig::default()
        .database("Open_vSwitch")
        .monitor_version(MonitorVersion::V1);

    let mut client = Client::connect_with_config(
        "unix:/var/run/openvswitch/db.sock",
        config,
    ).await?;

    // Read existing topology
    println!("Current bridges:");
    for row in client.idl().rows("Bridge") {
        let name = row.get_string("name").unwrap_or("?");
        let ports = row.get_set("ports");
        println!("  {} ({} ports)", name, ports.len());
    }

    // Create topology
    let mut txn = Transaction::new("Open_vSwitch");

    // Create two bridges
    let (br_int, _, _) = txn.create_bridge("br-int");
    let (br_ext, _, _) = txn.create_bridge("br-ext");

    // Add ports
    txn.add_internal_port("br-int", "vport0");
    txn.add_vlan_port("br-int", "vlan100", 100);

    // Connect bridges with patch ports
    txn.add_patch_ports("br-int", "br-ext", None, None);

    // Commit
    if client.commit(&mut txn).await? {
        println!("Topology created successfully");

        // Get actual UUIDs
        if let Some(uuid) = txn.get_uuid("row0") {
            println!("br-int interface UUID: {}", uuid);
        }
    }

    // Monitor for changes
    loop {
        client.wait().await?;
        println!("Update received at seqno {}", client.idl().change_seqno());

        // Process changes
        for row in client.idl().rows("Interface") {
            if let Some(ofport) = row.get_i64("ofport") {
                let name = row.get_string("name").unwrap_or("?");
                println!("Interface {} has ofport {}", name, ofport);
            }
        }
    }
}
```

### Finding Specific Rows

```rust
// Find bridge by name
let br0 = client.idl()
    .find_row("Bridge", |row| row.get_string("name") == Some("br0"));

// Find all internal interfaces
let internal_ifaces: Vec<_> = client.idl()
    .find_rows("Interface", |row| row.get_string("type") == Some("internal"));

// Get all port UUIDs for a bridge
if let Some(bridge) = br0 {
    let port_uuids: Vec<Uuid> = bridge.get_set("ports")
        .into_iter()
        .filter_map(|atom| atom.as_uuid())
        .collect();
}
```

### Transaction Error Handling

```rust
let mut txn = Transaction::new("Open_vSwitch");
txn.create_bridge("test-br");

match client.commit(&mut txn).await {
    Ok(true) => {
        println!("Success!");
        // Access UUID map
        for (name, uuid) in txn.uuid_map() {
            println!("{} -> {}", name, uuid);
        }
    }
    Ok(false) => {
        // Transaction failed (constraint violation, etc.)
        println!("Transaction failed: {:?}", txn.status());
    }
    Err(e) => {
        // RPC/connection error
        println!("Error: {}", e);
    }
}
```
