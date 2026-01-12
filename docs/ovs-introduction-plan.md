# OVS Rust Library Implementation Plan

Transform OVS Python OVSDB + OpenFlow functionality into a Rust library for network automation.

**Target**: Tokio-based async library
**Scope**: OVSDB topology management + OpenFlow flow programming

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    High-Level API                           │
│  OvsClient { db: OvsDb, openflow: OpenFlowClient }         │
├─────────────────────────────────────────────────────────────┤
│  OVSDB Layer                │  OpenFlow Layer              │
│  ├─ Idl (replica)           │  ├─ VConn (connection)       │
│  ├─ Transaction             │  ├─ FlowMod (add/del/mod)    │
│  ├─ Row/Table               │  ├─ Match/Actions            │
│  └─ Schema                  │  └─ Bundle (atomic ops)      │
├─────────────────────────────┴──────────────────────────────┤
│  Protocol Layer                                             │
│  ├─ JSON-RPC (ovsdb-server)                                │
│  └─ OpenFlow Protocol (1.0-1.5)                            │
├─────────────────────────────────────────────────────────────┤
│  Transport Layer (tokio)                                    │
│  ├─ Unix socket                                             │
│  ├─ TCP                                                     │
│  └─ TLS (tokio-rustls)                                      │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Foundation Crates

### Step 1.1: Project Structure
Create workspace with multiple crates:
```
rovs/
├── Cargo.toml              # workspace
├── rovs-types/             # Shared types (Atom, Datum, UUID)
├── rovs-transport/         # Stream abstraction (unix/tcp/tls)
├── rovs-jsonrpc/           # JSON-RPC protocol
├── rovs-ovsdb/             # OVSDB client (IDL, Transaction)
├── rovs-openflow/          # OpenFlow protocol & flows
├── rovs-client/            # High-level unified API
└── examples/
```

### Step 1.2: Transport Layer (`rovs-transport`)
Port `python/ovs/stream.py` patterns:
- `Stream` trait for async read/write
- `UnixStream` - tokio::net::UnixStream wrapper
- `TcpStream` - tokio::net::TcpStream wrapper
- `TlsStream` - tokio-rustls integration
- `StreamAddress` enum for connection strings (`unix:`, `tcp:`, `ssl:`)
- Auto-reconnection state machine (from `python/ovs/reconnect.py`)

**Key files to reference**:
- `python/ovs/stream.py` (32KB) - stream abstraction
- `python/ovs/reconnect.py` (26KB) - reconnection logic

### Step 1.3: Types Crate (`rovs-types`)
Port `python/ovs/db/data.py` and `python/ovs/db/types.py`:
- `Atom` enum: `Integer(i64)`, `Real(f64)`, `Boolean(bool)`, `String(String)`, `Uuid(Uuid)`
- `Datum` - collection type (Set or Map of Atoms)
- `OvsType` - type definitions with constraints
- Serde serialization for JSON-RPC encoding

---

## Phase 2: JSON-RPC Layer

### Step 2.1: JSON-RPC Protocol (`rovs-jsonrpc`)
Port `python/ovs/jsonrpc.py`:
- `Message` enum: `Request`, `Notify`, `Reply`, `Error`
- `Connection` - low-level send/recv over stream
- `Session` - high-level with reconnection, multiplexing
- Async methods: `send()`, `recv()`, `transact()`

**Key patterns**:
- Request IDs for matching replies
- Notification handling (no reply expected)
- Error propagation

---

## Phase 3: OVSDB Client

### Step 3.1: Schema Layer
Port `python/ovs/db/schema.py`:
- `DbSchema` - parsed database schema
- `TableSchema` - table definition (columns, constraints)
- `ColumnSchema` - column definition (type, references)
- Schema parsing from JSON (vswitch.ovsschema)

### Step 3.2: IDL Core
Port `python/ovs/db/idl.py` (2400+ lines):

**Idl struct**:
- In-memory replica of OVSDB tables
- State machine: `Initial → SchemaRequested → MonitorRequested → Monitoring`
- `run()` - process incoming updates
- `wait()` - async event integration
- Change sequence number tracking

**Row struct**:
- Represents database row
- Column access via methods or derive macro
- UUID-based foreign key resolution
- Tracks modifications for transactions

**Table struct**:
- Container for rows indexed by UUID
- Conditional monitoring support

### Step 3.3: Transactions
Port transaction handling from `python/ovs/db/idl.py:1630+`:
- `Transaction` - ACID transaction builder
- Operations: insert, modify, delete, verify
- Status: `Uncommitted → Incomplete → Success/Failed/TryAgain`
- `commit()` - async non-blocking
- `commit_block()` - async blocking until complete

### Step 3.4: Schema Helper & Code Generation
- `SchemaHelper` - selective table/column registration
- **Derive macro** for type-safe row access:
```rust
#[derive(OvsRow)]
#[ovs(table = "Bridge")]
struct Bridge {
    name: String,
    ports: Vec<Uuid>,  // references to Port
    // ...
}
```

---

## Phase 4: OpenFlow Client

### Step 4.1: OpenFlow Protocol Types (`rovs-openflow`)
Port from `include/openvswitch/ofp-flow.h` and `lib/ofp-flow.c`:

**Match fields** (from test patterns):
- L2: `dl_src`, `dl_dst`, `dl_type`, `vlan_vid`, `vlan_pcp`
- L3: `nw_src`, `nw_dst`, `nw_proto`, `nw_tos`, `nw_ttl`
- L4: `tcp_src`, `tcp_dst`, `udp_src`, `udp_dst`
- Tunnel: `tun_id`, `tun_src`, `tun_dst`
- Metadata: `in_port`, `metadata`, `cookie`

**Actions** (from `python/ovs/flow/ofp_act.py`):
- `Output(port)`, `Drop`, `Controller`
- `SetField(field, value)`
- `PushVlan`, `PopVlan`
- `Group(id)`, `Meter(id)`

**FlowMod struct**:
```rust
struct FlowMod {
    command: FlowModCommand,  // Add, Modify, Delete
    table_id: u8,
    priority: u16,
    cookie: u64,
    match_fields: Match,
    actions: Vec<Action>,
    timeouts: Timeouts,
    flags: FlowModFlags,
}
```

### Step 4.2: Flow Parsing
Port `python/ovs/flow/ofp.py`:
- Parse human-readable flow syntax: `"in_port=1,ip,nw_src=10.0.0.0/8,actions=output:2"`
- Builder pattern for programmatic construction
- Serialization to OpenFlow wire format

### Step 4.3: VConn (Virtual Connection)
Port from `include/openvswitch/vconn.h` concepts:
- `VConn` - OpenFlow connection to switch
- Protocol version negotiation (OF 1.0-1.5)
- `send_flow_mod()`, `dump_flows()`, `del_flows()`
- Bundle support for atomic operations (OF 1.3+)

---

## Phase 5: High-Level API

### Step 5.1: Unified Client (`rovs-client`)
```rust
pub struct OvsClient {
    db: OvsDb,
    openflow: OpenFlowClient,
}

impl OvsClient {
    /// Connect to OVS instance
    pub async fn connect(ovsdb: &str, openflow: &str) -> Result<Self>;

    // Topology operations
    pub async fn list_bridges(&self) -> Result<Vec<Bridge>>;
    pub async fn create_bridge(&self, name: &str) -> Result<Bridge>;
    pub async fn add_port(&self, bridge: &str, port: &str) -> Result<Port>;

    // Flow operations
    pub async fn add_flow(&self, bridge: &str, flow: FlowMod) -> Result<()>;
    pub async fn del_flows(&self, bridge: &str, match_: Match) -> Result<()>;
    pub async fn dump_flows(&self, bridge: &str) -> Result<Vec<Flow>>;
}
```

### Step 5.2: Network Automation Helpers
- `FlowBuilder` - ergonomic flow construction
- `TopologyWatcher` - async stream of topology changes
- `FlowSync` - declarative flow management (desired state → reconcile)

---

## Implementation Order

| Step | Crate | Effort | Dependencies |
|------|-------|--------|--------------|
| 1.1 | workspace | Small | None |
| 1.2 | rovs-transport | Medium | tokio, tokio-rustls |
| 1.3 | rovs-types | Small | serde, uuid |
| 2.1 | rovs-jsonrpc | Medium | rovs-transport, rovs-types |
| 3.1 | rovs-ovsdb (schema) | Medium | rovs-types, serde_json |
| 3.2 | rovs-ovsdb (idl) | Large | rovs-jsonrpc, schema |
| 3.3 | rovs-ovsdb (txn) | Medium | idl |
| 3.4 | rovs-ovsdb (codegen) | Medium | syn, quote |
| 4.1 | rovs-openflow (types) | Medium | rovs-types |
| 4.2 | rovs-openflow (parse) | Medium | nom or pest |
| 4.3 | rovs-openflow (vconn) | Large | rovs-transport |
| 5.1 | rovs-client | Medium | all above |
| 5.2 | rovs-client (helpers) | Small | rovs-client |

---

## Key Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-rustls = "0.25"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "1"
tracing = "0.1"
bytes = "1"
nom = "7"  # for flow parsing
```

---

## Verification Strategy

1. **Unit tests**: Each crate has internal tests
2. **Integration tests**: Against real OVS instance
   - Docker container with OVS for CI
   - Test OVSDB operations (CRUD bridges/ports)
   - Test flow operations (add/dump/del)
3. **Compatibility tests**: Compare output with `ovs-vsctl` and `ovs-ofctl`
4. **Examples**: Working examples for each major feature

---

## Reference Files (from OVS source)

**OVSDB Python**:
- `python/ovs/db/idl.py` - main IDL implementation
- `python/ovs/db/schema.py` - schema parsing
- `python/ovs/db/data.py` - Atom/Datum types
- `python/ovs/jsonrpc.py` - JSON-RPC protocol
- `python/ovs/stream.py` - transport layer
- `python/ovs/reconnect.py` - reconnection FSM

**OpenFlow**:
- `python/ovs/flow/ofp.py` - flow parsing
- `utilities/ovs-ofctl.c` - reference CLI implementation
- `lib/ofp-flow.c` - flow encoding
- `include/openvswitch/ofp-flow.h` - flow structures
