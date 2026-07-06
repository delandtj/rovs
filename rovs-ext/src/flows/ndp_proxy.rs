//! NDP (Neighbor Discovery Protocol) proxy flow templates.
//!
//! Provides flow builders for responding to IPv6 Neighbor Solicitation
//! messages on behalf of a host.

use std::net::Ipv6Addr;

use rovs_openflow::{ActionList, Flow, Match, VConn};

use crate::Result;

/// Configuration for NDP proxy flows.
#[derive(Debug, Clone)]
pub struct NdpProxyConfig {
    /// IPv6 address to proxy.
    pub ipv6: Ipv6Addr,
    /// MAC address to respond with.
    pub mac: [u8; 6],
    /// Port where NDP requests arrive.
    pub port: u32,
}

impl NdpProxyConfig {
    /// Create a new NDP proxy configuration.
    #[must_use]
    pub fn new(ipv6: Ipv6Addr, mac: [u8; 6], port: u32) -> Self {
        Self { ipv6, mac, port }
    }
}

/// NDP proxy flow builder.
///
/// Unlike ARP proxy flows which can be handled entirely in the datapath
/// using Nicira extensions, NDP proxy requires controller interaction
/// because of the ICMPv6 checksum that must be calculated.
///
/// This builder creates flows that send Neighbor Solicitation packets
/// to the controller, where they can be processed by a controller handler.
///
/// For a complete NDP proxy solution, use this with `NdpProxyHandler`
/// from the controller framework.
///
/// # Example
///
/// ```ignore
/// use std::net::Ipv6Addr;
/// use rovs_ext::flows::NdpProxyFlows;
///
/// let config = NdpProxyConfig::new(
///     "fd00::99".parse().unwrap(),
///     [0x02, 0x00, 0x00, 0x00, 0x00, 0x99],
///     2,
/// );
///
/// let flows = NdpProxyFlows::new(config);
/// flows.install(&mut conn, 0, 200).await?;
/// ```
#[derive(Debug, Clone)]
pub struct NdpProxyFlows {
    config: NdpProxyConfig,
}

impl NdpProxyFlows {
    /// Create a new NDP proxy flow builder.
    #[must_use]
    pub fn new(config: NdpProxyConfig) -> Self {
        Self { config }
    }

    /// Create a flow that sends Neighbor Solicitations to the controller.
    ///
    /// This flow matches ICMPv6 Neighbor Solicitation (type 135) messages
    /// and sends them to the controller for processing.
    #[must_use]
    pub fn to_controller_flow(&self, table: u8, priority: u16) -> Flow {
        Flow::add()
            .table(table)
            .priority(priority)
            .match_fields(
                Match::new()
                    .in_port(self.config.port)
                    .eth_type(0x86dd) // IPv6
                    .ip_proto(58) // ICMPv6
                    .icmpv6_type(135), // Neighbor Solicitation
            )
            .actions(
                ActionList::new().controller(0xffff), // Send entire packet
            )
    }

    /// Get all NDP proxy flows.
    #[must_use]
    pub fn all_flows(&self, table: u8, priority: u16) -> Vec<Flow> {
        vec![self.to_controller_flow(table, priority)]
    }

    /// Install NDP proxy flows to the switch.
    pub async fn install(&self, conn: &mut VConn, table: u8, priority: u16) -> Result<()> {
        for flow in self.all_flows(table, priority) {
            conn.send_flow_sync(&flow).await?;
        }
        Ok(())
    }

    /// Delete NDP proxy flows from the switch.
    pub async fn delete(&self, conn: &mut VConn, table: u8) -> Result<()> {
        let delete = Flow::delete().table(table).match_fields(
            Match::new()
                .in_port(self.config.port)
                .eth_type(0x86dd)
                .ip_proto(58)
                .icmpv6_type(135),
        );
        conn.send_flow_sync(&delete).await?;
        Ok(())
    }
}

/// Builder for multiple NDP proxy entries.
///
/// Use this when you need to proxy NDP for multiple IPv6/MAC pairs.
/// All entries share the same flow (NS to controller), but the handler
/// will match the target address against configured entries.
#[derive(Debug, Clone, Default)]
pub struct NdpProxyBuilder {
    entries: Vec<NdpProxyConfig>,
}

impl NdpProxyBuilder {
    /// Create a new NDP proxy builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an IPv6/MAC pair to proxy.
    #[must_use]
    pub fn add(mut self, ipv6: Ipv6Addr, mac: [u8; 6], port: u32) -> Self {
        self.entries.push(NdpProxyConfig::new(ipv6, mac, port));
        self
    }

    /// Add an `NdpProxyConfig` to proxy.
    #[must_use]
    pub fn add_config(mut self, config: NdpProxyConfig) -> Self {
        self.entries.push(config);
        self
    }

    /// Get the configured entries.
    pub fn entries(&self) -> &[NdpProxyConfig] {
        &self.entries
    }

    /// Create a general NS-to-controller flow.
    ///
    /// Since the target IPv6 address is in the ICMPv6 payload (not matchable
    /// in standard OpenFlow), we send all NS packets to the controller and
    /// filter by target address there.
    #[must_use]
    pub fn ns_to_controller_flow(&self, table: u8, priority: u16, port: u32) -> Flow {
        Flow::add()
            .table(table)
            .priority(priority)
            .match_fields(
                Match::new()
                    .in_port(port)
                    .eth_type(0x86dd)
                    .ip_proto(58)
                    .icmpv6_type(135),
            )
            .actions(ActionList::new().controller(0xffff))
    }

    /// Get the set of ports that need NS-to-controller flows.
    #[must_use]
    pub fn ports(&self) -> Vec<u32> {
        let mut ports: Vec<u32> = self.entries.iter().map(|e| e.port).collect();
        ports.sort_unstable();
        ports.dedup();
        ports
    }

    /// Get all NS-to-controller flows (one per unique port).
    #[must_use]
    pub fn all_flows(&self, table: u8, priority: u16) -> Vec<Flow> {
        self.ports()
            .into_iter()
            .map(|port| self.ns_to_controller_flow(table, priority, port))
            .collect()
    }

    /// Install all NDP proxy flows to the switch.
    pub async fn install(&self, conn: &mut VConn, table: u8, priority: u16) -> Result<()> {
        for flow in self.all_flows(table, priority) {
            conn.send_flow_sync(&flow).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> NdpProxyConfig {
        NdpProxyConfig::new(
            "fd00::99".parse().unwrap(),
            [0x02, 0x00, 0x00, 0x00, 0x00, 0x99],
            2,
        )
    }

    #[test]
    fn to_controller_flow_sets_correct_fields() {
        let flows = NdpProxyFlows::new(test_config());
        let flow = flows.to_controller_flow(0, 200);

        assert_eq!(flow.table_id, 0);
        assert_eq!(flow.priority, 200);
    }

    #[test]
    fn builder_deduplicates_ports() {
        let builder = NdpProxyBuilder::new()
            .add("fd00::1".parse().unwrap(), [0x02; 6], 1)
            .add("fd00::2".parse().unwrap(), [0x03; 6], 1) // same port
            .add("fd00::3".parse().unwrap(), [0x04; 6], 2);

        // Should have 2 flows (one per unique port)
        assert_eq!(builder.all_flows(0, 100).len(), 2);
    }
}
