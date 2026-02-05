# Caveats, Blockers, and Design Decisions

## Existing Rust Ecosystem

### OVSDB: `ovsdb` crate (v0.0.6)
- **Status**: Experimental, "heavily WIP"
- **Features**: TCP/Unix sockets, serde-based protocol, tokio async
- **Limitation**: Requires separate `ovsdb-build` crate for schema codegen
- **Docs**: https://docs.rs/ovsdb/latest/ovsdb/

**Decision needed**: Build on this or start fresh?
- Pro: Already has protocol basics
- Con: v0.0.6 is unstable, may have architectural decisions we disagree with

### OpenFlow: `rust_ofp` crate
- **Status**: Abandoned (2017), OpenFlow 1.0 only
- **Source**: https://github.com/baxtersa/rust_ofp
- **Reality**: Unusable for modern OVS (needs OF 1.3+)

**Decision needed**: We must implement OpenFlow from scratch.

---

## OVSDB Protocol Complexity

### 1. State Machine (7 states)
```
IDL_S_INITIAL
IDL_S_SERVER_SCHEMA_REQUESTED
IDL_S_SERVER_MONITOR_REQUESTED
IDL_S_DATA_MONITOR_REQUESTED
IDL_S_DATA_MONITOR_COND_REQUESTED
IDL_S_DATA_MONITOR_COND_SINCE_REQUESTED
IDL_S_MONITORING
```
This FSM handles connection establishment, schema negotiation, and monitor setup.
Must be implemented correctly for robust reconnection.

### 2. Three Monitor Protocols
| Protocol | Update Type | Features |
|----------|-------------|----------|
| `monitor` | OVSDB_UPDATE | Basic, OF 1.0 era |
| `monitor_cond` | OVSDB_UPDATE2 | Conditional filtering |
| `monitor_cond_since` | OVSDB_UPDATE3 | Conditional + incremental sync |

**Blocker**: Must support all three for compatibility with different ovsdb-server versions.

### 3. Cluster Awareness
- `leader_only` mode - only connect to Raft leader
- `cluster_id` tracking
- `_Server` database monitoring for cluster state
- Automatic failover between cluster members

**Complexity**: Production deployments use clustered OVSDB.

### 4. Distributed Locking
```python
self.lock_name = None
self.has_lock = False
self.is_lock_contended = False
```
OVSDB supports advisory locks for coordinating multiple clients.

### 5. Transaction Complexity
**Status codes**:
- `UNCOMMITTED` - not yet submitted
- `UNCHANGED` - no actual changes
- `INCOMPLETE` - commit in progress
- `ABORTED` - explicitly aborted
- `SUCCESS` - committed
- `TRY_AGAIN` - conflict, retry needed
- `NOT_LOCKED` - need lock first
- `ERROR` - hard failure

**Critical**: `TRY_AGAIN` requires automatic retry loop with sequence number tracking.

### 6. UUID Substitution
Newly inserted rows get temporary UUIDs that must be substituted with server-assigned UUIDs:
```python
["named-uuid", "row0"] → ["uuid", "actual-uuid-from-server"]
```

---

## OpenFlow Protocol Complexity

### 1. Binary Protocol
Unlike OVSDB (JSON-RPC), OpenFlow is a **binary protocol**:
- Fixed header: `struct ofp_header { version, type, length, xid }`
- Version-specific message formats
- Network byte order (big-endian)

**Blocker**: Need proper binary serialization (not just serde_json).

### 2. Version Differences
| Version | Year | Key Features |
|---------|------|--------------|
| OF 1.0 | 2009 | Single table, basic actions |
| OF 1.1 | 2011 | Multiple tables, groups |
| OF 1.3 | 2012 | Meters, bundles, OXM match |
| OF 1.4 | 2013 | Optical ports, bundles |
| OF 1.5 | 2014 | Egress tables, scheduling |

**Each version has different**:
- Message formats
- Match field encodings
- Action formats
- Error codes

### 3. OXM (OpenFlow Extensible Match)
Modern OpenFlow (1.2+) uses OXM for flexible match encoding:
```
OXM_HEADER = class(vendor) | field | hasmask | length
```
Over 40+ standard match fields, plus Nicira extensions.

### 4. Nicira Extensions
OVS adds many proprietary extensions:
- `NXM_*` match fields (predecessor to OXM)
- `NXAST_*` actions (resubmit, learn, etc.)
- Conntrack integration
- Registers (reg0-reg15)

**Blocker**: Must support Nicira extensions for practical OVS use.

### 5. Bundle Transactions (OF 1.3+)
Atomic multi-message transactions:
```
OFPT_BUNDLE_CONTROL (open)
OFPT_BUNDLE_ADD_MESSAGE (flow_mod 1)
OFPT_BUNDLE_ADD_MESSAGE (flow_mod 2)
OFPT_BUNDLE_CONTROL (commit)
```

---

## Design Decisions Needed

### Decision 1: OVSDB Foundation
**Options**:
1. **Fork/extend `ovsdb` crate** - faster start, inherit limitations
2. **Start fresh** - full control, more work
3. **Wrap Python via PyO3** - quick but adds Python dependency

**Recommendation**: Start fresh. The `ovsdb` crate is too immature and couples tightly to its codegen approach.

### Decision 2: Schema Handling
**Options**:
1. **Compile-time codegen** (like `ovsdb-build`) - type-safe, inflexible
2. **Runtime reflection** - flexible, runtime overhead
3. **Hybrid** - codegen for known schemas, reflection for dynamic

**Recommendation**: Hybrid. Provide derive macros for vswitch.ovsschema, allow runtime schema loading.

### Decision 3: OpenFlow Implementation Scope
**Options**:
1. **Full protocol implementation** - maximum control, huge effort
2. **Shell out to ovs-ofctl** - quick, requires OVS installed
3. **Minimal subset** - just flow add/mod/del/dump

**Recommendation**: Start with minimal subset (option 3), expand as needed.
Target OF 1.3+ only (don't bother with 1.0/1.1).

### Decision 4: Match Field Representation
**Options**:
1. **Enum with all fields** - exhaustive, large enum
2. **HashMap<String, Value>** - flexible, not type-safe
3. **Builder pattern** - ergonomic, can validate at build time

**Recommendation**: Builder pattern with typed methods:
```rust
Match::new()
    .in_port(1)
    .eth_type(0x0800)
    .ipv4_src("10.0.0.0/8".parse()?)
```

### Decision 5: Error Handling
**Options**:
1. **Single error enum** - simple, less specific
2. **Error per module** - specific, more boilerplate
3. **thiserror + anyhow** - best of both

**Recommendation**: `thiserror` for library errors, let users use `anyhow` if they want.

---

## Potential Blockers

### Blocker 1: OpenFlow Wire Format
The binary protocol with version-specific formats is complex.
**Mitigation**: Consider using `nom` for parsing, `bytes` for serialization.

### Blocker 2: Nicira Extension Coverage
Hundreds of extensions, not all documented.
**Mitigation**: Start with most common (resubmit, conntrack, registers).

### Blocker 3: Testing Without OVS
Need OVS running to integration test.
**Mitigation**:
- Docker container for CI
- Mock server for unit tests
- Record/replay of protocol traces

### Blocker 4: SSL/TLS Certificates
Production OVS uses SSL with certificates.
**Mitigation**: Support via `tokio-rustls`, document cert setup.

### Blocker 5: Schema Versioning
OVSDB schemas evolve (current vswitch.ovsschema is v8.x).
**Mitigation**: Version detection, graceful degradation for unknown columns.

---

## Suggested Implementation Order

Given blockers, recommend this order:

### Phase 1: OVSDB Core (Minimum Viable)
1. Transport (unix socket only first)
2. JSON-RPC (basic request/reply)
3. Schema parsing
4. Simple monitor (no conditions)
5. Basic transactions

**Milestone**: Can read bridges/ports/interfaces

### Phase 2: OVSDB Production-Ready
1. TCP + TLS transport
2. Reconnection FSM
3. Conditional monitoring
4. Full transaction retry
5. Cluster awareness

**Milestone**: Production-grade OVSDB client

### Phase 3: OpenFlow Minimal
1. OF 1.3 message encoding/decoding
2. Basic match fields (L2/L3/L4)
3. Basic actions (output, drop, set_field)
4. Flow add/mod/del

**Milestone**: Can program basic flows

### Phase 4: OpenFlow Extended
1. Nicira extensions
2. Bundles
3. Groups and meters
4. Full match field coverage

**Milestone**: Feature parity with ovs-ofctl

---

## Testing Strategy

| Test Type | Tool | Coverage |
|-----------|------|----------|
| Unit | cargo test | Protocol encoding, parsing |
| Integration | Docker + OVS | OVSDB ops, flow programming |
| Compatibility | ovs-vsctl/ofctl | Compare output |
| Fuzz | cargo-fuzz | Protocol parsers |

---

## Decisions Made

1. **License**: MIT/Apache 2.0 dual license
2. **MSRV**: Latest stable (no fixed minimum, edition 2024)
3. **Feature flags**: No - OpenFlow included from the start
4. **Async runtime**: Tokio-only (de facto standard, no abstraction overhead)
5. **Name**: `rovs`
