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

See [`rovs-ext/examples/README.md`](rovs-ext/examples/README.md) for the complete list of examples covering OVSDB, OpenFlow, NAT, firewalls, and controllers.

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
       ↓
rovs-ext        → Higher-level abstractions (controller framework, flow templates, topology builders)

rovs-types      → Shared types (Atom, MacAddr)
rovs-openflow   → OpenFlow 1.3 + Nicira extensions (VConn, flows, controller)
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

### OpenFlow Controller Development
Use `VConn` for OpenFlow switch connections:
```rust
let mut conn = VConn::connect(&addr).await?;

// Install flows
let flow = Flow::add()
    .table(0).priority(100)
    .match_fields(Match::new().icmpv6_type(135))
    .actions(ActionList::new().controller(0xffff));
conn.send_flow_sync(&flow).await?;

// Receive packets from switch
let packet_in = conn.recv_packet_in().await?;

// Send packets to switch
let packet_out = PacketOut::new()
    .in_port(in_port)
    .actions(ActionList::new().output(1))
    .data(packet_data);
conn.send_packet_out(&packet_out).await?;
```

### Nicira Extensions
OVS-specific actions beyond standard OpenFlow:
- `NxRegLoad` - Load immediate value into register/field
- `NxMove` - Copy bits between fields
- `NxLearn` - Dynamic flow learning
- `resubmit(port, table)` - Resubmit packet to another table
- `ct()` - Connection tracking with optional NAT

Example MAC NAT with Nicira:
```rust
ActionList::new()
    .nx_reg_load(OxmHeader::EthSrc, mac_bytes)  // Set source MAC
    .nx_move(OxmHeader::EthDst, OxmHeader::EthSrc, 48)  // Copy dst->src
    .output(1)
```

Example IP NAT with connection tracking:
```rust
use rovs_openflow::{ActionList, NatConfig, CT_COMMIT};
use std::net::Ipv4Addr;

// Simple SNAT
ActionList::new().ct_snat(1, Some(2), Ipv4Addr::new(10, 0, 0, 1))

// SNAT with port range
let nat = NatConfig::snat(Ipv4Addr::new(10, 0, 0, 1))
    .port_range(5000, 6000)
    .random();
ActionList::new().ct_nat(CT_COMMIT, 1, Some(2), nat)

// DNAT to specific IP:port
let nat = NatConfig::dnat(Ipv4Addr::new(192, 168, 1, 100)).port(8080);
ActionList::new().ct_nat(CT_COMMIT, 1, Some(2), nat)
```

## Testing with Container (Recommended)

The easiest way to run integration tests is using the containerized OVS environment:

```bash
# Run all tests (unit + integration)
./scripts/test-with-ovs.sh

# Run only integration tests
./scripts/test-with-ovs.sh integration

# Run examples
./scripts/test-with-ovs.sh examples

# Run with ovs-vswitchd for OpenFlow testing (privileged)
./scripts/test-with-ovs.sh full

# Start container for manual testing
./scripts/test-with-ovs.sh start
# Then in another terminal:
OVSDB_ADDR=tcp:127.0.0.1:6640 cargo test -- --ignored
```

The container uses Alpine Linux with OVS userspace datapath - no kernel modules required.

### Container Modes

| Mode | Command | Privileges | Use Case |
|------|---------|------------|----------|
| ovsdb-only | `./scripts/test-with-ovs.sh integration` | None | OVSDB protocol testing |
| full | `./scripts/test-with-ovs.sh full` | `--privileged` | OpenFlow + packet forwarding |

### Manual Container Usage

```bash
# Build image
podman build -t rovs-ovsdb .

# Run ovsdb-only (rootless)
podman run --rm -d -p 6640:6640 --name rovs-ovsdb rovs-ovsdb ovsdb-only

# Run with ovs-vswitchd (needs privileges for userspace datapath)
podman run --rm -d --privileged -p 6640:6640 --name rovs-ovsdb rovs-ovsdb

# Connect
OVSDB_ADDR=tcp:127.0.0.1:6640 cargo run --example ovsdb_transaction
```

## Testing with Host OVSDB (Alternative)

If you prefer running OVS directly on the host:

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

## OpenFlow Protocol Notes

- Default OpenFlow port: 6653 (IANA assigned), legacy: 6633
- Controller configuration via OVSDB: `txn.set_controller("bridge", "tcp:127.0.0.1:6653")`
- OXM (OpenFlow Extensible Match) field format: class(2) + field(7) + hasmask(1) + length(1) + value
- Packet-In reasons: NoMatch (table miss), Action (output to controller), InvalidTtl
- Linux interface names limited to 15 characters (IFNAMSIZ - 1)
- **Connection tracking (ct action)**: When using `ct()` with commit flag, OVS requires `eth_type` in the match (0x0800 for IPv4, 0x86dd for IPv6). Without it: `BadAction` error code 10. See `rovs-ext/examples/test_ct.rs`.
- **NAT with ct action**: Use `ct_nat()`, `ct_snat()`, or `ct_dnat()` for IP address translation. See `rovs-ext/examples/test_nat.rs`.

## rovs-ext Crate

The `rovs-ext` crate provides higher-level abstractions for OVS automation:

### Flow Templates
Pre-built flow patterns for common scenarios:
```rust
use rovs_ext::flows::{MacNatFlows, MacNatConfig};

let flows = MacNatFlows::new(MacNatConfig::new(
    [0x02, 0x00, 0x00, 0x00, 0x00, 0x01],  // internal MAC
    [0x02, 0x00, 0x00, 0x00, 0x00, 0x99],  // external MAC
    1, 2,  // internal/external ports
));
flows.install(&mut conn, 0, 100).await?;
```

Available templates:
- `MacNatFlows` - MAC address translation between ports
- `ArpProxyFlows` - ARP proxy using Nicira extensions
- `NdpProxyFlows` - NDP proxy (requires controller handler)
- `LearningSwitchFlows` - MAC learning switch with NxLearn
- `SnatGateway` - SNAT for outbound traffic (like iptables MASQUERADE)
- `DnatService` - DNAT for port forwarding to internal servers
- VLAN helpers: `push_vlan_flow`, `pop_vlan_flow`, `VlanAccessPort`

### Topology Builders
Create complex topologies with OVSDB transactions:
```rust
use rovs_ext::topology::BridgePair;

let pair = BridgePair::new("br-int", "br-ext")
    .vlans(vec![100, 200]);
pair.create(&mut client).await?;
```

- `BridgePair` - Two bridges connected by patch ports
- `VlanTrunk` - Bridge with VLAN access and trunk ports

### Controller Framework
Event-driven packet processing:
```rust
use rovs_ext::controller::{Controller, ControllerConfig};
use rovs_ext::controller::protocol::ArpProxyHandler;

let mut controller = Controller::new(&addr, ControllerConfig::default()).await?;
let mut arp_handler = ArpProxyHandler::new();
arp_handler.add_entry([10, 0, 0, 99], [0x02, 0x00, 0x00, 0x00, 0x00, 0x99]);
controller.register(arp_handler);
controller.run().await?;
```

- `Controller` - Main event loop with VConn
- `PacketHandler` trait - Implement for custom packet handling
- `ArpProxyHandler` - Pre-built ARP proxy
- `NdpProxyHandler` - Pre-built NDP proxy
