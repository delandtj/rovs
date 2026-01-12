//! OVS topology types (Bridge, Port, Interface).

use uuid::Uuid;

/// An OVS bridge.
#[derive(Debug, Clone)]
pub struct Bridge {
    /// Bridge UUID
    pub uuid: Uuid,
    /// Bridge name
    pub name: String,
    /// Datapath ID
    pub datapath_id: Option<String>,
    /// Datapath type (system, netdev, etc.)
    pub datapath_type: String,
    /// Port UUIDs
    pub ports: Vec<Uuid>,
    /// Fail mode (secure, standalone)
    pub fail_mode: Option<String>,
    /// Whether STP is enabled
    pub stp_enable: bool,
    /// OpenFlow controller addresses
    pub controller: Vec<String>,
}

impl Bridge {
    /// Create a new bridge with just a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            uuid: Uuid::nil(),
            name: name.into(),
            datapath_id: None,
            datapath_type: String::new(),
            ports: Vec::new(),
            fail_mode: None,
            stp_enable: false,
            controller: Vec::new(),
        }
    }
}

/// An OVS port.
#[derive(Debug, Clone)]
pub struct Port {
    /// Port UUID
    pub uuid: Uuid,
    /// Port name
    pub name: String,
    /// Interface UUIDs
    pub interfaces: Vec<Uuid>,
    /// VLAN tag
    pub tag: Option<u16>,
    /// Trunk VLANs
    pub trunks: Vec<u16>,
    /// VLAN mode
    pub vlan_mode: Option<String>,
    /// Bond mode
    pub bond_mode: Option<String>,
    /// LACP mode
    pub lacp: Option<String>,
}

impl Port {
    /// Create a new port with just a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            uuid: Uuid::nil(),
            name: name.into(),
            interfaces: Vec::new(),
            tag: None,
            trunks: Vec::new(),
            vlan_mode: None,
            bond_mode: None,
            lacp: None,
        }
    }
}

/// An OVS interface.
#[derive(Debug, Clone)]
pub struct Interface {
    /// Interface UUID
    pub uuid: Uuid,
    /// Interface name
    pub name: String,
    /// Interface type (internal, system, patch, vxlan, etc.)
    pub iface_type: String,
    /// OpenFlow port number
    pub ofport: Option<i64>,
    /// Admin state (up, down)
    pub admin_state: Option<String>,
    /// Link state (up, down)
    pub link_state: Option<String>,
    /// MAC address
    pub mac_in_use: Option<String>,
    /// MTU
    pub mtu: Option<i64>,
    /// Interface-specific options
    pub options: std::collections::HashMap<String, String>,
}

impl Interface {
    /// Create a new interface with just a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            uuid: Uuid::nil(),
            name: name.into(),
            iface_type: String::new(),
            ofport: None,
            admin_state: None,
            link_state: None,
            mac_in_use: None,
            mtu: None,
            options: std::collections::HashMap::new(),
        }
    }
}
