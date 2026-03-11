# rovs-ext Examples

This directory contains examples demonstrating various OVS configurations using the rovs library.

## Quick Start

```bash
# Start OVS container (OVSDB only)
./scripts/test-with-ovs.sh start

# Start OVS container with OpenFlow support (required for flow examples)
./scripts/test-with-ovs.sh start full

# Run an example
OVSDB_ADDR=tcp:127.0.0.1:6640 cargo run -p rovs-ext --example topology_builder
OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example stateful_firewall
```

## Examples by Category

### AppCtl / Datapath Inspection

| Example | Description | Requires |
|---------|-------------|----------|
| [`appctl_inspect`](appctl_inspect.rs) | Dump datapath flows, conntrack entries, and stats | Full (vswitchd) |

### OVSDB / Topology

| Example | Description | Requires |
|---------|-------------|----------|
| [`topology_builder`](topology_builder.rs) | Bridge pairs, VLAN trunks, access ports | OVSDB |
| [`cloud_hypervisor_vm`](cloud_hypervisor_vm.rs) | Attach VM tap interfaces to OVS bridges | OVSDB |

### OpenFlow / Flow Programming

| Example | Description | Requires |
|---------|-------------|----------|
| [`flow_templates`](flow_templates.rs) | MAC NAT, learning switch using flow templates | OpenFlow |
| [`stateful_firewall`](stateful_firewall.rs) | Connection tracking with ct_state matching | OpenFlow |
| [`nat_gateway`](nat_gateway.rs) | High-level SNAT/DNAT using flow templates | OpenFlow |
| [`install_nat_flows`](install_nat_flows.rs) | Dual-stack NAT for flow inspection | OpenFlow |

### Advanced Flow Examples

| Example | Description | Requires |
|---------|-------------|----------|
| [`ipv6_features`](ipv6_features.rs) | NDP proxy, NAT66, IPv6 firewall, dual-stack | OpenFlow |
| [`advanced_nat`](advanced_nat.rs) | Hairpin NAT, load balancing, 1:1 static NAT | OpenFlow |
| [`enhanced_firewall`](enhanced_firewall.rs) | Multi-zone firewall with policy matrix | OpenFlow |
| [`network_gateway`](network_gateway.rs) | Complete gateway: SNAT, DNAT, firewall, ARP/NDP proxy | OpenFlow |

### Controller Examples

| Example | Description | Requires |
|---------|-------------|----------|
| [`arp_ndp_controller`](arp_ndp_controller.rs) | ARP/NDP proxy using controller framework | OpenFlow + Controller |

### Low-Level Tests

| Example | Description | Requires |
|---------|-------------|----------|
| [`test_ct`](test_ct.rs) | Connection tracking action tests | OpenFlow |
| [`test_nat`](test_nat.rs) | NAT action encoding tests | OpenFlow |

## Example Details

### appctl_inspect

Connects directly to the `ovs-vswitchd` unixctl socket to inspect switch internals — the Rust equivalent of `ovs-appctl`. Shows datapath overview, flow table with stats, conntrack entries, and conntrack statistics.

```bash
# Start container with vswitchd (required for appctl)
./scripts/test-with-ovs.sh start full

# Basic inspection
cargo run -p rovs-ext --example appctl_inspect

# Inspect specific bridge with verbose flow masks
cargo run -p rovs-ext --example appctl_inspect -- --bridge br0 -m

# Filter conntrack by zone, flush before inspecting
cargo run -p rovs-ext --example appctl_inspect -- --zone 1 --flush

# Specify socket path explicitly
VSWITCHD_SOCKET=/var/run/openvswitch/ovs-vswitchd.123.ctl \
    cargo run -p rovs-ext --example appctl_inspect
```

### topology_builder

Demonstrates OVSDB topology builders:
- `BridgePair`: Two bridges connected by patch ports
- `VlanTrunk`: Bridge with VLAN access and trunk ports
- Adding ports to existing bridges

```bash
OVSDB_ADDR=tcp:127.0.0.1:6640 cargo run -p rovs-ext --example topology_builder
```

### cloud_hypervisor_vm

Shows how to integrate cloud-hypervisor VMs with OVS:
- Create a VM bridge with OpenFlow support
- Attach tap interfaces created by cloud-hypervisor
- Configure VLAN isolation between VMs
- Optional NAT gateway for VM internet access

```bash
OVSDB_ADDR=tcp:127.0.0.1:6640 cargo run -p rovs-ext --example cloud_hypervisor_vm
```

### stateful_firewall

3-table connection tracking pipeline:
- Table 0: Send packets through CT
- Table 1: Policy based on ct_state (established, new, invalid)
- Table 2: Output after commit

```bash
OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example stateful_firewall
```

### ipv6_features

IPv6-specific OpenFlow features:
- NDP proxy (Neighbor Solicitation to controller)
- NAT66 (IPv6-to-IPv6 address translation)
- IPv6 firewall with ICMPv6 handling
- Dual-stack policies

```bash
OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example ipv6_features --no-cleanup
```

### advanced_nat

Advanced NAT patterns beyond basic SNAT/DNAT:
- **Hairpin NAT**: Internal client -> public IP -> internal server
- **Load Balancing**: DNAT to multiple backends
- **1:1 Static NAT**: Bidirectional IP mapping

```bash
OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example advanced_nat --scenario 1
```

### enhanced_firewall

Multi-zone stateful firewall:
- 3 security zones: Internal, DMZ, External
- Zone-to-zone policy matrix
- Service-based filtering (HTTP, SSH, etc.)
- Blocked traffic logging to controller

```bash
OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example enhanced_firewall --verbose
```

### network_gateway

Complete gateway combining all features:
- Dual-stack SNAT (IPv4 + IPv6)
- DNAT for exposed services
- Stateful firewall
- ARP/NDP proxy via controller
- MAC learning on internal side

```bash
OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example network_gateway --no-cleanup
```

## Common Patterns

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OVSDB_ADDR` | `tcp:127.0.0.1:6640` | OVSDB server address |
| `OPENFLOW_ADDR` | `tcp:127.0.0.1:6653` | OpenFlow switch address |
| `CONTROLLER_ADDR` | `tcp:0.0.0.0:6653` | Controller listen address |

### Flow Inspection

Most OpenFlow examples support `--no-cleanup` to leave flows installed:

```bash
# Run example without cleanup
OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example ipv6_features -- --no-cleanup

# Inspect OpenFlow flows via container
podman exec rovs-ovsdb-test ovs-ofctl dump-flows br-test -O OpenFlow13

# Inspect datapath flows natively via appctl (no shelling out)
cargo run -p rovs-ext --example appctl_inspect -- --bridge br-test -m

# Manual cleanup
podman exec rovs-ovsdb-test ovs-ofctl del-flows br-test -O OpenFlow13
```

### Connection Tracking Notes

When using `ct()` action with commit flag, OVS requires `eth_type` in the match:
- IPv4: `Match::new().eth_type(0x0800)`
- IPv6: `Match::new().eth_type(0x86dd)`

Without eth_type, you'll get `BadAction` error code 10 (`OFPBAC_MATCH_INCONSISTENT`).

### Table Layout Conventions

Most examples follow a similar table layout:
- Table 0: Initial classification / L2
- Table 1+: Connection tracking entry
- Middle tables: Policy decisions
- Final table: Output

## Running Against Real OVS

For production use (not containerized):

```bash
# Point to your OVS installation
export OVSDB_ADDR=unix:/var/run/openvswitch/db.sock
export OPENFLOW_ADDR=tcp:127.0.0.1:6653

# Or TCP
export OVSDB_ADDR=tcp:192.168.1.1:6640
```
