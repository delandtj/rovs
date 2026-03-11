# ADR 002: AppCtl / Unixctl Client for vswitchd Management

## Status

Proposed

## Context

`ovs-appctl dpif/dump-flows <bridge>` is a key operational tool that dumps datapath flows from `ovs-vswitchd`. There is currently no Rust equivalent in rovs. The command uses the OVS **unixctl protocol** — JSON-RPC 1.0 over a Unix domain socket — to communicate with `ovs-vswitchd` (not `ovsdb-server`).

The unixctl protocol differs from OVSDB in minor ways:
- Connects to the **vswitchd** control socket (`/var/run/openvswitch/ovs-vswitchd.<pid>.ctl`), not the OVSDB socket
- Error field is a **plain string** (or null), not a `{"error": "...", "details": "..."}` object
- Params are a **flat array of strings**, not structured JSON
- Responses are typically **plain text** in the result field

However, the wire protocol is the same JSON-RPC 1.0 with the same brace-depth framing that `rovs-jsonrpc` already implements. The only incompatibility is the error format in `RpcError`.

### What `dpif/dump-flows` returns

One flow per line in OVS datapath flow format:

```
recirc_id(0),in_port(1),eth(src=00:01:02:03:04:05,dst=ff:ff:ff:ff:ff:ff),eth_type(0x0806), packets:5, bytes:210, used:1.234s, actions:2
recirc_id(0),in_port(2),eth_type(0x0800),ipv4(src=10.0.0.1), packets:100, bytes:10000, used:0.500s, actions:set(eth(src=02:00:00:00:00:01)),1
```

Each line contains: `<match-key>, packets:<n>, bytes:<n>, used:<n>s, actions:<actions>`

Some lines may also include `flags:` and `mask:` fields depending on OVS version and verbosity flags.

### Command scope

The unixctl interface exposes dozens of commands. We scope to what's operationally relevant for rovs users:

**Must have:**

| Command | Description |
|---------|-------------|
| `dpif/dump-flows [-m] <bridge>` | Dump datapath flows with stats |
| `dpif/show` | Show datapaths with ports and stats |

**Should have:**

| Command | Description |
|---------|-------------|
| `dpctl/dump-conntrack [-m] [-s] [zone=]` | Dump connection tracking entries |
| `dpctl/ct-stats-show [zone=] [-m]` | Conntrack counts by protocol |
| `dpctl/flush-conntrack [zone=] [ct-tuples]` | Flush conntrack entries (NAT resets) |

These align with rovs-ext's existing NAT/ct flow templates — if you're installing SNAT/DNAT flows, you need to inspect and flush conntrack state.

**Out of scope (for now):**
- `dpctl/add-dp`, `del-dp`, `add-if`, `del-if` — OVSDB handles datapath plumbing
- `dpctl/add-flow`, `mod-flow`, `del-flow` — OpenFlow handles flow programming
- `dpctl/cache-*`, `dpctl/ipf-*`, `dpctl/ct-*-tcp-seq-chk` — niche tuning
- `ofproto/trace`, `fdb/show`, `coverage/show` — add when needed

## Decision

### Fix `rovs-jsonrpc::RpcError` to handle both error formats

Rather than duplicating JSON-RPC handling, extend `RpcError` with a custom `Deserialize` impl that accepts both forms:

- **OVSDB** sends: `{"error": {"error": "constraint violation", "details": "..."}, ...}`
- **unixctl** sends: `{"error": "unknown command", ...}`

A plain string deserializes to `RpcError { error: "the string", details: None }`. This is a one-line serde change — no new JSON-RPC implementation needed.

This lets `AppCtl` use `rovs-jsonrpc::Connection` directly, with all its brace-depth parsing and request/response matching.

> **Note:** As a follow-up, error types and shared structs (`RpcError`, `Response`, etc.) should be consolidated into a common space so type definitions aren't scattered across crates. (Tracked separately.)

### Module placement: `rovs-ext::appctl`

Create a new `appctl` module in `rovs-ext` — consistent with the crate's role as "higher-level abstractions" built on the protocol layers.

### AppCtl client

```rust
use std::path::Path;

pub struct AppCtl { /* rovs_jsonrpc::Connection */ }

impl AppCtl {
    /// Connect to a specific unixctl socket path.
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self>;

    /// Discover and connect to the default ovs-vswitchd socket.
    /// Searches /var/run/openvswitch/ovs-vswitchd.*.ctl
    pub async fn connect_default() -> Result<Self>;

    // --- Must have ---

    /// Dump datapath flows for a bridge.
    /// Equivalent to `ovs-appctl dpif/dump-flows <bridge>`.
    pub async fn dpif_dump_flows(&mut self, bridge: &str) -> Result<Vec<DpifFlow>>;

    /// Dump datapath flows with wildcard mask information.
    /// Equivalent to `ovs-appctl dpif/dump-flows -m <bridge>`.
    pub async fn dpif_dump_flows_verbose(&mut self, bridge: &str) -> Result<Vec<DpifFlow>>;

    /// Show datapaths with port info and stats.
    /// Equivalent to `ovs-appctl dpif/show`.
    pub async fn dpif_show(&mut self) -> Result<String>;

    // --- Should have ---

    /// Dump connection tracking entries.
    /// Equivalent to `ovs-appctl dpctl/dump-conntrack`.
    pub async fn dump_conntrack(&mut self, zone: Option<u16>) -> Result<Vec<ConntrackEntry>>;

    /// Show conntrack statistics by protocol.
    /// Equivalent to `ovs-appctl dpctl/ct-stats-show`.
    pub async fn ct_stats(&mut self, zone: Option<u16>) -> Result<String>;

    /// Flush conntrack entries, optionally filtered by zone.
    /// Equivalent to `ovs-appctl dpctl/flush-conntrack`.
    pub async fn flush_conntrack(&mut self, zone: Option<u16>) -> Result<()>;
}
```

### Parsed types

```rust
/// A parsed datapath flow from `dpif/dump-flows`.
pub struct DpifFlow {
    /// Full match key (e.g., "recirc_id(0),in_port(1),eth_type(0x0806)")
    pub key: String,
    /// Wildcard mask (present with -m flag, e.g., "recirc_id(0),in_port(ffff)")
    pub mask: Option<String>,
    /// Actions (e.g., "2" or "set(eth(src=02:00:00:00:00:01)),1")
    pub actions: String,
    /// Packet count
    pub packets: u64,
    /// Byte count
    pub bytes: u64,
    /// Seconds since last used (None if never matched)
    pub used: Option<f64>,
}

/// A parsed connection tracking entry from `dpctl/dump-conntrack`.
pub struct ConntrackEntry {
    /// Protocol (e.g., "tcp", "udp", "icmp")
    pub protocol: String,
    /// Connection state (e.g., "ESTABLISHED", "SYN_SENT")
    pub state: Option<String>,
    /// Source address
    pub src: String,
    /// Destination address
    pub dst: String,
    /// Source port (if applicable)
    pub sport: Option<u16>,
    /// Destination port (if applicable)
    pub dport: Option<u16>,
    /// Zone ID
    pub zone: Option<u16>,
    /// Remaining entry text for fields we don't parse
    pub raw: String,
}
```

`key`, `mask`, and `actions` on `DpifFlow` are kept as strings — the datapath flow format has dozens of nested field types, and the primary use case is display/filtering rather than programmatic field manipulation.

`ConntrackEntry` parses the most useful fields (protocol, state, endpoints, zone) and keeps the rest in `raw` for anything we missed.

### Socket discovery

The vswitchd socket path follows the pattern `/var/run/openvswitch/ovs-vswitchd.<pid>.ctl`. The `connect_default()` method will glob for this pattern and use the first match. Users can also provide an explicit path via `connect()`.

### Error variant

Add `AppCtl(String)` to the `rovs-ext::Error` enum for command-level errors returned by vswitchd.

## Alternatives Considered

### 1. Duplicate JSON-RPC handling for unixctl

Implement a second, simpler JSON-RPC client using raw `serde_json::Value` to sidestep the `RpcError` format difference. Rejected — two JSON-RPC implementations in one codebase is unnecessary complexity when a one-line serde fix resolves the incompatibility.

### 2. Create a new `rovs-appctl` crate

A dedicated crate for the unixctl protocol. Rejected — the implementation is small and fits naturally in `rovs-ext` alongside the other operational abstractions.

### 3. Expose a generic `run()` escape hatch

Add `pub async fn run(&mut self, command: &str, args: &[&str]) -> Result<String>` for arbitrary commands. Rejected — prefer a curated API surface. When a new command is needed, add a typed method with proper parsing. This keeps the API intentional and discoverable.

### 4. Fully parse datapath flow keys into structured types

Parse match keys into typed fields (e.g., `in_port: u32`, `eth_src: MacAddr`). Rejected for now — the datapath flow format has complex nested syntax and many field types. Can be added as methods on `DpifFlow` later if demand emerges.

## Consequences

- Operators get Rust-native datapath and conntrack inspection without shelling out
- `rovs-jsonrpc` becomes usable for both OVSDB and unixctl protocols (small, backward-compatible change)
- API surface is curated: must-have + should-have commands only, extended as needed
- Conntrack methods complement the existing NAT/ct flow templates in `rovs-ext::flows`
- Testing requires a running `ovs-vswitchd` (full mode container), same as existing OpenFlow integration tests
- Follow-up: consolidate shared types (errors, RPC structs) into a common crate space
