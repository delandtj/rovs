# ADR 001: Display Formatting for OpenFlow Types

## Status

Accepted

## Context

The core OpenFlow types in `rovs-openflow` (`Match`, `Action`, `ActionList`, `OutputPort`, `Instruction`, `InstructionList`) only have derived `Debug` implementations. The `Debug` output is unreadable for operational use — it dumps all 33 `Option::None` fields in `Match` and shows raw byte arrays instead of human-readable representations.

Consumers like `mosnet flows` currently use `format!("{:?}", ...)` which produces multi-line noise instead of the compact `ovs-ofctl dump-flows` style that operators expect.

## Decision

Implement `std::fmt::Display` on the core OpenFlow types in `rovs-openflow`, matching `ovs-ofctl dump-flows` formatting conventions:

### Types and Format Examples

| Type | Format example |
|------|---------------|
| `OutputPort` | `1`, `NORMAL`, `CONTROLLER`, `LOCAL`, `IN_PORT`, `FLOOD`, `ALL` |
| `Action` | `output:1`, `NORMAL`, `set_eth_src:aa:bb:cc:dd:ee:ff`, `resubmit(,1)`, `ct(commit,zone=1)` |
| `ActionList` | `output:1,set_eth_src:aa:bb:cc:dd:ee:ff` (comma-separated) |
| `Match` | `in_port=1,eth_type=0x0800,ipv4_dst=10.0.0.1/24` (only non-None fields; empty → `*`) |
| `Instruction` | `goto_table:1`, `apply_actions(output:1,NORMAL)`, `clear_actions`, `meter:1` |
| `InstructionList` | Comma-separated; single `ApplyActions` unwraps to just the actions |

### Formatting Conventions

- MAC addresses: `aa:bb:cc:dd:ee:ff` (lowercase hex, colon-separated)
- `eth_type`: hex (`0x0800`, `0x86dd`, `0x0806`)
- IP with mask: `10.0.0.1/24` (omit mask if /32 or /128)
- ARP op: `arp_op=1`
- CT state: `ct_state=+trk+est` (OVS flag notation with `+` prefix)
- Empty match: `*`
- Nicira resubmit: `resubmit(,table)` or `resubmit(port,table)`
- CT: `ct(commit,zone=1,table=2)` or `ct(zone=1)`
- CT+NAT: `ct(commit,zone=1,nat(src=10.0.0.1))`

### Implementation Location

Directly on the types in `rovs-openflow` (not in `rovs-ext`), since `Display` is a fundamental trait that belongs on the core types. A private `fn format_mac([u8; 6])` helper is added in the action module to avoid depending on `rovs-ext`.

## Consequences

- `mosnet flows` and other consumers can use `format!("{}", ...)` for readable output
- `Debug` remains available for development/debugging
- No new dependencies required
- Breaking change: none (additive trait impl only)
