# OpenFlow Implementation Roadmap

This document tracks the implementation status and roadmap for OpenFlow support in rovs.

## Current State (~1100 lines)

| Component | File | Status | Notes |
|-----------|------|--------|-------|
| VConn | `vconn.rs` | Partial | Connect, handshake, echo, barrier work |
| Message header | `message.rs` | Done | Encode/decode for 8-byte header |
| Message types | `message.rs` | Done | All OF 1.0-1.5 types defined |
| OXM identifiers | `oxm.rs` | Done | Field IDs defined, header builder |
| Match builder | `match_fields.rs` | Done | L2/L3/L4 fields with builder pattern |
| Action types | `action.rs` | Done | 20+ actions including Nicira extensions |
| FlowMod struct | `flow.rs` | Done | Builder pattern for add/delete |

### What's Missing

- **Wire encoding**: None of the structs can serialize to OpenFlow wire format
- **FlowMod send**: `vconn.send_flow_mod()` has `todo!()` at `vconn.rs:97`
- **Multipart**: Flow dump, port stats, table features not implemented
- **Error parsing**: Can receive errors but no structured decoding

## Implementation Phases

### Phase 1: Wire Primitives

Foundation for all encoding. Each step is independently testable.

1. **OXM TLV encoding** (`oxm.rs`)
   - `OxmField::encode(&self, value: &[u8], mask: Option<&[u8]>) -> Vec<u8>`
   - Unit tests with known byte patterns from Wireshark captures
   - Fields: InPort, EthDst, EthSrc, EthType, VlanVid, Ipv4Src, Ipv4Dst, IpProto, TcpSrc, TcpDst

2. **Action encoding** (`action.rs`)
   - `Action::encode(&self, version: Version) -> Vec<u8>`
   - Start with: Output, SetField, PushVlan, PopVlan, GotoTable
   - OF 1.3 action format (type, length, payload)

3. **Unit tests**
   - Compare encoded bytes against known-good captures
   - Test round-trip encode/decode where applicable

### Phase 2: Match & FlowMod Encoding

Building on Phase 1 primitives.

4. **Match encoding** (`match_fields.rs`)
   - `Match::encode(&self) -> Vec<u8>`
   - Iterate fields, call OXM encode, build OXM list
   - Handle prerequisite fields (eth_type before ipv4_src, etc.)

5. **Instruction encoding** (new `instruction.rs`)
   - OF 1.3 instructions: ApplyActions, WriteActions, GotoTable, WriteMetadata
   - `ActionList::to_instructions(&self) -> Vec<u8>`

6. **FlowMod encoding** (`flow.rs`)
   - `FlowMod::encode(&self, version: Version) -> Vec<u8>`
   - Complete message: header + flowmod fields + match + instructions

### Phase 3: Integration Testing

7. **VConn integration**
   - Implement `send_flow_mod()` using Phase 2 encoding
   - Add timeout and error handling

8. **Test against OVS**
   - Container with ovs-vswitchd (privileged mode)
   - Add flow via rovs, verify with `ovs-ofctl dump-flows`
   - Delete flow, verify removed

9. **Flow dump** (multipart)
   - `MultipartRequest::FlowStats`
   - Parse `MultipartReply` with flow entries
   - Decode match fields and actions back to structs

### Phase 4: Advanced Features

10. **Nicira extensions**
    - `NxResubmit`, `NxCt`, `NxLearn`
    - Vendor action encoding (experimenter type)

11. **Group tables**
    - `GroupMod` message
    - Bucket encoding with actions

12. **Meters**
    - `MeterMod` message
    - Band encoding

13. **Table features**
    - Query switch capabilities
    - Match/action support detection

## Testing Strategy

### Unit Tests
- Encode known values, compare to expected bytes
- Example: `Match::new().eth_type(0x0800).ipv4_dst("10.0.0.1".parse(), 24)`
  should produce specific OXM bytes

### Integration Tests
- Require OVS container with vswitchd (privileged)
- Add to `scripts/test-with-ovs.sh full` mode
- Mark with `#[ignore = "requires ovs-vswitchd"]`

### Reference Materials
- OpenFlow 1.3.5 spec: https://opennetworking.org/wp-content/uploads/2014/10/openflow-spec-v1.3.5.pdf
- OVS source: `lib/ofp-actions.c`, `lib/nx-match.c`
- Wireshark captures of ovs-ofctl commands

## Wire Format Quick Reference

### OXM TLV (4-byte header + value + optional mask)
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|           class             |field|M|        length           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        value (variable)                       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### FlowMod (OF 1.3)
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                            cookie                             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         cookie_mask                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   table_id  |    command    |         idle_timeout            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         hard_timeout        |           priority              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          buffer_id                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          out_port                             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          out_group                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|            flags            |           pad (zeros)           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       match (variable)                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                   instructions (variable)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### Action (OF 1.3)
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|            type             |            length               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        payload (varies)                       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```
