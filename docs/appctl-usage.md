# rovs-ext::appctl — OVS Unixctl Client

`rovs-ext` has an `appctl` module that connects directly to the `ovs-vswitchd` unixctl socket (the same protocol `ovs-appctl` uses). It uses `rovs-jsonrpc::Connection` under the hood — no separate JSON-RPC implementation.

## Connecting

```rust
use rovs_ext::appctl::AppCtl;

// Explicit socket path
let mut ctl = AppCtl::connect("/var/run/openvswitch/ovs-vswitchd.123.ctl").await?;

// Auto-discover (globs /var/run/openvswitch/ovs-vswitchd.*.ctl)
let mut ctl = AppCtl::connect_default().await?;
```

## Datapath Flows

```rust
// Dump flows (equivalent to: ovs-appctl dpif/dump-flows br0)
let flows = ctl.dpif_dump_flows("br0").await?;
for flow in &flows {
    println!("{flow}");
    // Output: recirc_id(0),in_port(1),eth_type(0x0800) => 2  [150 pkts, 12.3 KB, used 0.500s ago]
}

// With wildcard masks (equivalent to: ovs-appctl dpif/dump-flows -m br0)
let flows = ctl.dpif_dump_flows_verbose("br0").await?;

// Show datapaths (equivalent to: ovs-appctl dpif/show)
let info = ctl.dpif_show().await?;
println!("{info}");
```

### DpifFlow

| Field | Type | Description |
|-------|------|-------------|
| `key` | `String` | Match key (e.g., `recirc_id(0),in_port(1),eth_type(0x0800)`) |
| `mask` | `Option<String>` | Wildcard mask (present with `-m` verbose mode) |
| `actions` | `String` | Actions (e.g., `2` or `set(eth(src=02:00:00:00:00:01)),1`) |
| `packets` | `u64` | Packet count |
| `bytes` | `u64` | Byte count |
| `used` | `Option<f64>` | Seconds since last match (`None` if never matched) |

The `key` and `actions` are kept as strings — not parsed into structured types.

`Display` format: `key => actions  [stats]` with human-friendly byte units (B/KB/MB/GB), singular "pkt", and "never used" for idle flows.

## Connection Tracking

```rust
// Dump conntrack (equivalent to: ovs-appctl dpctl/dump-conntrack)
let entries = ctl.dump_conntrack(None).await?;

// Filtered by zone
let entries = ctl.dump_conntrack(Some(1)).await?;
for e in &entries {
    println!("{e}");
    // Output: tcp ESTABLISHED 10.0.0.1:54321 -> 10.0.0.2:80 (zone=1)
}

// Conntrack stats (equivalent to: ovs-appctl dpctl/ct-stats-show)
let stats = ctl.ct_stats(None).await?;

// Flush conntrack (equivalent to: ovs-appctl dpctl/flush-conntrack)
ctl.flush_conntrack(Some(1)).await?; // flush zone 1
ctl.flush_conntrack(None).await?;    // flush all
```

### ConntrackEntry

| Field | Type | Description |
|-------|------|-------------|
| `protocol` | `String` | Protocol name (`tcp`, `udp`, `icmp`) |
| `state` | `Option<String>` | Connection state (`ESTABLISHED`, `SYN_SENT`, etc.) |
| `src` | `String` | Source address |
| `dst` | `String` | Destination address |
| `sport` | `Option<u16>` | Source port (TCP/UDP only) |
| `dport` | `Option<u16>` | Destination port (TCP/UDP only) |
| `zone` | `Option<u16>` | Zone ID |
| `mark` | `Option<u32>` | Mark value |
| `raw` | `String` | Full original line for unparsed fields |

`Display` format: `protocol [state] src[:port] -> dst[:port] [(zone, mark)]`

## Re-exports

Types are available from the crate root:

```rust
use rovs_ext::{AppCtl, DpifFlow, ConntrackEntry};
```

## RpcError Compatibility

`rovs-jsonrpc::RpcError` deserializes from both OVSDB format (`{"error": "...", "details": "..."}`) and unixctl format (plain `"error string"`). This allows `AppCtl` to reuse the same `rovs-jsonrpc::Connection` as OVSDB — no duplicate JSON-RPC implementation.

## Error Handling

AppCtl errors surface as `rovs_ext::Error::AppCtl(String)` — this covers both connection failures and command-level errors returned by vswitchd.
