# rovs Documentation

Rust Open vSwitch library - a complete Rust replacement for Python OVS bindings.

## Crate Structure

```
rovs/
├── rovs-transport/     # Network transport (Unix, TCP, TLS)
├── rovs-jsonrpc/       # JSON-RPC 1.0 protocol
├── rovs-ovsdb/         # OVSDB client and IDL
├── rovs-openflow/      # OpenFlow 1.3 + Nicira extensions
├── rovs-types/         # Shared types
├── rovs-client/        # High-level client and examples
├── rovs-ext/           # Flow templates, topology builders, controller framework
└── docs/               # Documentation
```

## Quick Start

### Using the Container (Recommended)

```bash
# Run all tests (unit + integration + examples)
./scripts/test-with-ovs.sh

# Run with full OpenFlow support (ovs-vswitchd)
./scripts/test-with-ovs.sh start full
OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example nat_gateway
```

### Using System OVS

```bash
# Check your OVS bridge
sudo ovs-vsctl show

# Run example (adjust address to your setup)
OPENFLOW_ADDR=tcp:127.0.0.1:6654 cargo run -p rovs-ext --example install_nat_flows

# Inspect flows
sudo ovs-ofctl dump-flows br-nat
```

## Implementation Status

| Feature | Status |
|---------|--------|
| Transport (Unix/TCP/TLS) | Complete |
| JSON-RPC connection | Complete |
| OVSDB client & IDL | Complete |
| OVSDB transactions | Complete |
| OpenFlow 1.3 protocol | Complete |
| Nicira extensions | Complete |
| Connection tracking (ct) | Complete |
| NAT (SNAT/DNAT) | Complete |
| IPv6 support | Complete |
| Controller framework | Complete |
| Flow templates | Complete |
| Topology builders | Complete |

## Examples

### rovs-client examples

| Example | Description |
|---------|-------------|
| `ovsdb_transaction` | Basic bridge/port creation and patch ports |
| `ovsdb_monitor` | Real-time database monitoring |
| `list_bridges` | High-level client API usage |
| `add_flow` | OpenFlow flow programming |
| `dual_bridge_vlan` | VLAN routing between two bridges |
| `mac_learning` | MAC learning with NxLearn action |
| `mac_translation` | MAC NAT with Nicira extensions |
| `vlan_mac_nat` | VLAN bridge with MAC NAT for IPv4/IPv6 |
| `ndp_controller` | OpenFlow controller for NDP proxy |

### rovs-ext examples

| Example | Description |
|---------|-------------|
| `nat_gateway` | SNAT/DNAT with high-level API |
| `install_nat_flows` | Dual-stack NAT for inspection |
| `test_nat` | NAT action encoding tests |
| `test_ct` | Connection tracking tests |
| `stateful_firewall` | Stateful firewall with ct |

Run examples:
```bash
OPENFLOW_ADDR=tcp:127.0.0.1:6654 cargo run -p rovs-ext --example nat_gateway
```

## Code Examples

### OVSDB Transaction

```rust
use rovs_ovsdb::{Client, Transaction};

let mut client = Client::connect("unix:/tmp/ovs-test/db.sock").await?;

let mut txn = Transaction::new("Open_vSwitch");
txn.create_bridge("br0");
txn.add_internal_port("br0", "vport0");
client.commit(&mut txn).await?;
```

### OpenFlow with Nicira Extensions

```rust
use rovs_openflow::{VConn, Flow, Match, ActionList};

let mut conn = VConn::connect(&addr).await?;

// MAC NAT with Nicira register load
let flow = Flow::add()
    .table(0).priority(100)
    .match_fields(Match::new().eth_type(0x0800).in_port(1))
    .actions(ActionList::new()
        .nx_reg_load(OxmHeader::EthSrc, mac_bytes)
        .output(2));
conn.send_flow_sync(&flow).await?;
```

### Dual-Stack NAT Gateway

```rust
use rovs_ext::flows::{SnatConfig, SnatGateway};
use std::net::{Ipv4Addr, Ipv6Addr};

let snat = SnatGateway::new(
    SnatConfig::dual_stack(
        Ipv4Addr::new(203, 0, 113, 1),
        Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
        1,  // internal port
        2,  // external port
    )
    .zone(1)
    .port_range(10000, 65000)
);
snat.install(&mut conn, 0, 100).await?;
```

### DNAT Port Forwarding

```rust
use rovs_ext::flows::{DnatConfig, DnatService};

let dnat = DnatService::new(
    DnatConfig::new(2, 1)  // external port, internal port
        .zone(2)
        .forward_tcp(80, Ipv4Addr::new(192, 168, 1, 10), 8080)
        .forward_tcp(443, Ipv4Addr::new(192, 168, 1, 10), 8443)
        .forward_tcp_v6(80, Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 10), 8080)
);
dnat.install(&mut conn, 10, 100).await?;
```

### OpenFlow Controller

```rust
use rovs_ext::controller::{Controller, ControllerConfig};
use rovs_ext::controller::protocol::ArpProxyHandler;

let mut controller = Controller::new(&addr, ControllerConfig::default()).await?;

let mut arp_handler = ArpProxyHandler::new();
arp_handler.add_entry([10, 0, 0, 99], [0x02, 0x00, 0x00, 0x00, 0x00, 0x99]);
controller.register(arp_handler);

controller.run().await?;
```

## Running Tests

```bash
# Unit tests (no external dependencies)
cargo test --lib --all

# Integration tests with container
./scripts/test-with-ovs.sh

# OpenFlow tests (requires full mode)
./scripts/test-with-ovs.sh start full
OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo test -p rovs-ext -- --ignored
```

## Key Features

### Flow Templates (rovs-ext)

| Template | Description |
|----------|-------------|
| `MacNatFlows` | MAC address translation between ports |
| `ArpProxyFlows` | ARP proxy with static entries |
| `NdpProxyFlows` | NDP proxy (requires controller) |
| `LearningSwitchFlows` | MAC learning with NxLearn |
| `SnatGateway` | Source NAT (masquerade) - IPv4/IPv6 |
| `DnatService` | Destination NAT (port forwarding) - IPv4/IPv6 |
| VLAN helpers | Push/pop/translate VLAN tags |

### Topology Builders (rovs-ext)

| Builder | Description |
|---------|-------------|
| `BridgePair` | Two bridges connected by patch ports |
| `VlanTrunk` | Bridge with VLAN access and trunk ports |

### Nicira Extensions (rovs-openflow)

| Extension | Description |
|-----------|-------------|
| `NxRegLoad` | Load value into register/field |
| `NxMove` | Copy bits between fields |
| `NxLearn` | Dynamic flow learning |
| `ct()` | Connection tracking |
| `ct(nat)` | NAT inside connection tracking |
| `resubmit` | Resubmit to another table |
