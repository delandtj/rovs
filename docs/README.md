# rovs Documentation

Rust Open vSwitch library - a Rust replacement for Python OVS bindings.

![CI](https://github.com/delandtj/rovs/actions/workflows/ci.yml/badge.svg)

## Documentation Index

| Document | Description |
|----------|-------------|
| [api-reference.md](api-reference.md) | Complete API documentation with function signatures |
| [ovsdb-implementation.md](ovsdb-implementation.md) | OVSDB architecture and implementation details |
| [openflow-planning.md](openflow-planning.md) | OpenFlow implementation plan for next session |
| [p4-programming-overview.md](p4-programming-overview.md) | P4 network programming background |

## Quick Start

### Testing OVSDB

```bash
# Start user-space OVSDB server
mkdir -p /tmp/ovs-test
ovsdb-tool create /tmp/ovs-test/conf.db /usr/share/openvswitch/vswitch.ovsschema
ovsdb-server /tmp/ovs-test/conf.db \
    --remote=punix:/tmp/ovs-test/db.sock \
    --unixctl=/tmp/ovs-test/ovsdb.ctl \
    --pidfile=/tmp/ovs-test/ovsdb.pid \
    --detach
ovs-vsctl --db=unix:/tmp/ovs-test/db.sock init

# Run example
cargo run --example ovsdb_transaction
```

### Example Code

```rust
use rovs_ovsdb::{Client, Transaction};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::connect("unix:/tmp/ovs-test/db.sock").await?;

    // Create a bridge
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge("br0");
    txn.add_internal_port("br0", "vport0");

    client.commit(&mut txn).await?;
    Ok(())
}
```

## Running Tests

```bash
# Unit tests (no external dependencies)
cargo test --lib --all

# Integration tests (requires OVSDB server)
OVSDB_ADDR=unix:/tmp/ovs-test/db.sock cargo test -- --ignored
```

## Examples

Examples are located in `rovs-client/examples/`:

| Example | Description |
|---------|-------------|
| `ovsdb_transaction` | Basic bridge/port creation and patch ports |
| `ovsdb_monitor` | Real-time database monitoring |
| `list_bridges` | High-level client API usage |
| `add_flow` | OpenFlow flow programming |

Run examples with:
```bash
OVSDB_ADDR=unix:/tmp/ovs-test/db.sock cargo run --example <name>
```

## Crate Structure

```
rovs/
├── rovs-transport/     # Network transport (Unix, TCP, TLS)
├── rovs-jsonrpc/       # JSON-RPC 1.0 protocol
├── rovs-ovsdb/         # OVSDB client and IDL
├── rovs-openflow/      # OpenFlow protocol (in progress)
├── rovs-types/         # Shared types
├── rovs-client/        # High-level client (examples)
└── docs/               # Documentation
```

## Implementation Status

| Feature | Status |
|---------|--------|
| Transport (Unix/TCP/TLS) | Complete |
| JSON-RPC connection | Complete |
| OVSDB schema fetch | Complete |
| OVSDB monitoring | Complete |
| OVSDB transactions | Complete |
| High-level topology ops | Complete |
| OpenFlow protocol | Planned |
| OpenFlow controller | Planned |

## Next Steps (OpenFlow)

See [openflow-planning.md](openflow-planning.md) for the detailed implementation plan.

Priority:
1. OpenFlow message encoding/decoding
2. Connection handshake (Hello, Features)
3. Flow mod operations
4. Packet In/Out handling
5. Controller framework
