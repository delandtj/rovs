# OpenFlow Implementation Planning

## Overview

This document outlines the plan for implementing OpenFlow support in rovs. OpenFlow is the protocol used to program flow tables in OVS switches.

## OpenFlow in OVS Context

```
┌─────────────────────────────────────────────────────────────────┐
│                        Controller                               │
│  (rovs-openflow)                                                │
└─────────────────────────────────────────────────────────────────┘
         │ OpenFlow (TCP/TLS, port 6653)
         ▼
┌─────────────────────────────────────────────────────────────────┐
│                      OVS Bridge                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ Flow Tables                                               │  │
│  │ ┌─────────┐ ┌─────────┐ ┌─────────┐                      │  │
│  │ │ Table 0 │→│ Table 1 │→│ Table N │→ [output/drop]       │  │
│  │ └─────────┘ └─────────┘ └─────────┘                      │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ OVSDB Configuration (rovs-ovsdb)                          │  │
│  │ - Bridge.controller = "tcp:127.0.0.1:6653"               │  │
│  │ - Bridge.fail_mode = "secure"                            │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Protocol Versions

| Version | Year | Key Features |
|---------|------|--------------|
| 1.0 | 2009 | Basic flow matching, single table |
| 1.1 | 2011 | Multiple tables, groups, MPLS |
| 1.2 | 2011 | Extensible match (OXM) |
| 1.3 | 2012 | Meters, per-flow counters (most common) |
| 1.4 | 2013 | Optical ports, bundles |
| 1.5 | 2014 | Egress tables, copy-field |

**Target: OpenFlow 1.3** (most widely deployed, OVS default)

## Implementation Plan

### Phase 1: Protocol Foundation

**1.1 Message Encoding/Decoding**
- OpenFlow header (version, type, length, xid)
- Hello message exchange
- Feature request/reply
- Error messages

**1.2 Connection Management**
- TCP connection handling
- TLS support (reuse rovs-transport)
- Connection state machine
- Echo request/reply keepalive

**1.3 Basic Message Types**
```rust
pub enum OfpMessage {
    Hello(Hello),
    Error(Error),
    EchoRequest(Echo),
    EchoReply(Echo),
    FeaturesRequest,
    FeaturesReply(FeaturesReply),
    // ... more
}
```

### Phase 2: Flow Programming

**2.1 Match Fields (OXM - OpenFlow Extensible Match)**
```rust
pub enum OxmField {
    InPort(u32),
    EthDst([u8; 6]),
    EthSrc([u8; 6]),
    EthType(u16),
    VlanVid(u16),
    VlanPcp(u8),
    IpDscp(u8),
    IpProto(u8),
    Ipv4Src(Ipv4Addr),
    Ipv4Dst(Ipv4Addr),
    TcpSrc(u16),
    TcpDst(u16),
    UdpSrc(u16),
    UdpDst(u16),
    // ... with optional masks
}
```

**2.2 Actions**
```rust
pub enum OfpAction {
    Output { port: u32, max_len: u16 },
    SetField(OxmField),
    PushVlan { ethertype: u16 },
    PopVlan,
    PushMpls { ethertype: u16 },
    PopMpls { ethertype: u16 },
    SetQueue { queue_id: u32 },
    Group { group_id: u32 },
    SetNwTtl { ttl: u8 },
    DecNwTtl,
    // ... more
}
```

**2.3 Instructions**
```rust
pub enum OfpInstruction {
    GotoTable { table_id: u8 },
    WriteMetadata { metadata: u64, mask: u64 },
    WriteActions(Vec<OfpAction>),
    ApplyActions(Vec<OfpAction>),
    ClearActions,
    Meter { meter_id: u32 },
}
```

**2.4 Flow Mod**
```rust
pub struct FlowMod {
    pub cookie: u64,
    pub cookie_mask: u64,
    pub table_id: u8,
    pub command: FlowModCommand,
    pub idle_timeout: u16,
    pub hard_timeout: u16,
    pub priority: u16,
    pub buffer_id: u32,
    pub out_port: u32,
    pub out_group: u32,
    pub flags: FlowModFlags,
    pub match_fields: Vec<OxmField>,
    pub instructions: Vec<OfpInstruction>,
}
```

### Phase 3: Controller Framework

**3.1 Switch Connection**
```rust
pub struct Switch {
    conn: OfConnection,
    datapath_id: u64,
    n_tables: u8,
    capabilities: u32,
    ports: HashMap<u32, Port>,
}
```

**3.2 Event-Driven API**
```rust
pub trait Controller {
    async fn on_switch_connected(&mut self, switch: &mut Switch);
    async fn on_switch_disconnected(&mut self, dpid: u64);
    async fn on_packet_in(&mut self, switch: &mut Switch, pkt: PacketIn);
    async fn on_flow_removed(&mut self, switch: &mut Switch, flow: FlowRemoved);
    async fn on_port_status(&mut self, switch: &mut Switch, port: PortStatus);
}
```

**3.3 Controller Server**
```rust
pub struct ControllerServer {
    listener: TcpListener,
    switches: HashMap<u64, Switch>,
}

impl ControllerServer {
    pub async fn run<C: Controller>(&mut self, controller: C);
}
```

### Phase 4: High-Level API

**4.1 Flow Builder**
```rust
let flow = Flow::builder()
    .table(0)
    .priority(100)
    .match_eth_type(0x0800)         // IPv4
    .match_ipv4_dst("10.0.0.0/8")
    .action_output(2)
    .build();

switch.add_flow(flow).await?;
```

**4.2 Table Pipeline**
```rust
// L2 learning switch
let pipeline = Pipeline::new()
    .table(0, "classifier")
    .table(1, "mac_learning")
    .table(2, "forwarding");
```

**4.3 Group Tables**
```rust
// Load balancing group
let group = Group::builder()
    .group_id(1)
    .group_type(GroupType::Select)
    .bucket(Bucket::new().action_output(1).weight(50))
    .bucket(Bucket::new().action_output(2).weight(50))
    .build();
```

### Phase 5: Integration with OVSDB

**5.1 Bridge Controller Configuration**
```rust
// Via OVSDB transaction
txn.update("Bridge", bridge_uuid, json!({
    "controller": ["set", [["uuid", controller_uuid]]],
    "fail_mode": "secure"
}));

// Or via helper
client.set_controller("br0", "tcp:127.0.0.1:6653").await?;
```

**5.2 Port Number Mapping**
```rust
// OVSDB Interface.ofport -> OpenFlow port number
let ofport = client.get_interface_ofport("eth0")?;
```

## Existing Code Review

### Current `rovs-openflow/src/`

**`lib.rs`** - Module structure (empty)

**`oxm.rs`** - OXM field definitions (partial)
```rust
pub enum OxmClass {
    Nxm0 = 0x0000,
    Nxm1 = 0x0001,
    OpenflowBasic = 0x8000,
}

pub enum OxmField {
    InPort = 0,
    InPhyPort = 1,
    Metadata = 2,
    // ... defined but not yet used
}

pub fn oxm_header(class, field, has_mask, length) -> u32
```

**Status:** Basic constants defined, need serialization/deserialization.

## Binary Protocol Details

### OpenFlow Header (8 bytes)
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    version    |     type      |            length             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                              xid                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### OXM TLV Format (4+ bytes)
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|           class               |field|H|  length |    value...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### Flow Mod Structure
```
Header (8) + Cookie (8) + Cookie Mask (8) + Table ID (1) + Command (1) +
Idle Timeout (2) + Hard Timeout (2) + Priority (2) + Buffer ID (4) +
Out Port (4) + Out Group (4) + Flags (2) + Pad (2) + Match + Instructions
```

## Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["net", "io-util", "sync"] }
bytes = "1"
thiserror = "2"
tracing = "0.1"

# For binary parsing
byteorder = "1"  # or use bytes directly

# Reuse from workspace
rovs-transport = { path = "../rovs-transport" }
```

## Testing Strategy

### Unit Tests
- Message encoding/decoding round-trips
- OXM field serialization
- Match field combinations

### Integration Tests
- Connect to OVS (mininet or user-space)
- Add/delete flows
- Verify flow stats

### Test Environment
```bash
# Start OVS with OpenFlow
ovs-vsctl add-br br0
ovs-vsctl set-controller br0 tcp:127.0.0.1:6653
ovs-vsctl set bridge br0 protocols=OpenFlow13

# Or use mininet
sudo mn --topo single,3 --controller remote
```

## Reference Implementation

Study these for implementation details:
- **OVS source:** `lib/ofp-*.c`, `lib/ox-*.c`
- **Python Ryu:** `ryu/ofproto/`
- **Rust openflow.rs:** (if exists, limited)

## Priority Order

1. **Message framing** - Read/write OpenFlow messages
2. **Hello/Features** - Handshake with switch
3. **Flow Mod** - Add flows (most important operation)
4. **Packet In/Out** - Reactive flow programming
5. **Port Status** - Track port changes
6. **Groups/Meters** - Advanced features
7. **Controller framework** - Multi-switch support

## API Design Goals

1. **Type-safe** - Catch errors at compile time
2. **Async** - Non-blocking I/O with tokio
3. **Builder pattern** - Ergonomic flow construction
4. **Extensible** - Support vendor extensions
5. **Integration** - Work seamlessly with rovs-ovsdb

## Example: Learning Switch

```rust
use rovs_openflow::{Controller, Switch, PacketIn, Flow};

struct LearningSwitch {
    mac_to_port: HashMap<[u8; 6], u32>,
}

impl Controller for LearningSwitch {
    async fn on_packet_in(&mut self, switch: &mut Switch, pkt: PacketIn) {
        let eth = pkt.parse_ethernet()?;
        let in_port = pkt.in_port;

        // Learn source MAC
        self.mac_to_port.insert(eth.src, in_port);

        // Lookup destination
        if let Some(&out_port) = self.mac_to_port.get(&eth.dst) {
            // Install flow
            let flow = Flow::builder()
                .table(0)
                .priority(100)
                .match_eth_dst(eth.dst)
                .action_output(out_port)
                .idle_timeout(300)
                .build();

            switch.add_flow(flow).await?;
            switch.packet_out(pkt.buffer_id, out_port).await?;
        } else {
            // Flood
            switch.packet_out(pkt.buffer_id, Port::FLOOD).await?;
        }
    }
}
```

## File Structure Plan

```
rovs-openflow/
├── src/
│   ├── lib.rs              # Public API
│   ├── error.rs            # Error types
│   ├── protocol/
│   │   ├── mod.rs
│   │   ├── header.rs       # Message header
│   │   ├── message.rs      # Message types enum
│   │   ├── hello.rs        # Hello message
│   │   ├── features.rs     # Features request/reply
│   │   ├── error.rs        # Error message
│   │   ├── flow_mod.rs     # Flow modification
│   │   ├── packet.rs       # Packet in/out
│   │   ├── port.rs         # Port status
│   │   ├── group.rs        # Group mod
│   │   └── meter.rs        # Meter mod
│   ├── oxm/
│   │   ├── mod.rs
│   │   ├── fields.rs       # OXM field types
│   │   └── codec.rs        # Serialization
│   ├── action.rs           # Action types
│   ├── instruction.rs      # Instruction types
│   ├── connection.rs       # OpenFlow connection
│   ├── switch.rs           # Switch abstraction
│   ├── controller.rs       # Controller trait
│   ├── server.rs           # Controller server
│   └── builder/
│       ├── mod.rs
│       ├── flow.rs         # Flow builder
│       └── match.rs        # Match builder
├── examples/
│   ├── learning_switch.rs
│   ├── hub.rs
│   └── firewall.rs
└── Cargo.toml
```
