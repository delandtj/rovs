//! MAC NAT (Network Address Translation) flow templates.
//!
//! Provides flow builders for translating MAC addresses between internal
//! and external networks, enabling scenarios like:
//!
//! - Internal hosts appearing with a different MAC externally
//! - Multiple internal hosts sharing a single external MAC
//! - Bidirectional MAC translation

use rovs_openflow::{ActionList, Flow, Match, VConn};

use crate::util::mac_to_u64;
use crate::Result;

/// Configuration for MAC NAT flows.
#[derive(Debug, Clone)]
pub struct MacNatConfig {
    /// Internal MAC address (the actual host MAC).
    pub internal_mac: [u8; 6],
    /// External MAC address (the translated MAC).
    pub external_mac: [u8; 6],
    /// Internal port (where the host is connected).
    pub internal_port: u32,
    /// External port (facing the external network).
    pub external_port: u32,
}

impl MacNatConfig {
    /// Create a new MAC NAT configuration.
    #[must_use]
    pub fn new(
        internal_mac: [u8; 6],
        external_mac: [u8; 6],
        internal_port: u32,
        external_port: u32,
    ) -> Self {
        Self {
            internal_mac,
            external_mac,
            internal_port,
            external_port,
        }
    }
}

/// MAC NAT flow builder.
///
/// Creates flows for translating MAC addresses between internal and external
/// ports. This is useful for scenarios where an internal host needs to appear
/// with a different MAC address on an external network.
///
/// # Example
///
/// ```ignore
/// use rovs_ext::flows::MacNatFlows;
///
/// let config = MacNatConfig::new(
///     [0x02, 0x00, 0x00, 0x00, 0x00, 0x01],  // internal MAC
///     [0x02, 0x00, 0x00, 0x00, 0x00, 0x99],  // external MAC
///     1,  // internal port
///     2,  // external port
/// );
///
/// let flows = MacNatFlows::new(config);
///
/// // Install all flows
/// flows.install(&mut conn, 0, 100).await?;
/// ```
#[derive(Debug, Clone)]
pub struct MacNatFlows {
    config: MacNatConfig,
}

impl MacNatFlows {
    /// Create a new MAC NAT flow builder.
    #[must_use]
    pub fn new(config: MacNatConfig) -> Self {
        Self { config }
    }

    /// Create an outbound flow (internal -> external) for IPv4.
    ///
    /// Matches packets from the internal port with the internal MAC
    /// and rewrites the source MAC to the external MAC.
    #[must_use]
    pub fn ipv4_outbound(&self, table: u8, priority: u16) -> Flow {
        Flow::add()
            .table(table)
            .priority(priority)
            .match_fields(
                Match::new()
                    .in_port(self.config.internal_port)
                    .eth_src(self.config.internal_mac)
                    .eth_type(0x0800), // IPv4
            )
            .actions(
                ActionList::new()
                    .set_eth_src(self.config.external_mac)
                    .output(self.config.external_port),
            )
    }

    /// Create an outbound flow (internal -> external) for IPv6.
    #[must_use]
    pub fn ipv6_outbound(&self, table: u8, priority: u16) -> Flow {
        Flow::add()
            .table(table)
            .priority(priority)
            .match_fields(
                Match::new()
                    .in_port(self.config.internal_port)
                    .eth_src(self.config.internal_mac)
                    .eth_type(0x86dd), // IPv6
            )
            .actions(
                ActionList::new()
                    .set_eth_src(self.config.external_mac)
                    .output(self.config.external_port),
            )
    }

    /// Create an outbound flow (internal -> external) for ARP.
    ///
    /// Note: For full ARP functionality, you may also need ARP proxy flows.
    #[must_use]
    pub fn arp_outbound(&self, table: u8, priority: u16) -> Flow {
        Flow::add()
            .table(table)
            .priority(priority)
            .match_fields(
                Match::new()
                    .in_port(self.config.internal_port)
                    .eth_src(self.config.internal_mac)
                    .eth_type(0x0806), // ARP
            )
            .actions(
                ActionList::new()
                    .set_eth_src(self.config.external_mac)
                    .set_arp_sha(mac_to_u64(&self.config.external_mac))
                    .output(self.config.external_port),
            )
    }

    /// Create an inbound flow (external -> internal) for IPv4.
    ///
    /// Matches packets from the external port destined to the external MAC
    /// and rewrites the destination MAC to the internal MAC.
    #[must_use]
    pub fn ipv4_inbound(&self, table: u8, priority: u16) -> Flow {
        Flow::add()
            .table(table)
            .priority(priority)
            .match_fields(
                Match::new()
                    .in_port(self.config.external_port)
                    .eth_dst(self.config.external_mac)
                    .eth_type(0x0800), // IPv4
            )
            .actions(
                ActionList::new()
                    .set_eth_dst(self.config.internal_mac)
                    .output(self.config.internal_port),
            )
    }

    /// Create an inbound flow (external -> internal) for IPv6.
    #[must_use]
    pub fn ipv6_inbound(&self, table: u8, priority: u16) -> Flow {
        Flow::add()
            .table(table)
            .priority(priority)
            .match_fields(
                Match::new()
                    .in_port(self.config.external_port)
                    .eth_dst(self.config.external_mac)
                    .eth_type(0x86dd), // IPv6
            )
            .actions(
                ActionList::new()
                    .set_eth_dst(self.config.internal_mac)
                    .output(self.config.internal_port),
            )
    }

    /// Create an inbound flow (external -> internal) for ARP.
    #[must_use]
    pub fn arp_inbound(&self, table: u8, priority: u16) -> Flow {
        Flow::add()
            .table(table)
            .priority(priority)
            .match_fields(
                Match::new()
                    .in_port(self.config.external_port)
                    .eth_dst(self.config.external_mac)
                    .eth_type(0x0806), // ARP
            )
            .actions(
                ActionList::new()
                    .set_eth_dst(self.config.internal_mac)
                    .set_arp_tha(mac_to_u64(&self.config.internal_mac))
                    .output(self.config.internal_port),
            )
    }

    /// Get all MAC NAT flows for IPv4, IPv6, and ARP.
    ///
    /// Returns flows for both inbound and outbound directions.
    #[must_use]
    pub fn all_flows(&self, table: u8, base_priority: u16) -> Vec<Flow> {
        vec![
            self.ipv4_outbound(table, base_priority),
            self.ipv4_inbound(table, base_priority),
            self.ipv6_outbound(table, base_priority),
            self.ipv6_inbound(table, base_priority),
            self.arp_outbound(table, base_priority),
            self.arp_inbound(table, base_priority),
        ]
    }

    /// Install all MAC NAT flows to the switch.
    pub async fn install(&self, conn: &mut VConn, table: u8, priority: u16) -> Result<()> {
        for flow in self.all_flows(table, priority) {
            conn.send_flow_sync(&flow).await?;
        }
        Ok(())
    }

    /// Delete all MAC NAT flows from the switch.
    ///
    /// Note: This deletes flows matching the internal/external ports and MACs.
    pub async fn delete(&self, conn: &mut VConn, table: u8) -> Result<()> {
        // Delete outbound flows
        let delete_outbound = Flow::delete()
            .table(table)
            .match_fields(
                Match::new()
                    .in_port(self.config.internal_port)
                    .eth_src(self.config.internal_mac),
            );
        conn.send_flow_sync(&delete_outbound).await?;

        // Delete inbound flows
        let delete_inbound = Flow::delete()
            .table(table)
            .match_fields(
                Match::new()
                    .in_port(self.config.external_port)
                    .eth_dst(self.config.external_mac),
            );
        conn.send_flow_sync(&delete_inbound).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MacNatConfig {
        MacNatConfig::new(
            [0x02, 0x00, 0x00, 0x00, 0x00, 0x01],
            [0x02, 0x00, 0x00, 0x00, 0x00, 0x99],
            1,
            2,
        )
    }

    #[test]
    fn all_flows_returns_six_flows() {
        let flows = MacNatFlows::new(test_config());
        assert_eq!(flows.all_flows(0, 100).len(), 6);
    }

    #[test]
    fn ipv4_outbound_sets_correct_fields() {
        let flows = MacNatFlows::new(test_config());
        let flow = flows.ipv4_outbound(0, 100);

        assert_eq!(flow.table_id, 0);
        assert_eq!(flow.priority, 100);
    }
}
