//! VLAN flow helper templates.
//!
//! Provides flow builders for common VLAN operations like
//! tagging, untagging, and VLAN-based forwarding.

use rovs_openflow::{ActionList, Flow, Match, VConn};

use crate::Result;

/// Push a VLAN tag on packets from a port.
///
/// Creates a flow that matches untagged packets from the specified port
/// and pushes a VLAN tag before forwarding.
///
/// # Arguments
///
/// * `in_port` - Input port (untagged)
/// * `out_port` - Output port (tagged)
/// * `vlan_id` - VLAN ID to push (1-4094)
/// * `table` - Flow table
/// * `priority` - Flow priority
#[must_use]
pub fn push_vlan_flow(in_port: u32, out_port: u32, vlan_id: u16, table: u8, priority: u16) -> Flow {
    Flow::add()
        .table(table)
        .priority(priority)
        .match_fields(Match::new().in_port(in_port))
        .actions(
            ActionList::new()
                .push_vlan(0x8100) // 802.1Q
                .set_vlan_vid(vlan_id)
                .output(out_port),
        )
}

/// Pop a VLAN tag from packets.
///
/// Creates a flow that matches tagged packets with a specific VLAN ID
/// and pops the tag before forwarding.
///
/// # Arguments
///
/// * `in_port` - Input port (tagged)
/// * `out_port` - Output port (untagged)
/// * `vlan_id` - VLAN ID to match and pop
/// * `table` - Flow table
/// * `priority` - Flow priority
#[must_use]
pub fn pop_vlan_flow(in_port: u32, out_port: u32, vlan_id: u16, table: u8, priority: u16) -> Flow {
    Flow::add()
        .table(table)
        .priority(priority)
        .match_fields(Match::new().in_port(in_port).vlan_vid(vlan_id))
        .actions(ActionList::new().pop_vlan().output(out_port))
}

/// Forward packets within a VLAN (tagged to tagged).
///
/// Creates a flow that matches packets with a specific VLAN ID
/// and forwards them to another port without modifying the tag.
#[must_use]
pub fn forward_vlan_flow(
    in_port: u32,
    out_port: u32,
    vlan_id: u16,
    table: u8,
    priority: u16,
) -> Flow {
    Flow::add()
        .table(table)
        .priority(priority)
        .match_fields(Match::new().in_port(in_port).vlan_vid(vlan_id))
        .actions(ActionList::new().output(out_port))
}

/// Rewrite VLAN tag (translate between VLANs).
///
/// Creates a flow that matches packets with one VLAN ID and
/// changes it to a different VLAN ID.
///
/// # Arguments
///
/// * `in_port` - Input port
/// * `out_port` - Output port
/// * `src_vlan` - Source VLAN ID to match
/// * `dst_vlan` - Destination VLAN ID to set
/// * `table` - Flow table
/// * `priority` - Flow priority
#[must_use]
pub fn translate_vlan_flow(
    in_port: u32,
    out_port: u32,
    src_vlan: u16,
    dst_vlan: u16,
    table: u8,
    priority: u16,
) -> Flow {
    Flow::add()
        .table(table)
        .priority(priority)
        .match_fields(Match::new().in_port(in_port).vlan_vid(src_vlan))
        .actions(ActionList::new().set_vlan_vid(dst_vlan).output(out_port))
}

/// VLAN access port configuration.
///
/// Configures a port as a VLAN access port (untagged), pushing
/// a VLAN tag on ingress and popping it on egress.
#[derive(Debug, Clone)]
pub struct VlanAccessPort {
    /// Port number.
    pub port: u32,
    /// VLAN ID for this access port.
    pub vlan_id: u16,
    /// Trunk port where tagged packets go.
    pub trunk_port: u32,
}

impl VlanAccessPort {
    /// Create a new VLAN access port configuration.
    #[must_use]
    pub fn new(port: u32, vlan_id: u16, trunk_port: u32) -> Self {
        Self {
            port,
            vlan_id,
            trunk_port,
        }
    }

    /// Create ingress flow (access -> trunk, push VLAN).
    #[must_use]
    pub fn ingress_flow(&self, table: u8, priority: u16) -> Flow {
        push_vlan_flow(self.port, self.trunk_port, self.vlan_id, table, priority)
    }

    /// Create egress flow (trunk -> access, pop VLAN).
    #[must_use]
    pub fn egress_flow(&self, table: u8, priority: u16) -> Flow {
        pop_vlan_flow(self.trunk_port, self.port, self.vlan_id, table, priority)
    }

    /// Get all flows for this access port.
    #[must_use]
    pub fn all_flows(&self, table: u8, priority: u16) -> Vec<Flow> {
        vec![
            self.ingress_flow(table, priority),
            self.egress_flow(table, priority),
        ]
    }

    /// Install access port flows to the switch.
    pub async fn install(&self, conn: &mut VConn, table: u8, priority: u16) -> Result<()> {
        for flow in self.all_flows(table, priority) {
            conn.send_flow_sync(&flow).await?;
        }
        Ok(())
    }
}

/// Builder for VLAN configuration with multiple access ports.
#[derive(Debug, Clone, Default)]
pub struct VlanConfig {
    /// Access ports in this VLAN.
    access_ports: Vec<VlanAccessPort>,
}

impl VlanConfig {
    /// Create a new VLAN configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an access port to this VLAN.
    #[must_use]
    pub fn add_access_port(mut self, port: u32, vlan_id: u16, trunk_port: u32) -> Self {
        self.access_ports
            .push(VlanAccessPort::new(port, vlan_id, trunk_port));
        self
    }

    /// Get all flows for this VLAN configuration.
    #[must_use]
    pub fn all_flows(&self, table: u8, priority: u16) -> Vec<Flow> {
        self.access_ports
            .iter()
            .flat_map(|ap| ap.all_flows(table, priority))
            .collect()
    }

    /// Install all VLAN flows to the switch.
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

    #[test]
    fn push_vlan_flow_sets_fields() {
        let flow = push_vlan_flow(1, 2, 100, 0, 50);
        assert_eq!(flow.table_id, 0);
        assert_eq!(flow.priority, 50);
    }

    #[test]
    fn pop_vlan_flow_sets_fields() {
        let flow = pop_vlan_flow(2, 1, 100, 0, 50);
        assert_eq!(flow.table_id, 0);
        assert_eq!(flow.priority, 50);
    }

    #[test]
    fn vlan_access_port_creates_two_flows() {
        let ap = VlanAccessPort::new(1, 100, 10);
        assert_eq!(ap.all_flows(0, 50).len(), 2);
    }

    #[test]
    fn vlan_config_accumulates_flows() {
        let config = VlanConfig::new()
            .add_access_port(1, 100, 10)
            .add_access_port(2, 100, 10);

        // 2 access ports * 2 flows each = 4 flows
        assert_eq!(config.all_flows(0, 50).len(), 4);
    }
}
