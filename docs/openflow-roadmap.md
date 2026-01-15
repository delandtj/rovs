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
- [x] `oxm.rs`: Add `OxmHeader` struct with class, field, hasmask, length
- [x] `OxmHeader::encode(&self) -> [u8; 4]` - write 4-byte header
- [x] Unit test: verify header bytes for known fields

#### 1.2 Fixed-Size Field Encoding
- [x] `encode_u8(class, field, value) -> Vec<u8>` (1 byte: IpProto, IpDscp)
- [x] `encode_u16(class, field, value) -> Vec<u8>` (2 bytes: EthType, TcpSrc, TcpDst, UdpSrc, UdpDst, VlanVid)
- [x] `encode_u32(class, field, value) -> Vec<u8>` (4 bytes: InPort, Ipv4Src, Ipv4Dst)
- [x] `encode_u64(class, field, value) -> Vec<u8>` (8 bytes: Metadata, TunnelId)
- [x] `encode_mac(class, field, value) -> Vec<u8>` (6 bytes: EthSrc, EthDst)
- [x] Unit tests for each size

#### 1.3 Masked Field Encoding
- [x] `encode_u32_masked(class, field, value, mask) -> Vec<u8>` (8 bytes: Ipv4 with prefix)
- [x] `encode_mac_masked(class, field, value, mask) -> Vec<u8>` (12 bytes)
- [x] `encode_u64_masked(class, field, value, mask) -> Vec<u8>` (16 bytes: Metadata)
- [x] `encode_ipv4_prefix(field, addr, prefix_len)` convenience function
- [x] Helper: `prefix_to_mask(prefix_len: u8) -> u32` for IPv4
- [x] Unit tests: `/24` prefix → `0xffffff00` mask

#### 1.4 NXM Field Encoding (Nicira Extensions)

NXM uses same TLV format as OXM but with class 0x0000 (NXM_OF_*) or 0x0001 (NXM_NX_*).

- [x] Add `NxmField` enum with Nicira field IDs
- [x] Registers: `NXM_NX_REG0-15` (field 0-15, class 0x0001, 4 bytes each)
- [x] `encode_reg(reg_num: u8, value: u32) -> Vec<u8>`
- [x] `encode_reg_masked(reg_num: u8, value: u32, mask: u32) -> Vec<u8>` (for partial matches)
- [x] Tunnel: `NXM_NX_TUN_ID` (field 16, 8 bytes)
- [x] Tunnel endpoints: `NXM_NX_TUN_IPV4_SRC/DST` (fields 31/32, 4 bytes)
- [x] Connection tracking:
  - [x] `NXM_NX_CT_STATE` (field 105, 4 bytes, bitmask: +trk, +new, +est, etc.)
  - [x] `NXM_NX_CT_ZONE` (field 106, 2 bytes)
  - [x] `NXM_NX_CT_MARK` (field 107, 4 bytes)
  - [x] `NXM_NX_CT_LABEL` (field 108, 16 bytes)
- [x] Packet mark: `NXM_NX_PKT_MARK` (field 33, 4 bytes)
- [x] `ct_state` module with flag constants (TRK, NEW, EST, REL, etc.)
- [x] Unit tests for each field type (18 new tests)

#### 1.5 Extended Registers (xxreg)
- [x] `NXM_NX_XXREG0-3` (128-bit registers, fields 111-114)
- [x] `encode_xxreg(reg_num: u8, value: u128) -> Vec<u8>`
- [x] `encode_xxreg_masked(reg_num: u8, value: u128, mask: u128) -> Vec<u8>`
- [x] `encode_xxreg_ipv6` / `encode_xxreg_ipv6_masked` for IPv6 addresses
- [x] `prefix_to_mask_v6(prefix_len: u8) -> u128` helper for IPv6
- [x] Unit tests for xxreg encoding (10 new tests)

#### 1.6 OxmField Trait
- [ ] Create `trait OxmEncode` with `fn encode(&self) -> Vec<u8>`
- [ ] Implement for both OXM (OpenFlow Basic) and NXM fields
- [ ] Unit tests comparing to Wireshark captures

### Phase 2: Action Encoding

Encode individual actions to wire format.

#### 2.1 Action Type Constants
- [x] `action.rs`: Add `ActionType` enum with wire values
- [x] Output=0, SetField=25, PushVlan=17, PopVlan=18, Group=22, etc.

#### 2.2 Simple Actions (fixed size)
- [x] `Action::Output` → 16 bytes (type=0, len=16, port=u32, max_len=u16, pad=6)
- [x] `Action::PopVlan` → 8 bytes (type=18, len=8, pad=4)
- [x] `Action::DecTtl` → 8 bytes (type=24, len=8, pad=4)
- [x] `Action::Group` → 8 bytes (type=22, len=8, group_id=u32)
- [x] Unit tests for each

#### 2.3 PushVlan Action
- [x] `Action::PushVlan(ethertype)` → 8 bytes (type=17, len=8, ethertype=u16, pad=2)
- [x] Unit test

#### 2.4 SetField Action (variable size)
- [x] `Action::SetEthSrc/Dst` → 16 bytes (type=25, len=16, oxm_header+mac+pad)
- [x] `Action::SetVlanVid` → 16 bytes
- [x] `Action::SetIpv4Src/Dst` → 16 bytes
- [x] `Action::SetTpSrc/Dst` → 16 bytes (TCP/UDP port)
- [x] Uses OXM encoding from Phase 1
- [x] Unit tests

#### 2.5 NXM Register Actions (Nicira Extensions)

Actions to load/move values into registers.

- [x] `NxLoad` - load immediate value into register/field
  - [x] `load:value->NXM_NX_REG0[0..31]` format
  - [x] Encode: vendor action (0xffff) + Nicira ID + subtype 7
  - [x] Fields: ofs_nbits (start bit, num bits), dst (NXM header), value
- [x] `NxMove` - copy bits between fields
  - [x] `move:NXM_OF_ETH_SRC->NXM_NX_REG0` format
  - [x] Subtype 6
  - [x] Fields: n_bits, src_ofs, dst_ofs, src, dst
- [x] `NxSetField` - Nicira version of SetField for NXM fields
  - [x] Subtype 33 (reg_load2) for setting tunnel ID and other NXM fields
- [x] Unit tests for each

#### 2.6 ActionList Encoding
- [x] `ActionList::encode(&self) -> Vec<u8>` - concatenate all actions
- [x] Ensure 8-byte alignment with padding
- [x] Unit test with multiple actions

### Phase 3: Match Encoding

Encode complete match structure.

#### 3.1 Match Header (OF 1.3 OXM format)
- [x] `match_fields.rs`: Add `MatchType` enum (Standard=0, OXM=1)
- [x] Match header: type=1 (OXM), length=u16
- [x] Padding to 8-byte boundary

#### 3.2 Field Prerequisite Ordering
- [x] Define prerequisite map: `ipv4_src` requires `eth_type=0x0800`
- [x] `tcp_src` requires `ip_proto=6`
- [x] Auto-insert prerequisites if missing (handled by builder methods)
- [x] Encoding order follows OpenFlow field ID order

#### 3.3 Match::encode()
- [x] Iterate `Match` fields in correct order
- [x] Call OXM encode for each non-None field
- [x] Build match header + OXM list + padding
- [x] `fn encode(&self) -> Vec<u8>`
- [x] Unit tests: empty match, single field, multiple fields (11 tests)

### Phase 4: Instruction Encoding

OF 1.3 instructions wrap actions.

#### 4.1 Instruction Types
- [x] Create `instruction.rs`
- [x] `InstructionType` enum: GotoTable=1, WriteMetadata=2, WriteActions=3, ApplyActions=4, Clear=5, Meter=6

#### 4.2 GotoTable Instruction
- [x] 8 bytes: type=1, len=8, table_id=u8, pad=3
- [x] Unit test

#### 4.3 WriteMetadata Instruction
- [x] 24 bytes: type=2, len=24, pad=4, metadata=u64, mask=u64
- [x] Unit test

#### 4.4 ApplyActions Instruction
- [x] Variable: type=4, len=variable, pad=4, actions...
- [x] Wraps `ActionList::encode()`
- [x] Unit test

#### 4.5 WriteActions Instruction
- [x] Same format as ApplyActions but type=3
- [x] Unit test

#### 4.6 InstructionList Encoding
- [x] `InstructionList::encode(&self) -> Vec<u8>`
- [x] Concatenate instructions in order
- [x] Unit test (17 tests total)

### Phase 5: FlowMod Encoding

Complete FlowMod message.

#### 5.1 FlowMod Fixed Fields
- [x] `flow.rs`: Add `Flow::encode_fixed(&self) -> [u8; 40]`
- [x] cookie(8) + cookie_mask(8) + table_id(1) + command(1) + idle_timeout(2) + hard_timeout(2) + priority(2) + buffer_id(4) + out_port(4) + out_group(4) + flags(2) + pad(2)
- [x] Unit tests (2 tests)

#### 5.2 Flow::encode()
- [x] Combine: fixed fields + match + instructions
- [x] Calculate total length for OF header
- [x] `fn encode(&self) -> Vec<u8>`
- [x] Unit tests (3 tests)

#### 5.3 Complete Message with Header
- [x] `Flow::to_message(&self, version, xid) -> Message`
- [x] Prepend 8-byte OF header
- [x] Unit tests: verify complete message bytes (2 tests)

### Phase 6: VConn Integration

Wire up encoding to connection.

#### 6.1 send_flow()
- [x] `vconn.rs`: Implement `send_flow()` using `Flow::to_message()`
- [x] Remove `todo!()`

#### 6.2 Error Handling
- [x] Parse OF error reply (type=1) via `OfError::parse()`
- [x] Map error codes to `Error::OfError` with structured error types
- [x] Added all OF 1.3 error type enums with human-readable Display
- [x] Return meaningful error on flow add failure
- [x] Unit tests for error parsing (7 tests)

#### 6.3 Barrier After FlowMod
- [x] `send_flow_sync()` - send flowmod + barrier, wait for barrier reply
- [x] Ensures flow is installed before returning
- [x] Handles echo requests while waiting (keep-alive)

### Phase 7: Integration Tests

Test against real OVS.

#### 7.1 Container Setup
- [x] Update `Containerfile` to expose OpenFlow port 6653
- [x] Update `scripts/ovsdb-entrypoint.sh` to create br-test bridge for OF testing
- [x] Add `scripts/test-with-ovs.sh openflow` mode
- [x] Create bridge in container with test-port1, test-port2

#### 7.2 Add Flow Test
- [x] Connect to OVS OpenFlow port (6653)
- [x] Add simple flow: `in_port=1 actions=output:2`
- [x] `#[ignore = "requires ovs-vswitchd"]`
- [x] Test cleanup with delete flow

#### 7.3 Delete Flow Test
- [x] Add flow, then delete with `Flow::delete()`
- [x] Delete by match test
- [x] Delete all flows in table test

#### 7.4 Flow with Match Tests
- [x] Add flow with IP match: `ip,nw_dst=10.0.0.0/24`
- [x] Add flow with TCP match: `tcp,tp_dst=80`
- [x] Add flow with VLAN match and pop_vlan action
- [x] Add flow with set_eth_dst action
- [x] Add flow with dec_ttl action
- [x] Add flow with timeout (idle/hard)
- [x] Add flow to specific table
- [x] Multiple sequential flows test

Total: 14 integration tests

### Phase 8: Flow Dump (Decode)

Read flows back from switch.

#### 8.1 Multipart Request
- [x] `MultipartRequest::FlowStats` encoding
- [x] `FlowStatsRequest` builder with table/cookie/match filters
- [x] Send request, receive reply via `VConn::dump_flows()`
- [x] Handle multipart MORE flag for multiple replies

#### 8.2 OXM Decoding
- [x] `Match::decode(bytes) -> (Match, usize)` decode match from wire format
- [x] Decode OXM header (class, field, hasmask, length)
- [x] Decode value bytes based on field type
- [x] Build `Match` struct from OXM list
- [x] Handle masked fields (IPv4, IPv6, MAC, metadata)
- [x] NXM field decoding (tunnel ID)
- [x] Unit tests for decode and roundtrip (12 new tests)

#### 8.3 Action Decoding
- [x] `Action::decode(bytes) -> (Action, usize)` decode single action
- [x] `ActionList::decode(bytes) -> ActionList` decode action list
- [x] Handle each action type (Output, PopVlan, PushVlan, DecTtl, SetTtl, Group, SetField)
- [x] Handle SetField OXM parsing (MAC, IPv4, VLAN, TCP/UDP ports, tunnel ID)
- [x] Handle Nicira experimenter actions (Resubmit, CT, RegLoad2)
- [x] `Instruction::decode(bytes)` and `InstructionList::decode(bytes)`
- [x] `FlowStatsEntry::decoded_instructions()` method
- [x] Unit tests for action decode (18 new tests)
- [x] Unit tests for instruction decode (10 new tests)

#### 8.4 FlowStats Parsing
- [x] `FlowStatsEntry` struct with all flow stats fields
- [x] `FlowStatsEntry::decode()` to parse single entry
- [x] `parse_flow_stats_reply()` to parse multipart body
- [x] Return `Vec<FlowStatsEntry>` with match, counters, durations
- [x] Unit tests for FlowStats decoding (7 tests)

### Phase 9: Nicira Extensions

For advanced OVS features.

#### 9.1 Vendor Action Header
- [x] Experimenter action type (0xffff)
- [x] Nicira vendor ID: 0x00002320

#### 9.2 NxResubmit
- [x] Subtype 14 (extended resubmit), encode port + table
- [x] `ActionList::resubmit()` and `resubmit_table()` builder methods
- [x] Wire encoding/decoding roundtrip tests

#### 9.3 NxCt (Connection Tracking)
- [x] Subtype 35, encode flags + zone + table
- [x] CT flags module (`ct_flags::COMMIT`, `ct_flags::FORCE`)
- [x] `ActionList::ct()` and `ct_commit()` builder methods
- [x] Wire encoding/decoding roundtrip tests

#### 9.4 NxLearn
- [x] Subtype 16, complete flow_mod_specs encoding
- [x] `NxLearn` struct with builder pattern
- [x] `LearnSpec` enum: MatchField, MatchImmediate, LoadField, LoadImmediate, OutputField
- [x] Learn flags module
- [x] Wire encoding with proper padding
- [x] Wire decoding for all spec types
- [x] `ActionList::learn()` builder method
- [x] Unit tests for all NxLearn functionality (11 new tests)

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
