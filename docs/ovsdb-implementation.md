# OVSDB Implementation in rovs

## Overview

The `rovs-ovsdb` crate provides a Rust implementation of the OVSDB (Open vSwitch Database) protocol as defined in RFC 7047. It enables monitoring and manipulation of OVS configuration through a type-safe, async API.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         rovs-ovsdb                              │
├─────────────────────────────────────────────────────────────────┤
│  Client                                                         │
│  ├── Connection management                                      │
│  ├── Schema fetching                                            │
│  ├── Monitor setup                                              │
│  └── Transaction execution                                      │
├─────────────────────────────────────────────────────────────────┤
│  IDL (In-memory Database Layer)                                 │
│  ├── Table storage (HashMap<String, Table>)                     │
│  ├── Row management (HashMap<Uuid, Row>)                        │
│  ├── Update processing                                          │
│  └── Change tracking (seqno)                                    │
├─────────────────────────────────────────────────────────────────┤
│  Transaction                                                    │
│  ├── Operation building (insert/update/delete/mutate)           │
│  ├── Named-UUID references                                      │
│  ├── High-level topology ops                                    │
│  └── Result processing                                          │
├─────────────────────────────────────────────────────────────────┤
│  Schema                                                         │
│  ├── Database schema parsing                                    │
│  ├── Table schemas                                              │
│  └── Column type information                                    │
└─────────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────┐
│                       rovs-jsonrpc                              │
├─────────────────────────────────────────────────────────────────┤
│  Connection                                                     │
│  ├── Stream splitting (read/write halves)                       │
│  ├── JSON message parsing (brace-depth tracking)                │
│  ├── Request/Response handling                                  │
│  └── Notification buffering                                     │
└─────────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────┐
│                       rovs-transport                            │
├─────────────────────────────────────────────────────────────────┤
│  Stream                                                         │
│  ├── Unix socket support                                        │
│  ├── TCP support                                                │
│  └── TLS support (via rustls)                                   │
└─────────────────────────────────────────────────────────────────┘
```

## Key Components

### 1. Client (`rovs-ovsdb/src/client.rs`)

The main entry point for OVSDB operations.

**Connection Flow:**
1. `Client::connect(addr)` - Establish connection
2. `fetch_schema()` - Get database schema via `get_schema` RPC
3. `start_monitor()` - Set up monitoring via `monitor` RPC
4. Client is now ready for `wait()` and `commit()` operations

**Monitor Versions:**
- `V1` - Original `monitor` method (OVSDB_UPDATE)
- `V2` - `monitor_cond` with conditions (OVSDB_UPDATE2)
- `V3` - `monitor_cond_since` with transaction IDs (OVSDB_UPDATE3)

### 2. IDL (`rovs-ovsdb/src/idl.rs`)

In-memory replica of the OVSDB database.

**Structure:**
```rust
pub struct Idl {
    schema: Option<DbSchema>,
    tables: HashMap<String, Table>,
    state: IdlState,
    change_seqno: u64,
}

struct Table {
    rows: HashMap<Uuid, Row>,
}
```

**Update Processing:**
The IDL processes OVSDB update notifications in the format:
```json
{
  "Bridge": {
    "<uuid>": {
      "new": { "name": "br0", "ports": [...] },
      "old": { "name": "br0", "ports": [...] }
    }
  }
}
```

- `new` only = row inserted
- `old` only = row deleted
- Both = row modified

### 3. Row (`rovs-ovsdb/src/row.rs`)

Represents a single row in an OVSDB table.

**Column Access:**
```rust
row.get("column_name")           // Option<&Atom>
row.get_string("name")           // Option<&str>
row.get_i64("tag")              // Option<i64>
row.get_bool("enabled")         // Option<bool>
row.get_uuid("_uuid")           // Option<Uuid>
row.get_set("ports")            // Vec<Atom>
row.get_map("external_ids")     // Vec<(Atom, Atom)>
```

**Atom Type:**
OVSDB values are represented as `Atom`:
```rust
pub enum Atom {
    String(String),
    Integer(i64),
    Real(f64),
    Boolean(bool),
    Uuid(Uuid),
}
```

### 4. Transaction (`rovs-ovsdb/src/transaction.rs`)

Builds OVSDB transactions following RFC 7047 semantics.

**Named-UUID References:**
When inserting rows, you get a `RowRef::Named(name)` that can be used to reference the row in subsequent operations within the same transaction:

```rust
let iface_ref = txn.insert("Interface", json!({"name": "eth0"}));
// iface_ref = RowRef::Named("row0")

let port_ref = txn.insert("Port", json!({
    "name": "eth0",
    "interfaces": iface_ref.to_json()  // ["named-uuid", "row0"]
}));
```

After commit, the `uuid_map` contains actual UUIDs:
```rust
txn.get_uuid("row0")  // Some(actual-uuid)
```

### 5. JSON-RPC Connection (`rovs-jsonrpc/src/connection.rs`)

**Critical Implementation Detail - No Newlines:**
OVSDB servers do NOT send newlines after JSON responses. The connection uses brace-depth tracking to parse complete JSON objects:

```rust
// Track JSON structure
let mut depth = 0;
for byte in buffer {
    match byte {
        b'{' if !in_string => depth += 1,
        b'}' if !in_string => {
            depth -= 1;
            if depth == 0 {
                // Complete JSON object found
            }
        }
        b'"' => in_string = !in_string,
        b'\\' if in_string => escape_next = true,
        _ => {}
    }
}
```

**Notification Buffering:**
Server notifications (updates) received while waiting for a response are buffered:
```rust
pending_notifications: VecDeque<Request>
```

## OVSDB Protocol Details

### Message Format

**Request:**
```json
{"method": "transact", "params": ["Open_vSwitch", ...], "id": 1}
```

**Response:**
```json
{"id": 1, "result": [...], "error": null}
```

**Notification (no id):**
```json
{"method": "update", "params": ["monitor-id", {...}]}
```

### Transaction Operations

**Insert:**
```json
{
  "op": "insert",
  "table": "Bridge",
  "row": {"name": "br0", "ports": ["named-uuid", "row1"]},
  "uuid-name": "row0"
}
```

**Update:**
```json
{
  "op": "update",
  "table": "Bridge",
  "where": [["_uuid", "==", ["uuid", "..."]]],
  "row": {"fail_mode": "secure"}
}
```

**Mutate:**
```json
{
  "op": "mutate",
  "table": "Bridge",
  "where": [["name", "==", "br0"]],
  "mutations": [["ports", "insert", ["named-uuid", "row1"]]]
}
```

**Delete:**
```json
{
  "op": "delete",
  "table": "Bridge",
  "where": [["name", "==", "br0"]]
}
```

### Set and Map Encoding

**Empty set:** `["set", []]`
**Single value:** `value` (not wrapped)
**Multiple values:** `["set", [v1, v2, ...]]`

**Empty map:** `["map", []]`
**Map with values:** `["map", [["k1", "v1"], ["k2", "v2"]]]`

**UUID reference:** `["uuid", "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"]`
**Named UUID:** `["named-uuid", "row0"]`

## Usage Examples

### Basic Monitoring

```rust
use rovs_ovsdb::Client;

let mut client = Client::connect("unix:/var/run/openvswitch/db.sock").await?;

// Read current state
for row in client.idl().rows("Bridge") {
    println!("Bridge: {}", row.get_string("name").unwrap_or("?"));
}

// Wait for changes
loop {
    client.wait().await?;
    println!("Update received, seqno: {}", client.idl().change_seqno());
}
```

### Creating Topology

```rust
use rovs_ovsdb::{Client, Transaction};

let mut client = Client::connect("unix:/tmp/ovs-test/db.sock").await?;

let mut txn = Transaction::new("Open_vSwitch");

// Create bridge with default port
let (bridge_ref, port_ref, iface_ref) = txn.create_bridge("br0");

// Add internal port
let (port_ref, iface_ref) = txn.add_internal_port("br0", "vport0");

// Add VLAN port
let (port_ref, iface_ref) = txn.add_vlan_port("br0", "vlan100", 100);

// Commit
if client.commit(&mut txn).await? {
    // Get actual UUIDs
    let bridge_uuid = txn.get_uuid("row2").unwrap();
}
```

### Patch Ports Between Bridges

```rust
let mut txn = Transaction::new("Open_vSwitch");

// Create two bridges
txn.create_bridge("br-int");
txn.create_bridge("br-ext");

// Create patch ports connecting them
let (p1, _, p2, _) = txn.add_patch_ports("br-int", "br-ext", None, None);
// Creates: patch-br-int-to-br-ext and patch-br-ext-to-br-int

client.commit(&mut txn).await?;
```

## Error Handling

### Transaction Errors

Transactions can fail for several reasons:
- **Constraint violation:** Duplicate unique values (e.g., bridge names)
- **Referential integrity:** Deleting rows still referenced
- **Unknown column:** Using non-existent columns in conditions

Check transaction result:
```rust
match client.commit(&mut txn).await {
    Ok(true) => println!("Success"),
    Ok(false) => println!("Transaction failed, check logs"),
    Err(e) => println!("RPC error: {}", e),
}
```

### Important: Strong References

OVSDB has strong and weak references. Strong references enforce referential integrity:
- You cannot delete a row that is strongly referenced
- You must remove the reference first, then delete

Example: To delete a bridge:
1. Remove bridge UUID from `Open_vSwitch.bridges`
2. Delete the Bridge row
3. Delete Port rows
4. Delete Interface rows

Use `delete_bridge_uuid()` which handles this correctly.

## Testing

### User-Space OVSDB Server

For testing without root privileges:

```bash
mkdir -p /tmp/ovs-test
ovsdb-tool create /tmp/ovs-test/conf.db /usr/share/openvswitch/vswitch.ovsschema
ovsdb-server /tmp/ovs-test/conf.db \
    --remote=punix:/tmp/ovs-test/db.sock \
    --unixctl=/tmp/ovs-test/ovsdb.ctl \
    --pidfile=/tmp/ovs-test/ovsdb.pid \
    --log-file=/tmp/ovs-test/ovsdb.log \
    --detach
ovs-vsctl --db=unix:/tmp/ovs-test/db.sock init
```

### Verification

```bash
# List databases
printf '{"method":"list_dbs","params":[],"id":1}' | nc -U /tmp/ovs-test/db.sock

# Show bridges
ovs-vsctl --db=unix:/tmp/ovs-test/db.sock show
```

## File Structure

```
rovs-ovsdb/
├── src/
│   ├── lib.rs          # Module exports
│   ├── client.rs       # OVSDB client
│   ├── idl.rs          # In-memory database
│   ├── row.rs          # Row representation
│   ├── schema.rs       # Schema parsing
│   ├── transaction.rs  # Transaction builder
│   └── error.rs        # Error types
└── Cargo.toml

rovs-jsonrpc/
├── src/
│   ├── lib.rs          # Module exports
│   ├── connection.rs   # JSON-RPC connection
│   ├── message.rs      # Request/Response types
│   └── error.rs        # Error types
└── Cargo.toml

rovs-transport/
├── src/
│   ├── lib.rs          # Module exports
│   ├── stream.rs       # Transport abstraction
│   ├── address.rs      # Address parsing
│   └── tls.rs          # TLS configuration
└── Cargo.toml
```
