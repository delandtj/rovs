//! Controller events.

use rovs_openflow::PacketIn;

/// Packet-In event with parsed metadata.
#[derive(Debug, Clone)]
pub struct PacketInEvent {
    /// The raw Packet-In message.
    pub packet_in: PacketIn,
    /// Input port where the packet was received.
    pub in_port: u32,
    /// Parsed Ethernet header (if available).
    pub eth: Option<ParsedEthernet>,
}

impl PacketInEvent {
    /// Create a new Packet-In event from a raw Packet-In message.
    #[must_use]
    pub fn from_packet_in(packet_in: PacketIn) -> Self {
        let in_port = packet_in.in_port();
        let eth = ParsedEthernet::parse(&packet_in.data);

        Self {
            packet_in,
            in_port,
            eth,
        }
    }

    /// Get the packet data.
    pub fn data(&self) -> &[u8] {
        &self.packet_in.data
    }

    /// Get the buffer ID (if any).
    pub fn buffer_id(&self) -> Option<u32> {
        if self.packet_in.buffer_id == rovs_openflow::OFP_NO_BUFFER {
            None
        } else {
            Some(self.packet_in.buffer_id)
        }
    }

    /// Get the Ethernet source MAC (if parsed).
    pub fn eth_src(&self) -> Option<[u8; 6]> {
        self.eth.as_ref().map(|e| e.src_mac)
    }

    /// Get the Ethernet destination MAC (if parsed).
    pub fn eth_dst(&self) -> Option<[u8; 6]> {
        self.eth.as_ref().map(|e| e.dst_mac)
    }

    /// Get the Ethertype (if parsed).
    pub fn ethertype(&self) -> Option<u16> {
        self.eth.as_ref().map(|e| e.ethertype)
    }

    /// Check if this is an ARP packet.
    pub fn is_arp(&self) -> bool {
        self.ethertype() == Some(0x0806)
    }

    /// Check if this is an IPv4 packet.
    pub fn is_ipv4(&self) -> bool {
        self.ethertype() == Some(0x0800)
    }

    /// Check if this is an IPv6 packet.
    pub fn is_ipv6(&self) -> bool {
        self.ethertype() == Some(0x86dd)
    }
}

/// Parsed Ethernet header.
#[derive(Debug, Clone)]
pub struct ParsedEthernet {
    /// Destination MAC address.
    pub dst_mac: [u8; 6],
    /// Source MAC address.
    pub src_mac: [u8; 6],
    /// Ethertype.
    pub ethertype: u16,
    /// Payload offset.
    pub payload_offset: usize,
}

impl ParsedEthernet {
    /// Parse an Ethernet header from raw bytes.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 14 {
            return None;
        }

        let dst_mac: [u8; 6] = data[0..6].try_into().ok()?;
        let src_mac: [u8; 6] = data[6..12].try_into().ok()?;
        let ethertype = u16::from_be_bytes([data[12], data[13]]);

        Some(Self {
            dst_mac,
            src_mac,
            ethertype,
            payload_offset: 14,
        })
    }
}

/// Controller event types.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ControllerEvent {
    /// Packet received from switch.
    PacketIn(PacketInEvent),
    /// Connection established.
    Connected,
    /// Connection lost.
    Disconnected,
}
