# ADR 002: Live Flow Monitoring via Nicira Flow Monitor

## Status

Proposed

## Context

Currently rovs only supports **polling** for flow table state via `dump_flows` (OpenFlow multipart Flow Stats request/reply). This requires the caller to periodically request a full table dump and diff the results to detect changes. There is no way to receive a live stream of flow add/modify/delete events.

OVS supports event-driven flow monitoring through the **Nicira Flow Monitor** extension (`NXST_FLOW_MONITOR`), which is the mechanism behind `ovs-ofctl monitor`. This extension allows a controller to register interest in flow table changes and receive asynchronous notifications as they happen.

Use cases:
- Observability dashboards that react to flow changes in real time
- Controllers that need to track flows installed by other controllers or `ovs-ofctl`
- Debugging tools that log flow churn without repeated polling

### Protocol Overview

The Nicira Flow Monitor uses the **Multipart Experimenter** framework:

1. **Request**: Sent as a Multipart Request with type `Experimenter` (0xffff), containing a Nicira vendor header (`vendor=0x00002320`, `subtype=2` for `NXST_FLOW_MONITOR`) followed by one or more monitor request bodies.

2. **Reply stream**: OVS sends Multipart Experimenter replies containing flow update entries:
   - `ADDED` (0) — flow was added (or initial snapshot entry)
   - `DELETED` (1) — flow was removed
   - `MODIFIED` (2) — flow was changed
   - `ABBREV` (3) — abbreviated notification for the caller's own changes

3. **Flow control**: If OVS falls behind, it sends `PAUSED` / `RESUMED` vendor messages (Experimenter type 4, not multipart).

### Monitor Request Flags

| Flag | Value | Meaning |
|------|-------|---------|
| `INITIAL` | 1 | Send existing flows as ADDED events |
| `ADD` | 2 | Notify on new flows |
| `DELETE` | 4 | Notify on deleted flows |
| `MODIFY` | 8 | Notify on modified flows |
| `ACTIONS` | 16 | Include actions in update entries |
| `OWN` | 32 | Include updates caused by the caller's own messages |

## Decision

### API Design

Add a `flow_monitor` module to `rovs-openflow` with two VConn methods following the existing `recv_packet_in()` pattern:

```rust
use rovs_openflow::{FlowMonitorRequest, FlowUpdate, monitor_flags};

// Open a dedicated connection for monitoring
let mut mon = VConn::connect(&addr).await?;

// Register monitor — returns initial snapshot (if INITIAL flag set)
let request = FlowMonitorRequest::all_changes(1); // monitor ID = 1
let initial = mon.monitor_flows(request).await?;

// Receive ongoing updates
loop {
    let updates = mon.recv_flow_updates().await?;
    for update in updates {
        match update {
            FlowUpdate::Full(f) => println!("{:?} flow: table={} priority={}",
                f.event, f.table_id, f.priority),
            FlowUpdate::Abbrev { xid } => println!("own change (xid={})", xid),
        }
    }
}
```

### Types

- `FlowMonitorRequest` — builder for monitor registration (id, flags, table filter, match filter, output port filter)
- `FlowUpdate` — enum: `Full(FlowUpdateFull)` or `Abbrev { xid }`
- `FlowUpdateFull` — event type, reason, priority, timeouts, table, cookie, match fields, actions
- `FlowUpdateEvent` — enum: `Added`, `Deleted`, `Modified`, `Abbrev`
- `monitor_flags` — constants module

### VConn Methods

- `monitor_flows(&mut self, req: FlowMonitorRequest) -> Result<Vec<FlowUpdate>>` — sends request, collects initial snapshot via multipart replies (loops until no MORE flag)
- `recv_flow_updates(&mut self) -> Result<Vec<FlowUpdate>>` — blocks until next batch of update messages, handles echo requests internally

### Encoding

The request is wrapped in a Multipart Experimenter message:

```
[OF Header: type=MultipartRequest]
[Multipart Header: type=0xffff, flags=0, pad=0]
[Nicira Header: vendor=0x00002320, subtype=2]
[Monitor Body: id(4) + flags(2) + out_port(2) + match_len(2) + table_id(1) + pad(1)]
[OXM match TLVs]
```

### Decoding

Reply parsing extracts flow update entries from Multipart Experimenter reply bodies. Each entry is self-describing via a `length` header. Match fields in replies use NXM encoding (class 0x0000 for NXM0, class 0x0001 for NXM1) — the existing `Match::decode_oxm()` handles NXM1 but needs a small addition for NXM0.

### Single Monitor Per Connection

OVS flow monitor update entries do not carry a monitor ID, so multiple monitors on one connection produce ambiguous interleaved events. The API supports a single active monitor per VConn, matching the standard OVS practice of dedicating a connection to monitoring.

### File Organization

| File | Change |
|------|--------|
| `rovs-openflow/src/flow_monitor.rs` | **New** — types, encoding, decoding |
| `rovs-openflow/src/vconn.rs` | Add `monitor_flows()` and `recv_flow_updates()` |
| `rovs-openflow/src/match_fields.rs` | Add NXM0 class handling in `decode_oxm()` |
| `rovs-openflow/src/lib.rs` | Add module and re-exports |

## Consequences

- Enables event-driven flow monitoring without polling
- No new dependencies (uses existing `bytes`, `tokio` already in the crate)
- `&mut self` approach means the caller dedicates a VConn to monitoring — this is a natural constraint, not a limitation, since OVS controllers typically use separate connections for monitoring
- Future enhancement: a channel-based `FlowMonitorHandle` that spawns a background task could allow monitoring alongside other VConn operations on a shared connection
- PAUSED/RESUMED flow control events should be handled gracefully (log + re-request on RESUMED) but can be deferred to a follow-up if the initial use case doesn't require high-churn environments
