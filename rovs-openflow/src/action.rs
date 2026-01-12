//! `OpenFlow` actions.

use std::net::Ipv4Addr;

use crate::match_fields::MacAddr;

/// An OpenFlow action.
#[derive(Debug, Clone)]
pub enum Action {
    /// Output to a port
    Output(OutputPort),
    /// Drop the packet (implicit, no action)
    Drop,
    /// Send to controller
    Controller { max_len: u16 },
    /// Set source MAC
    SetEthSrc(MacAddr),
    /// Set destination MAC
    SetEthDst(MacAddr),
    /// Set VLAN ID
    SetVlanVid(u16),
    /// Push VLAN tag
    PushVlan(u16),
    /// Pop VLAN tag
    PopVlan,
    /// Set IPv4 source
    SetIpv4Src(Ipv4Addr),
    /// Set IPv4 destination
    SetIpv4Dst(Ipv4Addr),
    /// Set TCP/UDP source port
    SetTpSrc(u16),
    /// Set TCP/UDP destination port
    SetTpDst(u16),
    /// Set IP TTL
    SetTtl(u8),
    /// Decrement IP TTL
    DecTtl,
    /// Go to table (OF 1.1+)
    GotoTable(u8),
    /// Write metadata
    WriteMetadata { metadata: u64, mask: u64 },
    /// Apply meter
    Meter(u32),
    /// Output to group
    Group(u32),
    /// Set tunnel ID
    SetTunnelId(u64),
    /// Resubmit to table (Nicira extension)
    NxResubmit {
        port: Option<u16>,
        table: Option<u8>,
    },
    /// Learn action (Nicira extension)
    NxLearn {/* TODO: learn spec */},
    /// Conntrack action (Nicira extension)
    NxCt {
        flags: u16,
        zone: u16,
        table: Option<u8>,
    },
}

/// Output port specification.
#[derive(Debug, Clone, Copy)]
pub enum OutputPort {
    /// Physical or logical port number
    Port(u32),
    /// Send to controller
    Controller,
    /// Flood (all ports except input)
    Flood,
    /// All ports except input
    All,
    /// Input port
    InPort,
    /// Local (management) port
    Local,
    /// Normal L2/L3 processing
    Normal,
    /// No output (drop)
    None,
}

impl From<u32> for OutputPort {
    fn from(port: u32) -> Self {
        Self::Port(port)
    }
}

/// A list of actions.
#[derive(Debug, Clone, Default)]
pub struct ActionList {
    actions: Vec<Action>,
}

impl ActionList {
    /// Create a new empty action list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an action to the list.
    pub fn push(&mut self, action: Action) {
        self.actions.push(action);
    }

    /// Output to a port.
    pub fn output(mut self, port: impl Into<OutputPort>) -> Self {
        self.actions.push(Action::Output(port.into()));
        self
    }

    /// Send to controller.
    pub fn controller(mut self, max_len: u16) -> Self {
        self.actions.push(Action::Controller { max_len });
        self
    }

    /// Drop the packet.
    pub fn drop(mut self) -> Self {
        self.actions.push(Action::Drop);
        self
    }

    /// Set destination MAC.
    pub fn set_eth_dst(mut self, mac: MacAddr) -> Self {
        self.actions.push(Action::SetEthDst(mac));
        self
    }

    /// Set source MAC.
    pub fn set_eth_src(mut self, mac: MacAddr) -> Self {
        self.actions.push(Action::SetEthSrc(mac));
        self
    }

    /// Push VLAN tag.
    pub fn push_vlan(mut self, tpid: u16) -> Self {
        self.actions.push(Action::PushVlan(tpid));
        self
    }

    /// Pop VLAN tag.
    pub fn pop_vlan(mut self) -> Self {
        self.actions.push(Action::PopVlan);
        self
    }

    /// Set VLAN ID.
    pub fn set_vlan_vid(mut self, vid: u16) -> Self {
        self.actions.push(Action::SetVlanVid(vid));
        self
    }

    /// Go to another table.
    pub fn goto_table(mut self, table: u8) -> Self {
        self.actions.push(Action::GotoTable(table));
        self
    }

    /// Decrement TTL.
    pub fn dec_ttl(mut self) -> Self {
        self.actions.push(Action::DecTtl);
        self
    }

    /// Get the actions.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Get the number of actions.
    pub fn len(&self) -> usize {
        self.actions.len()
    }
}
