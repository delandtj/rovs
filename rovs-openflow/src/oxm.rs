//! OpenFlow Extensible Match (OXM) encoding.
//!
//! OXM is used in OpenFlow 1.2+ for flexible match field encoding.

/// OXM class identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum OxmClass {
    /// Basic OpenFlow match fields
    OpenflowBasic = 0x8000,
    /// Experimenter match fields
    Experimenter = 0xffff,
    /// Nicira extensions
    Nxm0 = 0x0000,
    /// Nicira extensions (class 1)
    Nxm1 = 0x0001,
}

/// OXM field identifiers for OpenFlow Basic class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OxmField {
    /// Input port
    InPort = 0,
    /// Physical input port
    InPhyPort = 1,
    /// Metadata
    Metadata = 2,
    /// Ethernet destination
    EthDst = 3,
    /// Ethernet source
    EthSrc = 4,
    /// Ethernet type
    EthType = 5,
    /// VLAN ID
    VlanVid = 6,
    /// VLAN PCP
    VlanPcp = 7,
    /// IP DSCP
    IpDscp = 8,
    /// IP ECN
    IpEcn = 9,
    /// IP protocol
    IpProto = 10,
    /// IPv4 source
    Ipv4Src = 11,
    /// IPv4 destination
    Ipv4Dst = 12,
    /// TCP source port
    TcpSrc = 13,
    /// TCP destination port
    TcpDst = 14,
    /// UDP source port
    UdpSrc = 15,
    /// UDP destination port
    UdpDst = 16,
    /// SCTP source port
    SctpSrc = 17,
    /// SCTP destination port
    SctpDst = 18,
    /// ICMP type
    Icmpv4Type = 19,
    /// ICMP code
    Icmpv4Code = 20,
    /// ARP opcode
    ArpOp = 21,
    /// ARP source IPv4
    ArpSpa = 22,
    /// ARP target IPv4
    ArpTpa = 23,
    /// ARP source MAC
    ArpSha = 24,
    /// ARP target MAC
    ArpTha = 25,
    /// IPv6 source
    Ipv6Src = 26,
    /// IPv6 destination
    Ipv6Dst = 27,
    /// IPv6 flow label
    Ipv6Flabel = 28,
    /// ICMPv6 type
    Icmpv6Type = 29,
    /// ICMPv6 code
    Icmpv6Code = 30,
    /// Tunnel ID
    TunnelId = 38,
}

/// Build an OXM header.
///
/// Format: class (16 bits) | field (7 bits) | hasmask (1 bit) | length (8 bits)
pub fn oxm_header(class: OxmClass, field: OxmField, has_mask: bool, length: u8) -> u32 {
    let class_val = class as u32;
    let field_val = (field as u32) << 1;
    let mask_val = if has_mask { 1u32 } else { 0u32 };
    let len_val = length as u32;

    (class_val << 16) | (field_val << 8) | (mask_val << 8) | len_val
}
