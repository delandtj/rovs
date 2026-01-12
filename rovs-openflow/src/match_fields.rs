//! OpenFlow match field builder.

use std::net::{Ipv4Addr, Ipv6Addr};

/// MAC address type.
pub type MacAddr = [u8; 6];

/// Match fields for flow matching.
///
/// Uses a builder pattern for ergonomic construction.
#[derive(Debug, Clone, Default)]
pub struct Match {
    /// Input port
    pub in_port: Option<u32>,
    /// Input physical port
    pub in_phy_port: Option<u32>,
    /// Metadata
    pub metadata: Option<u64>,
    /// Metadata mask
    pub metadata_mask: Option<u64>,

    // L2
    /// Source MAC address
    pub eth_src: Option<MacAddr>,
    /// Source MAC mask
    pub eth_src_mask: Option<MacAddr>,
    /// Destination MAC address
    pub eth_dst: Option<MacAddr>,
    /// Destination MAC mask
    pub eth_dst_mask: Option<MacAddr>,
    /// Ethernet type
    pub eth_type: Option<u16>,
    /// VLAN ID
    pub vlan_vid: Option<u16>,
    /// VLAN PCP
    pub vlan_pcp: Option<u8>,

    // L3 IPv4
    /// IPv4 source address
    pub ipv4_src: Option<Ipv4Addr>,
    /// IPv4 source mask (prefix length)
    pub ipv4_src_mask: Option<u8>,
    /// IPv4 destination address
    pub ipv4_dst: Option<Ipv4Addr>,
    /// IPv4 destination mask (prefix length)
    pub ipv4_dst_mask: Option<u8>,
    /// IP protocol
    pub ip_proto: Option<u8>,
    /// IP DSCP
    pub ip_dscp: Option<u8>,
    /// IP ECN
    pub ip_ecn: Option<u8>,

    // L3 IPv6
    /// IPv6 source address
    pub ipv6_src: Option<Ipv6Addr>,
    /// IPv6 source mask (prefix length)
    pub ipv6_src_mask: Option<u8>,
    /// IPv6 destination address
    pub ipv6_dst: Option<Ipv6Addr>,
    /// IPv6 destination mask (prefix length)
    pub ipv6_dst_mask: Option<u8>,
    /// IPv6 flow label
    pub ipv6_flabel: Option<u32>,

    // L4 TCP
    /// TCP source port
    pub tcp_src: Option<u16>,
    /// TCP destination port
    pub tcp_dst: Option<u16>,
    /// TCP flags
    pub tcp_flags: Option<u16>,

    // L4 UDP
    /// UDP source port
    pub udp_src: Option<u16>,
    /// UDP destination port
    pub udp_dst: Option<u16>,

    // L4 ICMP
    /// ICMP type
    pub icmp_type: Option<u8>,
    /// ICMP code
    pub icmp_code: Option<u8>,

    // ARP
    /// ARP opcode
    pub arp_op: Option<u16>,
    /// ARP source IPv4
    pub arp_spa: Option<Ipv4Addr>,
    /// ARP target IPv4
    pub arp_tpa: Option<Ipv4Addr>,
    /// ARP source MAC
    pub arp_sha: Option<MacAddr>,
    /// ARP target MAC
    pub arp_tha: Option<MacAddr>,

    // Tunnel
    /// Tunnel ID
    pub tunnel_id: Option<u64>,
}

impl Match {
    /// Create a new empty match (matches all packets).
    pub fn new() -> Self {
        Self::default()
    }

    /// Match on input port.
    pub fn in_port(mut self, port: u32) -> Self {
        self.in_port = Some(port);
        self
    }

    /// Match on source MAC address.
    pub fn eth_src(mut self, addr: MacAddr) -> Self {
        self.eth_src = Some(addr);
        self
    }

    /// Match on destination MAC address.
    pub fn eth_dst(mut self, addr: MacAddr) -> Self {
        self.eth_dst = Some(addr);
        self
    }

    /// Match on Ethernet type.
    pub fn eth_type(mut self, etype: u16) -> Self {
        self.eth_type = Some(etype);
        self
    }

    /// Match on VLAN ID.
    pub fn vlan_vid(mut self, vid: u16) -> Self {
        self.vlan_vid = Some(vid);
        self
    }

    /// Match on IPv4 source address with prefix length.
    pub fn ipv4_src(mut self, addr: Ipv4Addr, prefix_len: u8) -> Self {
        self.eth_type = Some(0x0800); // IP
        self.ipv4_src = Some(addr);
        self.ipv4_src_mask = Some(prefix_len);
        self
    }

    /// Match on IPv4 destination address with prefix length.
    pub fn ipv4_dst(mut self, addr: Ipv4Addr, prefix_len: u8) -> Self {
        self.eth_type = Some(0x0800); // IP
        self.ipv4_dst = Some(addr);
        self.ipv4_dst_mask = Some(prefix_len);
        self
    }

    /// Match on IP protocol.
    pub fn ip_proto(mut self, proto: u8) -> Self {
        self.ip_proto = Some(proto);
        self
    }

    /// Match on TCP source port.
    pub fn tcp_src(mut self, port: u16) -> Self {
        self.ip_proto = Some(6); // TCP
        self.tcp_src = Some(port);
        self
    }

    /// Match on TCP destination port.
    pub fn tcp_dst(mut self, port: u16) -> Self {
        self.ip_proto = Some(6); // TCP
        self.tcp_dst = Some(port);
        self
    }

    /// Match on UDP source port.
    pub fn udp_src(mut self, port: u16) -> Self {
        self.ip_proto = Some(17); // UDP
        self.udp_src = Some(port);
        self
    }

    /// Match on UDP destination port.
    pub fn udp_dst(mut self, port: u16) -> Self {
        self.ip_proto = Some(17); // UDP
        self.udp_dst = Some(port);
        self
    }

    /// Match on tunnel ID.
    pub fn tunnel_id(mut self, id: u64) -> Self {
        self.tunnel_id = Some(id);
        self
    }

    /// Check if this match is empty (matches all).
    pub fn is_empty(&self) -> bool {
        self.in_port.is_none()
            && self.eth_src.is_none()
            && self.eth_dst.is_none()
            && self.eth_type.is_none()
            && self.vlan_vid.is_none()
            && self.ipv4_src.is_none()
            && self.ipv4_dst.is_none()
            && self.ip_proto.is_none()
            && self.tcp_src.is_none()
            && self.tcp_dst.is_none()
            && self.udp_src.is_none()
            && self.udp_dst.is_none()
            && self.tunnel_id.is_none()
    }
}
