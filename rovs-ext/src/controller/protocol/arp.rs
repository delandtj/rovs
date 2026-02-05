//! ARP protocol handler.
//!
//! Handles ARP requests by responding with configured MAC addresses.
//! This is useful for ARP proxy scenarios that can't be handled
//! entirely in the datapath (e.g., when the Nicira extensions aren't
//! available or more complex logic is needed).

use std::collections::HashMap;

use rovs_openflow::{ActionList, PacketOut};

use crate::controller::event::PacketInEvent;
use crate::controller::handler::{HandlerAction, HandlerContext, PacketHandler};
use crate::util::format_ipv4;
use crate::Result;

/// ARP operation codes.
const ARP_REQUEST: u16 = 1;
const ARP_REPLY: u16 = 2;

/// ARP packet offsets (after Ethernet header).
const ARP_HTYPE: usize = 0;
const ARP_PTYPE: usize = 2;
const ARP_HLEN: usize = 4;
const ARP_PLEN: usize = 5;
const ARP_OPER: usize = 6;
const ARP_SHA: usize = 8;
const ARP_SPA: usize = 14;
const ARP_THA: usize = 18;
const ARP_TPA: usize = 24;

/// Parsed ARP packet.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ArpPacket {
    /// Hardware type.
    htype: u16,
    /// Protocol type.
    ptype: u16,
    /// Hardware address length.
    hlen: u8,
    /// Protocol address length.
    plen: u8,
    /// Operation.
    pub oper: u16,
    /// Sender hardware address.
    pub sha: [u8; 6],
    /// Sender protocol address.
    pub spa: [u8; 4],
    /// Target hardware address.
    tha: [u8; 6],
    /// Target protocol address.
    pub tpa: [u8; 4],
}

impl ArpPacket {
    /// Parse an ARP packet from raw bytes (starting at ARP header).
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 28 {
            return None;
        }

        let htype = u16::from_be_bytes([data[ARP_HTYPE], data[ARP_HTYPE + 1]]);
        let ptype = u16::from_be_bytes([data[ARP_PTYPE], data[ARP_PTYPE + 1]]);
        let hlen = data[ARP_HLEN];
        let plen = data[ARP_PLEN];
        let oper = u16::from_be_bytes([data[ARP_OPER], data[ARP_OPER + 1]]);

        // Only handle Ethernet/IPv4 ARP
        if htype != 1 || ptype != 0x0800 || hlen != 6 || plen != 4 {
            return None;
        }

        let sha: [u8; 6] = data[ARP_SHA..ARP_SHA + 6].try_into().ok()?;
        let spa: [u8; 4] = data[ARP_SPA..ARP_SPA + 4].try_into().ok()?;
        let tha: [u8; 6] = data[ARP_THA..ARP_THA + 6].try_into().ok()?;
        let tpa: [u8; 4] = data[ARP_TPA..ARP_TPA + 4].try_into().ok()?;

        Some(Self {
            htype,
            ptype,
            hlen,
            plen,
            oper,
            sha,
            spa,
            tha,
            tpa,
        })
    }

    /// Check if this is an ARP request.
    pub fn is_request(&self) -> bool {
        self.oper == ARP_REQUEST
    }

    /// Build an ARP reply to this request.
    pub fn build_reply(&self, reply_mac: [u8; 6]) -> Vec<u8> {
        let mut reply = vec![0u8; 28];

        // Hardware type: Ethernet
        reply[ARP_HTYPE..ARP_HTYPE + 2].copy_from_slice(&1u16.to_be_bytes());
        // Protocol type: IPv4
        reply[ARP_PTYPE..ARP_PTYPE + 2].copy_from_slice(&0x0800u16.to_be_bytes());
        // Hardware length: 6
        reply[ARP_HLEN] = 6;
        // Protocol length: 4
        reply[ARP_PLEN] = 4;
        // Operation: reply
        reply[ARP_OPER..ARP_OPER + 2].copy_from_slice(&ARP_REPLY.to_be_bytes());
        // Sender MAC: our MAC
        reply[ARP_SHA..ARP_SHA + 6].copy_from_slice(&reply_mac);
        // Sender IP: the IP they requested
        reply[ARP_SPA..ARP_SPA + 4].copy_from_slice(&self.tpa);
        // Target MAC: original sender
        reply[ARP_THA..ARP_THA + 6].copy_from_slice(&self.sha);
        // Target IP: original sender IP
        reply[ARP_TPA..ARP_TPA + 4].copy_from_slice(&self.spa);

        reply
    }
}

/// ARP proxy handler.
///
/// Responds to ARP requests for configured IP addresses with
/// configured MAC addresses.
///
/// # Example
///
/// ```ignore
/// let mut handler = ArpProxyHandler::new();
/// handler.add_entry([10, 0, 0, 99], [0x02, 0x00, 0x00, 0x00, 0x00, 0x99]);
///
/// controller.register(handler);
/// ```
#[derive(Debug, Clone, Default)]
pub struct ArpProxyHandler {
    /// Map from IP address to MAC address.
    entries: HashMap<[u8; 4], [u8; 6]>,
}

impl ArpProxyHandler {
    /// Create a new ARP proxy handler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an IP/MAC entry to proxy.
    pub fn add_entry(&mut self, ip: [u8; 4], mac: [u8; 6]) {
        self.entries.insert(ip, mac);
    }

    /// Remove an IP entry.
    pub fn remove_entry(&mut self, ip: &[u8; 4]) {
        self.entries.remove(ip);
    }

    /// Check if we have an entry for an IP.
    pub fn has_entry(&self, ip: &[u8; 4]) -> bool {
        self.entries.contains_key(ip)
    }

    /// Get the MAC for an IP (if configured).
    pub fn get_mac(&self, ip: &[u8; 4]) -> Option<[u8; 6]> {
        self.entries.get(ip).copied()
    }

    /// Build a full Ethernet+ARP reply packet.
    #[allow(clippy::unused_self)]
    fn build_reply_packet(&self, event: &PacketInEvent, arp: &ArpPacket, reply_mac: [u8; 6]) -> Vec<u8> {
        let eth_src = event.eth_src().unwrap_or([0; 6]);

        let mut packet = Vec::with_capacity(14 + 28);

        // Ethernet header
        packet.extend_from_slice(&eth_src); // dst = original sender
        packet.extend_from_slice(&reply_mac); // src = our MAC
        packet.extend_from_slice(&0x0806u16.to_be_bytes()); // ethertype = ARP

        // ARP payload
        packet.extend_from_slice(&arp.build_reply(reply_mac));

        packet
    }
}

impl PacketHandler for ArpProxyHandler {
    fn can_handle(&self, event: &PacketInEvent) -> bool {
        event.is_arp()
    }

    fn handle<'a>(
        &'a self,
        event: &'a PacketInEvent,
        _ctx: &'a mut HandlerContext<'_>,
    ) -> crate::controller::handler::BoxFuture<'a, Result<HandlerAction>> {
        Box::pin(async move {
            // Parse ARP packet (starts after Ethernet header)
            let data = event.data();
            if data.len() < 14 + 28 {
                return Ok(HandlerAction::NotHandled);
            }

            let arp = match ArpPacket::parse(&data[14..]) {
                Some(arp) => arp,
                None => return Ok(HandlerAction::NotHandled),
            };

            // Only handle ARP requests
            if !arp.is_request() {
                return Ok(HandlerAction::NotHandled);
            }

            // Check if we have an entry for the target IP
            let reply_mac = match self.get_mac(&arp.tpa) {
                Some(mac) => mac,
                None => return Ok(HandlerAction::NotHandled),
            };

            tracing::debug!(
                "ARP proxy: {} -> {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                format_ipv4(&arp.tpa),
                reply_mac[0], reply_mac[1], reply_mac[2],
                reply_mac[3], reply_mac[4], reply_mac[5]
            );

            // Build and send reply
            let reply_packet = self.build_reply_packet(event, &arp, reply_mac);
            let packet_out = PacketOut::new()
                .in_port(event.in_port)
                .actions(ActionList::new().in_port())
                .data(reply_packet);

            Ok(HandlerAction::SendPacketOut(packet_out))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_arp_request() {
        // Minimal valid ARP request
        let mut data = vec![0u8; 28];
        data[0..2].copy_from_slice(&1u16.to_be_bytes()); // htype = Ethernet
        data[2..4].copy_from_slice(&0x0800u16.to_be_bytes()); // ptype = IPv4
        data[4] = 6; // hlen
        data[5] = 4; // plen
        data[6..8].copy_from_slice(&1u16.to_be_bytes()); // oper = request

        let arp = ArpPacket::parse(&data).unwrap();
        assert!(arp.is_request());
    }

    #[test]
    fn handler_has_entry() {
        let mut handler = ArpProxyHandler::new();
        handler.add_entry([10, 0, 0, 99], [0x02, 0x00, 0x00, 0x00, 0x00, 0x99]);

        assert!(handler.has_entry(&[10, 0, 0, 99]));
        assert!(!handler.has_entry(&[10, 0, 0, 100]));
    }
}
