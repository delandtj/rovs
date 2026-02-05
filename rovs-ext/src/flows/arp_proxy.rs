//! ARP proxy flow templates.
//!
//! Provides flow builders for responding to ARP requests on behalf of
//! a host, enabling Layer 2 proxy functionality.

use rovs_openflow::{nxm, ActionList, Flow, Match, VConn};

use crate::util::{ipv4_to_u32, mac_to_u64};
use crate::Result;

/// Configuration for ARP proxy flows.
#[derive(Debug, Clone)]
pub struct ArpProxyConfig {
    /// IP address to proxy.
    pub ip: [u8; 4],
    /// MAC address to respond with.
    pub mac: [u8; 6],
    /// Port where ARP requests arrive.
    pub port: u32,
}

impl ArpProxyConfig {
    /// Create a new ARP proxy configuration.
    #[must_use]
    pub fn new(ip: [u8; 4], mac: [u8; 6], port: u32) -> Self {
        Self { ip, mac, port }
    }
}

/// ARP proxy flow builder.
///
/// Creates flows that respond to ARP requests for a specific IP address
/// with a configured MAC address. This is useful for:
///
/// - Proxy ARP for hosts behind a gateway
/// - MAC NAT scenarios where the external IP needs ARP responses
/// - Virtual IP addresses that need to respond to ARP
///
/// # How it works
///
/// When an ARP request is received:
/// 1. Match ARP request (opcode=1) for the target IP
/// 2. Transform the packet into an ARP reply by:
///    - Moving sender -> target (IP and MAC)
///    - Setting sender to our MAC/IP
///    - Setting ARP opcode to reply (2)
///    - Swapping Ethernet src/dst
/// 3. Send the reply back to the input port
///
/// # Example
///
/// ```ignore
/// use rovs_ext::flows::ArpProxyFlows;
///
/// let config = ArpProxyConfig::new(
///     [10, 0, 0, 99],  // IP to proxy
///     [0x02, 0x00, 0x00, 0x00, 0x00, 0x99],  // MAC to respond with
///     2,  // port where ARP requests arrive
/// );
///
/// let flows = ArpProxyFlows::new(config);
/// flows.install(&mut conn, 0, 200).await?;
/// ```
#[derive(Debug, Clone)]
pub struct ArpProxyFlows {
    config: ArpProxyConfig,
}

impl ArpProxyFlows {
    /// Create a new ARP proxy flow builder.
    #[must_use]
    pub fn new(config: ArpProxyConfig) -> Self {
        Self { config }
    }

    /// Create the ARP proxy flow.
    ///
    /// This flow matches ARP requests for the configured IP and
    /// generates an ARP reply using Nicira extensions.
    #[must_use]
    pub fn proxy_flow(&self, table: u8, priority: u16) -> Flow {
        Flow::add()
            .table(table)
            .priority(priority)
            .match_fields(
                Match::new()
                    .in_port(self.config.port)
                    .eth_type(0x0806) // ARP
                    .arp_op(1) // ARP Request
                    .arp_tpa(self.config.ip), // Target IP
            )
            .actions(
                ActionList::new()
                    // Move sender MAC to target MAC (ARP SHA -> ARP THA)
                    .move_field(nxm::ARP_SHA, nxm::ARP_THA, 48, 0, 0)
                    // Move sender IP to target IP (ARP SPA -> ARP TPA)
                    .move_field(nxm::ARP_SPA, nxm::ARP_TPA, 32, 0, 0)
                    // Set sender MAC to our MAC
                    .set_arp_sha(mac_to_u64(&self.config.mac))
                    // Set sender IP to our IP
                    .set_arp_spa(ipv4_to_u32(&self.config.ip))
                    // Set ARP opcode to reply (2)
                    .set_arp_op(2)
                    // Move original Ethernet src to dst
                    .move_field(nxm::ETH_SRC, nxm::ETH_DST, 48, 0, 0)
                    // Set Ethernet src to our MAC
                    .set_eth_src(self.config.mac)
                    // Send back to input port
                    .in_port(),
            )
    }

    /// Get all ARP proxy flows (currently just one).
    #[must_use]
    pub fn all_flows(&self, table: u8, priority: u16) -> Vec<Flow> {
        vec![self.proxy_flow(table, priority)]
    }

    /// Install ARP proxy flows to the switch.
    pub async fn install(&self, conn: &mut VConn, table: u8, priority: u16) -> Result<()> {
        for flow in self.all_flows(table, priority) {
            conn.send_flow_sync(&flow).await?;
        }
        Ok(())
    }

    /// Delete ARP proxy flows from the switch.
    pub async fn delete(&self, conn: &mut VConn, table: u8) -> Result<()> {
        let delete = Flow::delete()
            .table(table)
            .match_fields(
                Match::new()
                    .in_port(self.config.port)
                    .eth_type(0x0806)
                    .arp_op(1)
                    .arp_tpa(self.config.ip),
            );
        conn.send_flow_sync(&delete).await?;
        Ok(())
    }
}

/// Builder for multiple ARP proxy entries.
///
/// Use this when you need to proxy ARP for multiple IP/MAC pairs.
#[derive(Debug, Clone, Default)]
pub struct ArpProxyBuilder {
    entries: Vec<ArpProxyConfig>,
}

impl ArpProxyBuilder {
    /// Create a new ARP proxy builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an IP/MAC pair to proxy.
    #[must_use]
    pub fn add(mut self, ip: [u8; 4], mac: [u8; 6], port: u32) -> Self {
        self.entries.push(ArpProxyConfig::new(ip, mac, port));
        self
    }

    /// Add an `ArpProxyConfig` to proxy.
    #[must_use]
    pub fn add_config(mut self, config: ArpProxyConfig) -> Self {
        self.entries.push(config);
        self
    }

    /// Get all ARP proxy flows.
    #[must_use]
    pub fn all_flows(&self, table: u8, priority: u16) -> Vec<Flow> {
        self.entries
            .iter()
            .map(|config| ArpProxyFlows::new(config.clone()).proxy_flow(table, priority))
            .collect()
    }

    /// Install all ARP proxy flows to the switch.
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

    fn test_config() -> ArpProxyConfig {
        ArpProxyConfig::new([10, 0, 0, 99], [0x02, 0x00, 0x00, 0x00, 0x00, 0x99], 2)
    }

    #[test]
    fn proxy_flow_sets_correct_fields() {
        let flows = ArpProxyFlows::new(test_config());
        let flow = flows.proxy_flow(0, 200);

        assert_eq!(flow.table_id, 0);
        assert_eq!(flow.priority, 200);
    }

    #[test]
    fn builder_accumulates_entries() {
        let builder = ArpProxyBuilder::new()
            .add([10, 0, 0, 1], [0x02, 0x00, 0x00, 0x00, 0x00, 0x01], 1)
            .add([10, 0, 0, 2], [0x02, 0x00, 0x00, 0x00, 0x00, 0x02], 2);

        assert_eq!(builder.all_flows(0, 100).len(), 2);
    }
}
