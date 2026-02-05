//! VLAN trunk topology builder.
//!
//! Creates VLAN trunk configurations for bridges with multiple
//! access ports in different VLANs.

use rovs_ovsdb::{Client, Transaction};
use serde_json::json;

use crate::Result;

/// Access port configuration.
#[derive(Debug, Clone)]
pub struct AccessPortConfig {
    /// Port name.
    pub name: String,
    /// VLAN ID for this access port.
    pub vlan_id: u16,
    /// Interface type (default: "internal").
    pub iface_type: String,
}

impl AccessPortConfig {
    /// Create a new access port configuration.
    #[must_use]
    pub fn new(name: impl Into<String>, vlan_id: u16) -> Self {
        Self {
            name: name.into(),
            vlan_id,
            iface_type: "internal".to_owned(),
        }
    }

    /// Set the interface type.
    #[must_use]
    pub fn iface_type(mut self, iface_type: impl Into<String>) -> Self {
        self.iface_type = iface_type.into();
        self
    }

    /// Create a system port (for existing kernel interfaces).
    #[must_use]
    pub fn system(name: impl Into<String>, vlan_id: u16) -> Self {
        Self {
            name: name.into(),
            vlan_id,
            iface_type: "system".to_owned(),
        }
    }
}

/// Trunk port configuration.
#[derive(Debug, Clone)]
pub struct TrunkPortConfig {
    /// Port name.
    pub name: String,
    /// Allowed VLANs (empty = all VLANs).
    pub allowed_vlans: Vec<u16>,
    /// Interface type (default: "internal").
    pub iface_type: String,
}

impl TrunkPortConfig {
    /// Create a new trunk port configuration.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            allowed_vlans: Vec::new(),
            iface_type: "internal".to_owned(),
        }
    }

    /// Set the allowed VLANs for this trunk.
    #[must_use]
    pub fn allowed_vlans(mut self, vlans: Vec<u16>) -> Self {
        self.allowed_vlans = vlans;
        self
    }

    /// Set the interface type.
    #[must_use]
    pub fn iface_type(mut self, iface_type: impl Into<String>) -> Self {
        self.iface_type = iface_type.into();
        self
    }

    /// Create a system trunk port (for existing kernel interfaces).
    #[must_use]
    pub fn system(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            allowed_vlans: Vec::new(),
            iface_type: "system".to_owned(),
        }
    }
}

/// VLAN trunk configuration for a bridge.
#[derive(Debug, Clone)]
pub struct VlanTrunkConfig {
    /// Bridge name.
    pub bridge: String,
    /// Whether to create the bridge (false = assume it exists).
    pub create_bridge: bool,
    /// Access ports.
    pub access_ports: Vec<AccessPortConfig>,
    /// Trunk ports.
    pub trunk_ports: Vec<TrunkPortConfig>,
    /// Set fail_mode to secure.
    pub secure_fail_mode: bool,
}

impl VlanTrunkConfig {
    /// Create a new VLAN trunk configuration.
    #[must_use]
    pub fn new(bridge: impl Into<String>) -> Self {
        Self {
            bridge: bridge.into(),
            create_bridge: true,
            access_ports: Vec::new(),
            trunk_ports: Vec::new(),
            secure_fail_mode: false,
        }
    }
}

/// VLAN trunk topology builder.
///
/// Creates a bridge with VLAN-configured access and trunk ports.
///
/// # Example
///
/// ```ignore
/// use rovs_ext::topology::{VlanTrunk, AccessPortConfig, TrunkPortConfig};
///
/// let trunk = VlanTrunk::new("br-vlan")
///     .access_port(AccessPortConfig::new("vm1", 100))
///     .access_port(AccessPortConfig::new("vm2", 100))
///     .access_port(AccessPortConfig::new("vm3", 200))
///     .trunk_port(TrunkPortConfig::new("uplink").allowed_vlans(vec![100, 200]));
///
/// trunk.create(&mut client).await?;
/// ```
#[derive(Debug, Clone)]
pub struct VlanTrunk {
    config: VlanTrunkConfig,
}

impl VlanTrunk {
    /// Create a new VLAN trunk builder.
    #[must_use]
    pub fn new(bridge: impl Into<String>) -> Self {
        Self {
            config: VlanTrunkConfig::new(bridge),
        }
    }

    /// Don't create the bridge (assume it already exists).
    #[must_use]
    pub fn existing_bridge(mut self) -> Self {
        self.config.create_bridge = false;
        self
    }

    /// Add an access port.
    #[must_use]
    pub fn access_port(mut self, port: AccessPortConfig) -> Self {
        self.config.access_ports.push(port);
        self
    }

    /// Add a trunk port.
    #[must_use]
    pub fn trunk_port(mut self, port: TrunkPortConfig) -> Self {
        self.config.trunk_ports.push(port);
        self
    }

    /// Set fail_mode to secure.
    #[must_use]
    pub fn secure_fail_mode(mut self) -> Self {
        self.config.secure_fail_mode = true;
        self
    }

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &VlanTrunkConfig {
        &self.config
    }

    /// Build an OVSDB transaction to create the VLAN trunk.
    #[must_use]
    pub fn build_transaction(&self) -> Transaction {
        let mut txn = Transaction::new("Open_vSwitch");

        // Create bridge if needed
        if self.config.create_bridge {
            txn.create_bridge(&self.config.bridge);
        }

        // Add access ports
        for port in &self.config.access_ports {
            self.add_access_port(&mut txn, port);
        }

        // Add trunk ports
        for port in &self.config.trunk_ports {
            self.add_trunk_port(&mut txn, port);
        }

        // Set fail_mode if requested
        if self.config.secure_fail_mode {
            txn.update_by_name(
                "Bridge",
                &self.config.bridge,
                json!({"fail_mode": "secure"}),
            );
        }

        txn
    }

    /// Add an access port to the transaction.
    fn add_access_port(&self, txn: &mut Transaction, port: &AccessPortConfig) {
        // Create interface
        let iface_ref = txn.insert(
            "Interface",
            json!({
                "name": port.name,
                "type": port.iface_type
            }),
        );

        // Create port with VLAN tag
        let port_ref = txn.insert(
            "Port",
            json!({
                "name": port.name,
                "interfaces": iface_ref.to_json(),
                "tag": port.vlan_id
            }),
        );

        // Add port to bridge
        txn.mutate_by_name(
            "Bridge",
            &self.config.bridge,
            vec![json!(["ports", "insert", port_ref.to_json()])],
        );
    }

    /// Add a trunk port to the transaction.
    fn add_trunk_port(&self, txn: &mut Transaction, port: &TrunkPortConfig) {
        // Create interface
        let iface_ref = txn.insert(
            "Interface",
            json!({
                "name": port.name,
                "type": port.iface_type
            }),
        );

        // Create port with trunk configuration
        let port_json = if port.allowed_vlans.is_empty() {
            // All VLANs allowed
            json!({
                "name": port.name,
                "interfaces": iface_ref.to_json(),
                "vlan_mode": "trunk"
            })
        } else {
            // Specific VLANs
            let vlan_set = if port.allowed_vlans.len() == 1 {
                json!(port.allowed_vlans[0])
            } else {
                json!(["set", port.allowed_vlans])
            };
            json!({
                "name": port.name,
                "interfaces": iface_ref.to_json(),
                "vlan_mode": "trunk",
                "trunks": vlan_set
            })
        };

        let port_ref = txn.insert("Port", port_json);

        // Add port to bridge
        txn.mutate_by_name(
            "Bridge",
            &self.config.bridge,
            vec![json!(["ports", "insert", port_ref.to_json()])],
        );
    }

    /// Create the VLAN trunk on an OVSDB client.
    pub async fn create(&self, client: &mut Client) -> Result<()> {
        let mut txn = self.build_transaction();
        client.commit(&mut txn).await?;
        Ok(())
    }

    /// Build an OVSDB transaction to delete the VLAN trunk.
    #[must_use]
    pub fn build_delete_transaction(&self) -> Transaction {
        let mut txn = Transaction::new("Open_vSwitch");

        // Delete all access ports
        for port in &self.config.access_ports {
            txn.delete_port(&self.config.bridge, &port.name);
        }

        // Delete all trunk ports
        for port in &self.config.trunk_ports {
            txn.delete_port(&self.config.bridge, &port.name);
        }

        // Delete bridge if we created it
        if self.config.create_bridge {
            txn.delete_bridge(&self.config.bridge);
        }

        txn
    }

    /// Delete the VLAN trunk from an OVSDB client.
    pub async fn delete(&self, client: &mut Client) -> Result<()> {
        let mut txn = self.build_delete_transaction();
        client.commit(&mut txn).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_transaction_creates_operations() {
        let trunk = VlanTrunk::new("br-vlan")
            .access_port(AccessPortConfig::new("vm1", 100))
            .access_port(AccessPortConfig::new("vm2", 200))
            .trunk_port(TrunkPortConfig::new("uplink").allowed_vlans(vec![100, 200]));

        let txn = trunk.build_transaction();
        assert!(!txn.is_empty());
    }

    #[test]
    fn access_port_config() {
        let port = AccessPortConfig::new("eth0", 100);
        assert_eq!(port.name, "eth0");
        assert_eq!(port.vlan_id, 100);
        assert_eq!(port.iface_type, "internal");
    }

    #[test]
    fn access_port_system() {
        let port = AccessPortConfig::system("eth0", 100);
        assert_eq!(port.iface_type, "system");
    }

    #[test]
    fn trunk_port_config() {
        let port = TrunkPortConfig::new("trunk0").allowed_vlans(vec![100, 200]);
        assert_eq!(port.name, "trunk0");
        assert_eq!(port.allowed_vlans, vec![100, 200]);
    }

    #[test]
    fn existing_bridge_mode() {
        let trunk = VlanTrunk::new("br-vlan").existing_bridge();
        assert!(!trunk.config().create_bridge);
    }
}
