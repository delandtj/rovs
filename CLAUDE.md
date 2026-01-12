# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**rovs** (Rust Open vSwitch) is a Rust library replacing Python OVS bindings. It provides async, type-safe APIs for OVSDB and OpenFlow protocols.

## Build Commands

```bash
cargo build                           # Build all crates
cargo build -p rovs-ovsdb             # Build specific crate
cargo clippy                          # Lint (pedantic warnings enabled)
cargo doc --open                      # Generate and view documentation
```

## Testing

```bash
# Unit tests (no OVSDB required)
cargo test --lib --all

# Integration tests (requires OVSDB server, see below)
OVSDB_ADDR=unix:/tmp/ovs-test/db.sock cargo test -- --ignored

# Run specific example
OVSDB_ADDR=unix:/tmp/ovs-test/db.sock cargo run --example ovsdb_transaction
```

## Examples

Located in `rovs-client/examples/`:

| Example | Description |
|---------|-------------|
| `ovsdb_transaction` | Basic bridge/port creation and patch ports |
| `ovsdb_monitor` | Real-time database monitoring |
| `list_bridges` | High-level client API usage |
| `add_flow` | OpenFlow flow programming |

## CI Pipeline

GitHub Actions runs on every push/PR:
- **check**: `cargo fmt --check`, `cargo clippy`, `cargo doc`
- **unit-tests**: `cargo test --lib --all`
- **integration-tests**: Tests against real OVSDB server (user-space)
- **examples**: Runs all examples against OVSDB

## Crate Architecture

```
rovs-transport  → Network layer (Unix/TCP/TLS)
       ↓
rovs-jsonrpc    → JSON-RPC 1.0 (brace-depth parsing, no newlines)
       ↓
rovs-ovsdb      → OVSDB client, IDL, transactions
       ↓
rovs-client     → High-level API and examples

rovs-types      → Shared types (Atom, MacAddr)
rovs-openflow   → OpenFlow 1.3 (in progress, OXM fields defined)
```

## Key Design Patterns

### OVSDB Transaction Flow
1. Create `Transaction::new("Open_vSwitch")`
2. Use `txn.insert()` which returns `RowRef::Named(name)` for cross-references
3. Reference rows in same transaction with `RowRef::to_json()` → `["named-uuid", "row0"]`
4. `client.commit(&mut txn)` populates `txn.uuid_map()` with actual UUIDs

### IDL (In-memory Database Layer)
- `client.idl().rows("Bridge")` - iterate table rows
- `row.get_string("name")`, `row.get_set("ports")`, `row.get_map("external_ids")`
- Change tracking via `idl.change_seqno()`

### JSON-RPC Parsing
The connection uses brace-depth tracking (not newlines) to parse JSON - OVSDB servers don't send newlines after responses.

## Testing with User-Space OVSDB

```bash
mkdir -p /tmp/ovs-test
ovsdb-tool create /tmp/ovs-test/conf.db /usr/share/openvswitch/vswitch.ovsschema
ovsdb-server /tmp/ovs-test/conf.db \
    --remote=punix:/tmp/ovs-test/db.sock \
    --unixctl=/tmp/ovs-test/ovsdb.ctl \
    --pidfile=/tmp/ovs-test/ovsdb.pid \
    --detach
ovs-vsctl --db=unix:/tmp/ovs-test/db.sock init
```

Set `OVSDB_ADDR=unix:/tmp/ovs-test/db.sock` for examples.

## Workspace Configuration

- Rust 2024 edition, requires rustc 1.85+
- `unsafe_code = "deny"` enforced
- Clippy pedantic warnings enabled
- Uses workspace dependencies (edit root Cargo.toml, use `cargo add` for new deps)

## OVSDB Protocol Notes

- Strong references enforce referential integrity - delete operations must remove references first
- Set encoding: empty = `["set", []]`, single = raw value, multiple = `["set", [v1, v2]]`
- UUID ref: `["uuid", "..."]`, named-uuid: `["named-uuid", "row0"]`
- Monitor versions: V1 (original), V2 (monitor_cond), V3 (monitor_cond_since)
