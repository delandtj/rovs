# P4 Network Programming Overview

An introduction to P4 and its relationship to OVS for the rovs project.

---

## What is P4?

**P4** (Programming Protocol-independent Packet Processors) is a domain-specific language for programming how network switches and routers process packets.

### Traditional vs P4 Switches

| Traditional Switches | P4-Programmable Switches |
|---------------------|-------------------------|
| Fixed behavior hardcoded by vendor | You define parsing and processing |
| Only predefined protocols (Ethernet, IP, TCP) | Custom/new protocols supported |
| Configure, but can't change processing | "Blank slate" you program |

### What P4 Lets You Define

```
┌─────────────────────────────────────────┐
│  1. HEADERS    - What packet fields     │
│                  to recognize           │
│  2. PARSER     - How to extract headers │
│  3. TABLES     - What to match on       │
│  4. ACTIONS    - What to do (forward,   │
│                  drop, modify, etc.)    │
│  5. PIPELINE   - Order of processing    │
└─────────────────────────────────────────┘
```

---

## Relationship to OVS

P4 and OVS can work together in several ways:

```
┌─────────────────────────────────────────────────────────┐
│                    Control Plane                         │
│  (SDN Controller, rovs, ovs-ofctl, P4Runtime)           │
├─────────────────────────────────────────────────────────┤
│                    Data Plane                            │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐ │
│  │    OVS      │    │ P4 Software │    │ P4 Hardware │ │
│  │ (OpenFlow)  │    │   Switch    │    │   Switch    │ │
│  │             │    │   (BMv2)    │    │  (Barefoot) │ │
│  └─────────────┘    └─────────────┘    └─────────────┘ │
└─────────────────────────────────────────────────────────┘
```

**Key integration points**:
- **p4c-behavioral** (companion repo) compiles P4 to behavioral models
- OpenFlow plugin generates OVS-compatible code
- SAI plugin for hardware abstraction
- Both use similar control plane concepts (tables, matches, actions)

---

## Simple P4 Example: L2 Switch

```p4
/* Simple L2 Switch */

// Define the Ethernet header structure
header ethernet_t {
    bit<48> dstAddr;
    bit<48> srcAddr;
    bit<16> etherType;
}

// Packet headers we'll parse
struct headers {
    ethernet_t ethernet;
}

// Parser: extract Ethernet header from incoming packet
parser MyParser(packet_in packet, out headers hdr) {
    state start {
        packet.extract(hdr.ethernet);
        transition accept;
    }
}

// Control block: packet processing logic
control MyIngress(inout headers hdr,
                  inout standard_metadata_t standard_metadata) {

    // Action: forward to a specific port
    action forward(bit<9> port) {
        standard_metadata.egress_spec = port;
    }

    // Action: drop the packet
    action drop() {
        mark_to_drop(standard_metadata);
    }

    // Table: match on destination MAC, decide what to do
    table dmac {
        key = {
            hdr.ethernet.dstAddr: exact;
        }
        actions = {
            forward;
            drop;
        }
        default_action = drop();
    }

    apply {
        dmac.apply();  // Apply the table lookup
    }
}

// Deparser: reassemble packet for output
control MyDeparser(packet_out packet, in headers hdr) {
    apply {
        packet.emit(hdr.ethernet);
    }
}
```

### Processing Flow

```
Packet arrives
     ↓
Parser extracts Ethernet header (dst MAC, src MAC, etherType)
     ↓
Table lookup: "Which port for this dst MAC?"
     ↓
Action: forward(port) or drop()
     ↓
Deparser reassembles packet → sent out
```

---

## Learning Switch Architecture

A learning switch dynamically learns MAC addresses and their associated ports. In P4, this requires cooperation between data plane and control plane.

### Data Plane (P4)

```p4
control MyIngress(inout headers hdr,
                  inout standard_metadata_t std_meta) {

    // Table for source MAC learning (triggers digest)
    table smac {
        key = { hdr.ethernet.srcAddr: exact; }
        actions = { NoAction; }
        default_action = NoAction();
    }

    // Table for destination MAC forwarding
    table dmac {
        key = { hdr.ethernet.dstAddr: exact; }
        actions = { forward; broadcast; }
        default_action = broadcast();
    }

    apply {
        // Learn: if src MAC unknown, send digest to controller
        if (!smac.apply().hit) {
            digest(1, { hdr.ethernet.srcAddr, std_meta.ingress_port });
        }

        // Forward: lookup dst MAC or broadcast
        dmac.apply();
    }
}
```

### Control Plane Flow

```
┌─────────────────────────────────────────────────────────┐
│                    Controller                            │
│  1. Receive digest (MAC, port)                          │
│  2. Add entry to smac table (for learning)              │
│  3. Add entry to dmac table (for forwarding)            │
└─────────────────────────────────────────────────────────┘
                           ↑ digest
                           ↓ table updates
┌─────────────────────────────────────────────────────────┐
│                    P4 Switch                             │
│  - smac table: tracks known source MACs                 │
│  - dmac table: forwards to learned ports                │
└─────────────────────────────────────────────────────────┘
```

---

## P4 Resources

### Official Examples

| Resource | Description | URL |
|----------|-------------|-----|
| **p4lang/p4-spec** | Official PSA learning switch example | [psa-example-digest.p4](https://github.com/p4lang/p4-spec/blob/main/p4-16/psa/examples/psa-example-digest.p4) |
| **p4lang/tutorials** | Official P4 tutorials | [github.com/p4lang/tutorials](https://github.com/p4lang/tutorials) |
| **p4lang/switch** | Complete switch implementation | [github.com/p4lang/switch](https://github.com/p4lang/switch) |

### Learning Resources

| Resource | Description | URL |
|----------|-------------|-----|
| **nsg-ethz/p4-learning** | ETH Zurich exercises and examples | [github.com/nsg-ethz/p4-learning](https://github.com/nsg-ethz/p4-learning) |
| **p4c-behavioral** | P4 compiler for behavioral model | [../p4c-behavioral](../p4c-behavioral) |

### Key Concepts to Learn

1. **Headers and Parsing** - Defining packet structure
2. **Match-Action Tables** - Core processing primitive
3. **Actions and Primitives** - What operations are available
4. **Digests** - Communicating with control plane
5. **Stateful Processing** - Registers, counters, meters
6. **Architectures** - V1Model, PSA, TNA

---

## P4 vs OpenFlow Comparison

| Aspect | P4 | OpenFlow |
|--------|----|---------|
| **Level** | Defines switch behavior | Controls pre-defined switch |
| **Flexibility** | Full programmability | Fixed match/action capabilities |
| **Headers** | Custom defined | Predefined (Ethernet, IP, etc.) |
| **Tables** | Custom defined | Fixed pipeline (OF 1.0) or configurable (OF 1.3+) |
| **Runtime** | P4Runtime API | OpenFlow protocol |
| **Targets** | BMv2, Tofino, FPGA | OVS, hardware switches |

### Complementary Usage

```
P4 Program (defines capabilities)
     ↓ compile
Switch with P4-defined tables
     ↓ control via
P4Runtime or OpenFlow (populates tables)
```

---

## Relevance to rovs

While rovs focuses on OpenFlow/OVSDB for OVS control, understanding P4 provides:

1. **Conceptual foundation** - Match-action tables, pipelines
2. **Future compatibility** - P4Runtime support possible
3. **Testing** - BMv2 can be controlled via OpenFlow
4. **Architecture insight** - How programmable switches work

The p4c-behavioral compiler in the adjacent repository can generate OpenFlow-compatible code from P4 programs, bridging both worlds.
