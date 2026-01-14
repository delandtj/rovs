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

### Phase 1: OXM Encoding

Foundation - encode individual match fields to wire format.

#### 1.1 OXM Header Builder
- [ ] `oxm.rs`: Add `OxmHeader` struct with class, field, hasmask, length
- [ ] `OxmHeader::encode(&self) -> [u8; 4]` - write 4-byte header
- [ ] Unit test: verify header bytes for known fields

#### 1.2 Fixed-Size Field Encoding
- [ ] `encode_u8(class, field, value) -> Vec<u8>` (1 byte: IpProto, IpDscp)
- [ ] `encode_u16(class, field, value) -> Vec<u8>` (2 bytes: EthType, TcpSrc, TcpDst, UdpSrc, UdpDst, VlanVid)
- [ ] `encode_u32(class, field, value) -> Vec<u8>` (4 bytes: InPort, Ipv4Src, Ipv4Dst)
- [ ] `encode_u64(class, field, value) -> Vec<u8>` (8 bytes: Metadata, TunnelId)
- [ ] `encode_mac(class, field, value) -> Vec<u8>` (6 bytes: EthSrc, EthDst)
- [ ] Unit tests for each size

#### 1.3 Masked Field Encoding
- [ ] `encode_u32_masked(class, field, value, mask) -> Vec<u8>` (8 bytes: Ipv4 with prefix)
- [ ] `encode_mac_masked(class, field, value, mask) -> Vec<u8>` (12 bytes)
- [ ] Helper: `prefix_to_mask(prefix_len: u8) -> u32` for IPv4
- [ ] Unit tests: `/24` prefix → `0xffffff00` mask

#### 1.4 OxmField Trait
- [ ] Create `trait OxmEncode` with `fn encode(&self) -> Vec<u8>`
- [ ] Implement for each field type in `Match` struct
- [ ] Unit tests comparing to Wireshark captures

### Phase 2: Action Encoding

Encode individual actions to wire format.

#### 2.1 Action Type Constants
- [ ] `action.rs`: Add `ActionType` enum with wire values
- [ ] Output=0, SetField=25, PushVlan=17, PopVlan=18, Group=22, etc.

#### 2.2 Simple Actions (fixed size)
- [ ] `Action::Output` → 16 bytes (type=0, len=16, port=u32, max_len=u16, pad=6)
- [ ] `Action::PopVlan` → 8 bytes (type=18, len=8, pad=4)
- [ ] `Action::DecTtl` → 8 bytes (type=24, len=8, pad=4)
- [ ] `Action::Group` → 8 bytes (type=22, len=8, group_id=u32)
- [ ] Unit tests for each

#### 2.3 PushVlan Action
- [ ] `Action::PushVlan(ethertype)` → 8 bytes (type=17, len=8, ethertype=u16, pad=2)
- [ ] Unit test

#### 2.4 SetField Action (variable size)
- [ ] `Action::SetEthSrc/Dst` → 16 bytes (type=25, len=16, oxm_header+mac+pad)
- [ ] `Action::SetVlanVid` → 16 bytes
- [ ] `Action::SetIpv4Src/Dst` → 16 bytes
- [ ] `Action::SetTpSrc/Dst` → 16 bytes (TCP/UDP port)
- [ ] Uses OXM encoding from Phase 1
- [ ] Unit tests

#### 2.5 ActionList Encoding
- [ ] `ActionList::encode(&self) -> Vec<u8>` - concatenate all actions
- [ ] Ensure 8-byte alignment with padding
- [ ] Unit test with multiple actions

### Phase 3: Match Encoding

Encode complete match structure.

#### 3.1 Match Header (OF 1.3 OXM format)
- [ ] `match_fields.rs`: Add `MatchType` enum (Standard=0, OXM=1)
- [ ] Match header: type=1 (OXM), length=u16
- [ ] Padding to 8-byte boundary

#### 3.2 Field Prerequisite Ordering
- [ ] Define prerequisite map: `ipv4_src` requires `eth_type=0x0800`
- [ ] `tcp_src` requires `ip_proto=6`
- [ ] Auto-insert prerequisites if missing
- [ ] Or error if inconsistent

#### 3.3 Match::encode()
- [ ] Iterate `Match` fields in correct order
- [ ] Call OXM encode for each non-None field
- [ ] Build match header + OXM list + padding
- [ ] `fn encode(&self) -> Vec<u8>`
- [ ] Unit tests: empty match, single field, multiple fields

### Phase 4: Instruction Encoding

OF 1.3 instructions wrap actions.

#### 4.1 Instruction Types
- [ ] Create `instruction.rs`
- [ ] `InstructionType` enum: GotoTable=1, WriteMetadata=2, WriteActions=3, ApplyActions=4, Clear=5, Meter=6

#### 4.2 GotoTable Instruction
- [ ] 8 bytes: type=1, len=8, table_id=u8, pad=3
- [ ] Unit test

#### 4.3 WriteMetadata Instruction
- [ ] 24 bytes: type=2, len=24, pad=4, metadata=u64, mask=u64
- [ ] Unit test

#### 4.4 ApplyActions Instruction
- [ ] Variable: type=4, len=variable, pad=4, actions...
- [ ] Wraps `ActionList::encode()`
- [ ] Unit test

#### 4.5 WriteActions Instruction
- [ ] Same format as ApplyActions but type=3
- [ ] Unit test

#### 4.6 InstructionList Encoding
- [ ] `InstructionList::encode(&self) -> Vec<u8>`
- [ ] Concatenate instructions in order
- [ ] Unit test

### Phase 5: FlowMod Encoding

Complete FlowMod message.

#### 5.1 FlowMod Fixed Fields
- [ ] `flow.rs`: Add `FlowMod::encode_fixed(&self) -> [u8; 40]`
- [ ] cookie(8) + cookie_mask(8) + table_id(1) + command(1) + idle_timeout(2) + hard_timeout(2) + priority(2) + buffer_id(4) + out_port(4) + out_group(4) + flags(2) + pad(2)
- [ ] Unit test

#### 5.2 FlowMod::encode()
- [ ] Combine: fixed fields + match + instructions
- [ ] Calculate total length for OF header
- [ ] `fn encode(&self, version: Version) -> Vec<u8>`
- [ ] Unit test

#### 5.3 Complete Message with Header
- [ ] `FlowMod::to_message(&self, xid: u32) -> Message`
- [ ] Prepend 8-byte OF header
- [ ] Unit test: verify complete message bytes

### Phase 6: VConn Integration

Wire up encoding to connection.

#### 6.1 send_flow_mod()
- [ ] `vconn.rs`: Implement `send_flow_mod()` using `FlowMod::to_message()`
- [ ] Remove `todo!()`

#### 6.2 Error Handling
- [ ] Parse OF error reply (type=1)
- [ ] Map error codes to `Error::OfError`
- [ ] Return meaningful error on flow add failure

#### 6.3 Barrier After FlowMod
- [ ] `send_flow_mod_sync()` - send flowmod + barrier, wait for barrier reply
- [ ] Ensures flow is installed before returning

### Phase 7: Integration Tests

Test against real OVS.

#### 7.1 Container Setup
- [ ] Update `Containerfile` for privileged mode with vswitchd
- [ ] Add `scripts/test-with-ovs.sh openflow` mode
- [ ] Create bridge in container for flow tests

#### 7.2 Add Flow Test
- [ ] Connect to OVS OpenFlow port (6653)
- [ ] Add simple flow: `in_port=1 actions=output:2`
- [ ] Verify with `ovs-ofctl dump-flows`
- [ ] `#[ignore = "requires ovs-vswitchd"]`

#### 7.3 Delete Flow Test
- [ ] Add flow, then delete with `FlowModCommand::Delete`
- [ ] Verify flow removed

#### 7.4 Flow with Match Test
- [ ] Add flow: `ip,nw_dst=10.0.0.0/24 actions=output:1`
- [ ] Verify match fields correct in dump

### Phase 8: Flow Dump (Decode)

Read flows back from switch.

#### 8.1 Multipart Request
- [ ] `MultipartRequest::FlowStats` encoding
- [ ] Send request, receive reply

#### 8.2 OXM Decoding
- [ ] `OxmHeader::decode(bytes) -> (OxmHeader, &[u8])`
- [ ] Decode value bytes based on field type
- [ ] Build `Match` struct from OXM list

#### 8.3 Action Decoding
- [ ] `Action::decode(bytes) -> (Action, usize)`
- [ ] Handle each action type
- [ ] Build `ActionList`

#### 8.4 FlowStats Parsing
- [ ] Parse flow stats reply body
- [ ] Return `Vec<Flow>` with match, actions, counters

### Phase 9: Nicira Extensions (Future)

For advanced OVS features.

#### 9.1 Vendor Action Header
- [ ] Experimenter action type (0xffff)
- [ ] Nicira vendor ID: 0x00002320

#### 9.2 NxResubmit
- [ ] Subtype 14, encode port + table

#### 9.3 NxCt (Connection Tracking)
- [ ] Subtype 35, encode flags + zone + table + actions

#### 9.4 NxLearn
- [ ] Subtype 16, complex flow_mod_specs encoding

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
