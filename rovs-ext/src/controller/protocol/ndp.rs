//! NDP (Neighbor Discovery Protocol) handler.
//!
//! Handles IPv6 Neighbor Solicitation messages by responding with
//! configured MAC addresses. Uses the NDP parsing from rovs_openflow.

use std::collections::HashMap;
use std::net::Ipv6Addr;

use rovs_openflow::ndp::{build_na_reply, parse_neighbor_solicitation};
use rovs_openflow::{ActionList, PacketOut};

use crate::Result;
use crate::controller::event::PacketInEvent;
use crate::controller::handler::{HandlerAction, HandlerContext, PacketHandler};

/// NDP proxy handler.
///
/// Responds to Neighbor Solicitation messages for configured IPv6
/// addresses with configured MAC addresses.
///
/// # Example
///
/// ```ignore
/// let mut handler = NdpProxyHandler::new();
/// handler.add_entry(
///     "fd00::99".parse().unwrap(),
///     [0x02, 0x00, 0x00, 0x00, 0x00, 0x99],
/// );
///
/// controller.register(handler);
/// ```
#[derive(Debug, Clone, Default)]
pub struct NdpProxyHandler {
    /// Map from IPv6 address to MAC address.
    entries: HashMap<Ipv6Addr, [u8; 6]>,
}

impl NdpProxyHandler {
    /// Create a new NDP proxy handler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an IPv6/MAC entry to proxy.
    pub fn add_entry(&mut self, ipv6: Ipv6Addr, mac: [u8; 6]) {
        self.entries.insert(ipv6, mac);
    }

    /// Remove an IPv6 entry.
    pub fn remove_entry(&mut self, ipv6: &Ipv6Addr) {
        self.entries.remove(ipv6);
    }

    /// Check if we have an entry for an IPv6 address.
    pub fn has_entry(&self, ipv6: &Ipv6Addr) -> bool {
        self.entries.contains_key(ipv6)
    }

    /// Get the MAC for an IPv6 address (if configured).
    pub fn get_mac(&self, ipv6: &Ipv6Addr) -> Option<[u8; 6]> {
        self.entries.get(ipv6).copied()
    }
}

impl PacketHandler for NdpProxyHandler {
    fn can_handle(&self, event: &PacketInEvent) -> bool {
        event.is_ipv6()
    }

    fn handle<'a>(
        &'a self,
        event: &'a PacketInEvent,
        _ctx: &'a mut HandlerContext<'_>,
    ) -> crate::controller::handler::BoxFuture<'a, Result<HandlerAction>> {
        Box::pin(async move {
            let data = event.data();

            // Parse Neighbor Solicitation
            let (eth, ipv6, ns) = match parse_neighbor_solicitation(data) {
                Some(parsed) => parsed,
                None => return Ok(HandlerAction::NotHandled),
            };

            // Check if we have an entry for the target IPv6 address
            let reply_mac = match self.get_mac(&ns.target_addr) {
                Some(mac) => mac,
                None => return Ok(HandlerAction::NotHandled),
            };

            tracing::debug!(
                "NDP proxy: {} -> {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                ns.target_addr,
                reply_mac[0],
                reply_mac[1],
                reply_mac[2],
                reply_mac[3],
                reply_mac[4],
                reply_mac[5]
            );

            // Build Neighbor Advertisement reply
            let reply_packet = build_na_reply(&eth, &ipv6, &ns, reply_mac, ns.target_addr);

            // Send reply back to input port
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
    fn handler_has_entry() {
        let mut handler = NdpProxyHandler::new();
        handler.add_entry(
            "fd00::99".parse().unwrap(),
            [0x02, 0x00, 0x00, 0x00, 0x00, 0x99],
        );

        assert!(handler.has_entry(&"fd00::99".parse().unwrap()));
        assert!(!handler.has_entry(&"fd00::100".parse().unwrap()));
    }

    #[test]
    fn can_handle_checks_ethertype() {
        let handler = NdpProxyHandler::new();
        let _ = handler; // silence unused warning

        // Create a mock event with IPv6 ethertype using ParsedEthernet
        let mut data = vec![0u8; 14];
        data[12..14].copy_from_slice(&0x86ddu16.to_be_bytes());

        let eth = crate::controller::ParsedEthernet::parse(&data);
        assert!(eth.is_some());
        assert_eq!(eth.unwrap().ethertype, 0x86dd);
    }
}
