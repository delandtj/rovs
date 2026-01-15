//! `OpenFlow` actions.

use std::net::Ipv4Addr;

use crate::match_fields::MacAddr;
use crate::oxm::{OxmClass, OxmField};

/// OpenFlow action type wire values (OF 1.3+).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ActionType {
    /// Output to switch port
    Output = 0,
    /// Copy TTL out
    CopyTtlOut = 11,
    /// Copy TTL in
    CopyTtlIn = 12,
    /// Set MPLS TTL
    SetMplsTtl = 15,
    /// Decrement MPLS TTL
    DecMplsTtl = 16,
    /// Push VLAN tag
    PushVlan = 17,
    /// Pop VLAN tag
    PopVlan = 18,
    /// Push MPLS label
    PushMpls = 19,
    /// Pop MPLS label
    PopMpls = 20,
    /// Set queue
    SetQueue = 21,
    /// Group action
    Group = 22,
    /// Set IP TTL
    SetNwTtl = 23,
    /// Decrement IP TTL
    DecNwTtl = 24,
    /// Set field using OXM
    SetField = 25,
    /// Push PBB header
    PushPbb = 26,
    /// Pop PBB header
    PopPbb = 27,
    /// Experimenter/vendor action
    Experimenter = 0xffff,
}

impl TryFrom<u16> for ActionType {
    type Error = crate::Error;

    fn try_from(v: u16) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Output),
            11 => Ok(Self::CopyTtlOut),
            12 => Ok(Self::CopyTtlIn),
            15 => Ok(Self::SetMplsTtl),
            16 => Ok(Self::DecMplsTtl),
            17 => Ok(Self::PushVlan),
            18 => Ok(Self::PopVlan),
            19 => Ok(Self::PushMpls),
            20 => Ok(Self::PopMpls),
            21 => Ok(Self::SetQueue),
            22 => Ok(Self::Group),
            23 => Ok(Self::SetNwTtl),
            24 => Ok(Self::DecNwTtl),
            25 => Ok(Self::SetField),
            26 => Ok(Self::PushPbb),
            27 => Ok(Self::PopPbb),
            0xffff => Ok(Self::Experimenter),
            _ => Err(crate::Error::Parse(format!("unknown action type: {v}"))),
        }
    }
}

/// Nicira vendor ID for experimenter actions.
pub const NICIRA_VENDOR_ID: u32 = 0x0000_2320;

/// Nicira action subtypes.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum NxActionSubtype {
    /// Resubmit to table
    Resubmit = 1,
    /// Resubmit to table (extended)
    ResubmitTable = 14,
    /// Move bits between fields
    Move = 6,
    /// Load immediate value into field
    RegLoad = 7,
    /// Connection tracking
    Ct = 35,
    /// Learn action
    Learn = 16,
    /// Set field (Nicira version)
    RegLoad2 = 33,
}

/// Reserved OpenFlow port numbers.
#[allow(dead_code)]
pub mod port {
    /// Maximum valid physical port number
    pub const MAX: u32 = 0xffff_ff00;
    /// Send to controller as packet-in
    pub const CONTROLLER: u32 = 0xffff_fffd;
    /// Submit to first flow table (packet-out only)
    pub const TABLE: u32 = 0xffff_fff9;
    /// Process with normal L2/L3 switching
    pub const NORMAL: u32 = 0xffff_fffa;
    /// All physical ports except input port
    pub const FLOOD: u32 = 0xffff_fffb;
    /// All physical ports except input port
    pub const ALL: u32 = 0xffff_fffc;
    /// Local openflow port
    pub const LOCAL: u32 = 0xffff_fffe;
    /// Not associated with a physical port
    pub const NONE: u32 = 0xffff_ffff;
    /// Send back out input port
    pub const IN_PORT: u32 = 0xffff_fff8;
}

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
    ///
    /// TODO: Add learn_spec fields: idle_timeout, hard_timeout, priority, cookie,
    /// flags, table_id, fin_idle_timeout, fin_hard_timeout, and flow_mod_specs
    /// for field assignments and match criteria.
    NxLearn,
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

    /// Encode all actions to wire format.
    ///
    /// Actions are concatenated and padded to 8-byte alignment.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        for action in &self.actions {
            buf.extend(action.encode());
        }
        // Pad to 8-byte boundary
        let padding = (8 - (buf.len() % 8)) % 8;
        buf.extend(std::iter::repeat_n(0u8, padding));
        buf
    }

    /// Decode all actions from wire format.
    ///
    /// Reads actions until the data is exhausted.
    pub fn decode(data: &[u8]) -> crate::Result<Self> {
        let mut actions = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            // Need at least 4 bytes for action header
            if data.len() - offset < 4 {
                break;
            }

            // Check for zero-length padding at end
            let length = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            if length == 0 {
                break;
            }

            let (action, consumed) = Action::decode(&data[offset..])?;
            // Skip Drop actions (used as placeholder for unsupported types)
            if !matches!(action, Action::Drop) {
                actions.push(action);
            }
            offset += consumed;
        }

        Ok(Self { actions })
    }
}

impl OutputPort {
    /// Convert to wire format port number.
    pub const fn to_wire_port(self) -> u32 {
        match self {
            Self::Port(p) => p,
            Self::Controller => port::CONTROLLER,
            Self::Flood => port::FLOOD,
            Self::All => port::ALL,
            Self::InPort => port::IN_PORT,
            Self::Local => port::LOCAL,
            Self::Normal => port::NORMAL,
            Self::None => port::NONE,
        }
    }

    /// Create from wire format port number.
    pub const fn from_wire(port: u32) -> Self {
        match port {
            port::CONTROLLER => Self::Controller,
            port::FLOOD => Self::Flood,
            port::ALL => Self::All,
            port::IN_PORT => Self::InPort,
            port::LOCAL => Self::Local,
            port::NORMAL => Self::Normal,
            port::NONE => Self::None,
            p => Self::Port(p),
        }
    }
}

impl Action {
    /// Encode action to OpenFlow wire format.
    #[allow(clippy::match_same_arms)]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Output(port) => encode_output(port.to_wire_port(), 0xffff),
            Self::Drop => Vec::new(), // Drop is implicit (no actions)
            Self::Controller { max_len } => encode_output(port::CONTROLLER, *max_len),
            Self::SetEthSrc(mac) => encode_set_field_mac(OxmField::EthSrc, *mac),
            Self::SetEthDst(mac) => encode_set_field_mac(OxmField::EthDst, *mac),
            Self::SetVlanVid(vid) => encode_set_field_u16(OxmField::VlanVid, *vid | 0x1000),
            Self::PushVlan(ethertype) => encode_push_vlan(*ethertype),
            Self::PopVlan => encode_pop_vlan(),
            Self::SetIpv4Src(addr) => encode_set_field_u32(OxmField::Ipv4Src, (*addr).into()),
            Self::SetIpv4Dst(addr) => encode_set_field_u32(OxmField::Ipv4Dst, (*addr).into()),
            Self::SetTpSrc(port) => encode_set_field_u16(OxmField::TcpSrc, *port),
            Self::SetTpDst(port) => encode_set_field_u16(OxmField::TcpDst, *port),
            Self::SetTtl(ttl) => encode_set_nw_ttl(*ttl),
            Self::DecTtl => encode_dec_ttl(),
            Self::GotoTable(_) => Vec::new(), // GotoTable is an instruction, not action
            Self::WriteMetadata { .. } => Vec::new(), // WriteMetadata is an instruction
            Self::Meter(_) => Vec::new(), // Meter is an instruction
            Self::Group(group_id) => encode_group(*group_id),
            Self::SetTunnelId(tun_id) => encode_set_tunnel_id(*tun_id),
            Self::NxResubmit { port, table } => encode_nx_resubmit(*port, *table),
            Self::NxLearn => todo!("NxLearn encoding not implemented"),
            Self::NxCt { flags, zone, table } => encode_nx_ct(*flags, *zone, *table),
        }
    }

    /// Decode action from OpenFlow wire format.
    ///
    /// Returns the decoded action and the number of bytes consumed.
    #[allow(clippy::too_many_lines)]
    pub fn decode(data: &[u8]) -> crate::Result<(Self, usize)> {
        if data.len() < 4 {
            return Err(crate::Error::Parse("action too short".into()));
        }

        let action_type = u16::from_be_bytes([data[0], data[1]]);
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;

        if data.len() < length {
            return Err(crate::Error::Parse("action truncated".into()));
        }

        let action_type = ActionType::try_from(action_type)?;

        let action = match action_type {
            ActionType::Output => {
                if length < 16 {
                    return Err(crate::Error::Parse("output action too short".into()));
                }
                let port = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                let max_len = u16::from_be_bytes([data[8], data[9]]);
                let output_port = OutputPort::from_wire(port);
                if port == port::CONTROLLER {
                    Self::Controller { max_len }
                } else {
                    Self::Output(output_port)
                }
            }
            ActionType::PopVlan => Self::PopVlan,
            ActionType::PushVlan => {
                if length < 8 {
                    return Err(crate::Error::Parse("push_vlan action too short".into()));
                }
                let ethertype = u16::from_be_bytes([data[4], data[5]]);
                Self::PushVlan(ethertype)
            }
            ActionType::DecNwTtl => Self::DecTtl,
            ActionType::SetNwTtl => {
                if length < 8 {
                    return Err(crate::Error::Parse("set_nw_ttl action too short".into()));
                }
                Self::SetTtl(data[4])
            }
            ActionType::Group => {
                if length < 8 {
                    return Err(crate::Error::Parse("group action too short".into()));
                }
                let group_id = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                Self::Group(group_id)
            }
            ActionType::SetField => {
                decode_set_field_action(&data[4..length])?
            }
            ActionType::Experimenter => {
                if length < 10 {
                    return Err(crate::Error::Parse("experimenter action too short".into()));
                }
                let vendor = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                if vendor == NICIRA_VENDOR_ID {
                    decode_nicira_action(&data[8..length])?
                } else {
                    // Unknown vendor, skip
                    return Err(crate::Error::Parse(format!(
                        "unknown experimenter vendor: {vendor:#x}"
                    )));
                }
            }
            // Actions we don't fully decode yet - return a placeholder
            ActionType::CopyTtlOut
            | ActionType::CopyTtlIn
            | ActionType::SetMplsTtl
            | ActionType::DecMplsTtl
            | ActionType::PushMpls
            | ActionType::PopMpls
            | ActionType::SetQueue
            | ActionType::PushPbb
            | ActionType::PopPbb => {
                // Skip unsupported actions by returning Drop as a placeholder
                Self::Drop
            }
        };

        Ok((action, length))
    }
}

// ============================================================================
// Action Encoding Functions
// ============================================================================

/// Encode Output action (16 bytes).
///
/// ```text
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |         type (0)            |          length (16)            |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                            port                               |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |          max_len            |           pad (zeros)           |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                           pad (zeros)                         |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
fn encode_output(port: u32, max_len: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16);
    buf.extend((ActionType::Output as u16).to_be_bytes());
    buf.extend(16u16.to_be_bytes()); // length
    buf.extend(port.to_be_bytes());
    buf.extend(max_len.to_be_bytes());
    buf.extend([0u8; 6]); // padding
    buf
}

/// Encode PopVlan action (8 bytes).
fn encode_pop_vlan() -> Vec<u8> {
    let mut buf = Vec::with_capacity(8);
    buf.extend((ActionType::PopVlan as u16).to_be_bytes());
    buf.extend(8u16.to_be_bytes()); // length
    buf.extend([0u8; 4]); // padding
    buf
}

/// Encode PushVlan action (8 bytes).
fn encode_push_vlan(ethertype: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8);
    buf.extend((ActionType::PushVlan as u16).to_be_bytes());
    buf.extend(8u16.to_be_bytes()); // length
    buf.extend(ethertype.to_be_bytes());
    buf.extend([0u8; 2]); // padding
    buf
}

/// Encode DecNwTtl action (8 bytes).
fn encode_dec_ttl() -> Vec<u8> {
    let mut buf = Vec::with_capacity(8);
    buf.extend((ActionType::DecNwTtl as u16).to_be_bytes());
    buf.extend(8u16.to_be_bytes()); // length
    buf.extend([0u8; 4]); // padding
    buf
}

/// Encode SetNwTtl action (8 bytes).
fn encode_set_nw_ttl(ttl: u8) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8);
    buf.extend((ActionType::SetNwTtl as u16).to_be_bytes());
    buf.extend(8u16.to_be_bytes()); // length
    buf.push(ttl);
    buf.extend([0u8; 3]); // padding
    buf
}

/// Encode Group action (8 bytes).
fn encode_group(group_id: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8);
    buf.extend((ActionType::Group as u16).to_be_bytes());
    buf.extend(8u16.to_be_bytes()); // length
    buf.extend(group_id.to_be_bytes());
    buf
}

// ============================================================================
// SetField Actions
// ============================================================================

/// Encode SetField action for MAC address (16 bytes).
///
/// SetField uses OXM format: action header + OXM header + value + padding.
fn encode_set_field_mac(field: OxmField, mac: [u8; 6]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16);
    buf.extend((ActionType::SetField as u16).to_be_bytes());
    buf.extend(16u16.to_be_bytes()); // length

    // OXM header for MAC field: class=0x8000, field, has_mask=false, length=6
    let oxm_header =
        ((OxmClass::OpenflowBasic as u32) << 16) | ((field as u32) << 9) | 6;
    buf.extend(oxm_header.to_be_bytes());
    buf.extend(mac);
    buf.extend([0u8; 2]); // padding to 16 bytes
    buf
}

/// Encode SetField action for u16 value (16 bytes).
fn encode_set_field_u16(field: OxmField, value: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16);
    buf.extend((ActionType::SetField as u16).to_be_bytes());
    buf.extend(16u16.to_be_bytes()); // length

    // OXM header for u16 field: class=0x8000, field, has_mask=false, length=2
    let oxm_header =
        ((OxmClass::OpenflowBasic as u32) << 16) | ((field as u32) << 9) | 2;
    buf.extend(oxm_header.to_be_bytes());
    buf.extend(value.to_be_bytes());
    buf.extend([0u8; 6]); // padding to 16 bytes
    buf
}

/// Encode SetField action for u32 value (16 bytes).
fn encode_set_field_u32(field: OxmField, value: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16);
    buf.extend((ActionType::SetField as u16).to_be_bytes());
    buf.extend(16u16.to_be_bytes()); // length

    // OXM header for u32 field: class=0x8000, field, has_mask=false, length=4
    let oxm_header =
        ((OxmClass::OpenflowBasic as u32) << 16) | ((field as u32) << 9) | 4;
    buf.extend(oxm_header.to_be_bytes());
    buf.extend(value.to_be_bytes());
    buf.extend([0u8; 4]); // padding to 16 bytes
    buf
}

// ============================================================================
// Action Decoding Functions
// ============================================================================

/// Decode SetField action.
///
/// SetField uses OXM format: OXM header (4 bytes) + value.
fn decode_set_field_action(data: &[u8]) -> crate::Result<Action> {
    if data.len() < 4 {
        return Err(crate::Error::Parse("set_field action too short".into()));
    }

    let oxm_header = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let oxm_class = (oxm_header >> 16) as u16;
    let field = ((oxm_header >> 9) & 0x7f) as u8;
    let length = (oxm_header & 0xff) as usize;

    if data.len() < 4 + length {
        return Err(crate::Error::Parse("set_field value truncated".into()));
    }

    let value = &data[4..4 + length];

    // OpenFlow Basic class
    if oxm_class == OxmClass::OpenflowBasic as u16 {
        match field {
            f if f == OxmField::EthSrc as u8 && length >= 6 => {
                let mut mac = [0u8; 6];
                mac.copy_from_slice(&value[..6]);
                Ok(Action::SetEthSrc(mac))
            }
            f if f == OxmField::EthDst as u8 && length >= 6 => {
                let mut mac = [0u8; 6];
                mac.copy_from_slice(&value[..6]);
                Ok(Action::SetEthDst(mac))
            }
            f if f == OxmField::VlanVid as u8 && length >= 2 => {
                let vid = u16::from_be_bytes([value[0], value[1]]);
                // Remove CFI bit
                Ok(Action::SetVlanVid(vid & 0x0fff))
            }
            f if f == OxmField::Ipv4Src as u8 && length >= 4 => {
                let addr = Ipv4Addr::new(value[0], value[1], value[2], value[3]);
                Ok(Action::SetIpv4Src(addr))
            }
            f if f == OxmField::Ipv4Dst as u8 && length >= 4 => {
                let addr = Ipv4Addr::new(value[0], value[1], value[2], value[3]);
                Ok(Action::SetIpv4Dst(addr))
            }
            f if f == OxmField::TcpSrc as u8 && length >= 2 => {
                let port = u16::from_be_bytes([value[0], value[1]]);
                Ok(Action::SetTpSrc(port))
            }
            f if f == OxmField::TcpDst as u8 && length >= 2 => {
                let port = u16::from_be_bytes([value[0], value[1]]);
                Ok(Action::SetTpDst(port))
            }
            f if f == OxmField::UdpSrc as u8 && length >= 2 => {
                let port = u16::from_be_bytes([value[0], value[1]]);
                Ok(Action::SetTpSrc(port))
            }
            f if f == OxmField::UdpDst as u8 && length >= 2 => {
                let port = u16::from_be_bytes([value[0], value[1]]);
                Ok(Action::SetTpDst(port))
            }
            _ => {
                // Unknown field, return Drop as placeholder
                Ok(Action::Drop)
            }
        }
    } else if oxm_class == OxmClass::Nxm1 as u16 {
        // NXM1 class (Nicira extensions)
        // Field 16 is tunnel ID
        if field == 16 && length >= 8 {
            let tun_id = u64::from_be_bytes([
                value[0], value[1], value[2], value[3],
                value[4], value[5], value[6], value[7],
            ]);
            Ok(Action::SetTunnelId(tun_id))
        } else {
            Ok(Action::Drop)
        }
    } else {
        // Unknown class
        Ok(Action::Drop)
    }
}

/// Decode Nicira experimenter action.
///
/// The vendor ID has already been consumed. Data starts at subtype.
fn decode_nicira_action(data: &[u8]) -> crate::Result<Action> {
    if data.len() < 2 {
        return Err(crate::Error::Parse("nicira action too short".into()));
    }

    let subtype = u16::from_be_bytes([data[0], data[1]]);

    match subtype {
        s if s == NxActionSubtype::ResubmitTable as u16 => {
            // Resubmit: subtype (2) + in_port (2) + table (1) + pad (3)
            if data.len() < 6 {
                return Err(crate::Error::Parse("resubmit action too short".into()));
            }
            let in_port = u16::from_be_bytes([data[2], data[3]]);
            let table = data[4];
            let port = if in_port == 0xfff8 { None } else { Some(in_port) };
            let table = if table == 255 { None } else { Some(table) };
            Ok(Action::NxResubmit { port, table })
        }
        s if s == NxActionSubtype::Resubmit as u16 => {
            // Simple resubmit: subtype (2) + in_port (2)
            if data.len() < 4 {
                return Err(crate::Error::Parse("resubmit action too short".into()));
            }
            let in_port = u16::from_be_bytes([data[2], data[3]]);
            let port = if in_port == 0xfff8 { None } else { Some(in_port) };
            Ok(Action::NxResubmit { port, table: None })
        }
        s if s == NxActionSubtype::Ct as u16 => {
            // CT: subtype (2) + flags (2) + zone_src (4) + zone (2) + recirc_table (1) + ...
            if data.len() < 10 {
                return Err(crate::Error::Parse("ct action too short".into()));
            }
            let flags = u16::from_be_bytes([data[2], data[3]]);
            // zone_src at data[4..8]
            let zone = u16::from_be_bytes([data[8], data[9]]);
            let recirc_table = if data.len() > 10 { data[10] } else { 255 };
            let table = if recirc_table == 255 { None } else { Some(recirc_table) };
            Ok(Action::NxCt { flags, zone, table })
        }
        s if s == NxActionSubtype::RegLoad2 as u16 => {
            // RegLoad2: subtype (2) + OXM header (4) + value
            if data.len() < 6 {
                return Err(crate::Error::Parse("reg_load2 action too short".into()));
            }
            let oxm_header = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);
            let oxm_class = (oxm_header >> 16) as u16;
            let field = ((oxm_header >> 9) & 0x7f) as u8;
            let length = (oxm_header & 0xff) as usize;

            if data.len() < 6 + length {
                return Err(crate::Error::Parse("reg_load2 value truncated".into()));
            }

            let value = &data[6..6 + length];

            // NXM1 class, field 16 = tunnel ID
            if oxm_class == OxmClass::Nxm1 as u16 && field == 16 && length >= 8 {
                let tun_id = u64::from_be_bytes([
                    value[0], value[1], value[2], value[3],
                    value[4], value[5], value[6], value[7],
                ]);
                Ok(Action::SetTunnelId(tun_id))
            } else {
                Ok(Action::Drop)
            }
        }
        _ => {
            // Unknown Nicira subtype
            Ok(Action::Drop)
        }
    }
}

// ============================================================================
// Nicira Extension Actions
// ============================================================================

/// Encode Nicira action header.
fn encode_nx_header(subtype: NxActionSubtype, len: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16);
    buf.extend((ActionType::Experimenter as u16).to_be_bytes());
    buf.extend(len.to_be_bytes());
    buf.extend(NICIRA_VENDOR_ID.to_be_bytes());
    buf.extend((subtype as u16).to_be_bytes());
    buf
}

/// Encode SetTunnelId as Nicira reg_load2 action (24 bytes).
fn encode_set_tunnel_id(tun_id: u64) -> Vec<u8> {
    // Use NXM reg_load2 (subtype 33) for setting tunnel ID
    // Format: NX header (10) + OXM header (4) + value (8) + pad (2) = 24 bytes
    let mut buf = encode_nx_header(NxActionSubtype::RegLoad2, 24);

    // OXM header for tun_id: NXM_NX_TUN_ID (class=1, field=16, len=8)
    let oxm_header: u32 = (1 << 16) | (16 << 9) | 8;
    buf.extend(oxm_header.to_be_bytes());
    buf.extend(tun_id.to_be_bytes());
    buf.extend([0u8; 2]); // padding to 24 bytes
    buf
}

/// Encode NxResubmit action (16 bytes for extended resubmit).
fn encode_nx_resubmit(in_port: Option<u16>, table: Option<u8>) -> Vec<u8> {
    // Use extended resubmit (subtype 14) which supports table
    let mut buf = encode_nx_header(NxActionSubtype::ResubmitTable, 16);
    buf.extend(in_port.unwrap_or(0xfff8).to_be_bytes()); // OFPP_IN_PORT = 0xfff8 (16-bit)
    buf.push(table.unwrap_or(255)); // 255 = current table
    buf.extend([0u8; 3]); // padding
    buf
}

/// Encode NxCt (connection tracking) action.
fn encode_nx_ct(flags: u16, zone: u16, table: Option<u8>) -> Vec<u8> {
    // CT action format (24 bytes minimum):
    // NX header (10) + flags (2) + zone_src (4) + zone (2) + recirc_table (1) + pad (3) + alg (2)
    let mut buf = encode_nx_header(NxActionSubtype::Ct, 24);
    buf.extend(flags.to_be_bytes());
    buf.extend(0u32.to_be_bytes()); // zone_src (0 = use zone_imm field)
    buf.extend(zone.to_be_bytes()); // zone_imm
    buf.push(table.unwrap_or(255)); // recirc_table (255 = no recirculation)
    buf.extend([0u8; 3]); // pad (3 bytes, not 1)
    buf.extend(0u16.to_be_bytes()); // alg (0 = no ALG)
    // No nested actions for now
    buf
}

/// Encode NxRegLoad action for loading immediate value into register.
///
/// Format: `load:value->NXM_NX_REGn[start..end]`
#[allow(dead_code)]
pub fn encode_nx_reg_load(reg_num: u8, value: u32, start_bit: u8, n_bits: u8) -> Vec<u8> {
    // reg_load uses subtype 7
    // Format: NX header (10) + ofs_nbits (2) + dst (4) + value (8)
    let mut buf = encode_nx_header(NxActionSubtype::RegLoad, 24);

    // ofs_nbits: (start_bit << 6) | (n_bits - 1)
    let ofs_nbits = ((start_bit as u16) << 6) | ((n_bits - 1) as u16);
    buf.extend(ofs_nbits.to_be_bytes());

    // dst: NXM header for register (class=1, field=reg_num, len=4)
    let dst_header: u32 = (1 << 16) | ((reg_num as u32) << 9) | 4;
    buf.extend(dst_header.to_be_bytes());

    // value: 64-bit value (upper bits zero)
    buf.extend((value as u64).to_be_bytes());
    buf
}

/// Encode NxMove action for copying bits between fields.
///
/// Format: `move:src[start..end]->dst[start..end]`
#[allow(dead_code)]
pub fn encode_nx_move(
    src_field: u32, // NXM header of source field
    dst_field: u32, // NXM header of destination field
    n_bits: u16,
    src_ofs: u16,
    dst_ofs: u16,
) -> Vec<u8> {
    // move uses subtype 6
    // Format: NX header (10) + n_bits (2) + src_ofs (2) + dst_ofs (2) + src (4) + dst (4)
    let mut buf = encode_nx_header(NxActionSubtype::Move, 24);
    buf.extend(n_bits.to_be_bytes());
    buf.extend(src_ofs.to_be_bytes());
    buf.extend(dst_ofs.to_be_bytes());
    buf.extend(src_field.to_be_bytes());
    buf.extend(dst_field.to_be_bytes());
    buf
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_type_wire_values() {
        assert_eq!(ActionType::Output as u16, 0);
        assert_eq!(ActionType::PushVlan as u16, 17);
        assert_eq!(ActionType::PopVlan as u16, 18);
        assert_eq!(ActionType::Group as u16, 22);
        assert_eq!(ActionType::DecNwTtl as u16, 24);
        assert_eq!(ActionType::SetField as u16, 25);
        assert_eq!(ActionType::Experimenter as u16, 0xffff);
    }

    #[test]
    fn nx_action_subtype_values() {
        assert_eq!(NxActionSubtype::Resubmit as u16, 1);
        assert_eq!(NxActionSubtype::Move as u16, 6);
        assert_eq!(NxActionSubtype::RegLoad as u16, 7);
        assert_eq!(NxActionSubtype::ResubmitTable as u16, 14);
        assert_eq!(NxActionSubtype::Ct as u16, 35);
    }

    #[test]
    fn encode_output_port_1() {
        let bytes = encode_output(1, 0xffff);
        assert_eq!(bytes.len(), 16);
        // type = 0
        assert_eq!(&bytes[0..2], &[0x00, 0x00]);
        // length = 16
        assert_eq!(&bytes[2..4], &[0x00, 0x10]);
        // port = 1
        assert_eq!(&bytes[4..8], &[0x00, 0x00, 0x00, 0x01]);
        // max_len = 0xffff
        assert_eq!(&bytes[8..10], &[0xff, 0xff]);
        // padding
        assert_eq!(&bytes[10..16], &[0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn encode_output_controller() {
        let bytes = encode_output(port::CONTROLLER, 128);
        assert_eq!(bytes.len(), 16);
        // port = CONTROLLER (0xfffffffd)
        assert_eq!(&bytes[4..8], &[0xff, 0xff, 0xff, 0xfd]);
        // max_len = 128
        assert_eq!(&bytes[8..10], &[0x00, 0x80]);
    }

    #[test]
    fn encode_pop_vlan_action() {
        let bytes = encode_pop_vlan();
        assert_eq!(bytes.len(), 8);
        // type = 18
        assert_eq!(&bytes[0..2], &[0x00, 0x12]);
        // length = 8
        assert_eq!(&bytes[2..4], &[0x00, 0x08]);
        // padding
        assert_eq!(&bytes[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn encode_push_vlan_8021q() {
        let bytes = encode_push_vlan(0x8100);
        assert_eq!(bytes.len(), 8);
        // type = 17
        assert_eq!(&bytes[0..2], &[0x00, 0x11]);
        // length = 8
        assert_eq!(&bytes[2..4], &[0x00, 0x08]);
        // ethertype = 0x8100
        assert_eq!(&bytes[4..6], &[0x81, 0x00]);
        // padding
        assert_eq!(&bytes[6..8], &[0, 0]);
    }

    #[test]
    fn encode_dec_ttl_action() {
        let bytes = encode_dec_ttl();
        assert_eq!(bytes.len(), 8);
        // type = 24
        assert_eq!(&bytes[0..2], &[0x00, 0x18]);
        // length = 8
        assert_eq!(&bytes[2..4], &[0x00, 0x08]);
    }

    #[test]
    fn encode_set_nw_ttl_action() {
        let bytes = encode_set_nw_ttl(64);
        assert_eq!(bytes.len(), 8);
        // type = 23
        assert_eq!(&bytes[0..2], &[0x00, 0x17]);
        // length = 8
        assert_eq!(&bytes[2..4], &[0x00, 0x08]);
        // ttl = 64
        assert_eq!(bytes[4], 64);
    }

    #[test]
    fn encode_group_action() {
        let bytes = encode_group(100);
        assert_eq!(bytes.len(), 8);
        // type = 22
        assert_eq!(&bytes[0..2], &[0x00, 0x16]);
        // length = 8
        assert_eq!(&bytes[2..4], &[0x00, 0x08]);
        // group_id = 100
        assert_eq!(&bytes[4..8], &[0x00, 0x00, 0x00, 0x64]);
    }

    #[test]
    fn encode_set_field_mac_eth_dst() {
        let mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let bytes = encode_set_field_mac(OxmField::EthDst, mac);
        assert_eq!(bytes.len(), 16);
        // type = 25 (SetField)
        assert_eq!(&bytes[0..2], &[0x00, 0x19]);
        // length = 16
        assert_eq!(&bytes[2..4], &[0x00, 0x10]);
        // OXM header: class=0x8000, field=3 (EthDst), has_mask=0, length=6
        // = 0x8000_0606 = (0x8000 << 16) | (3 << 9) | 6
        let expected_oxm: u32 = (0x8000 << 16) | (3 << 9) | 6;
        assert_eq!(
            &bytes[4..8],
            &expected_oxm.to_be_bytes(),
            "OXM header mismatch"
        );
        // MAC address
        assert_eq!(&bytes[8..14], &mac);
        // padding
        assert_eq!(&bytes[14..16], &[0, 0]);
    }

    #[test]
    fn encode_set_field_u32_ipv4_dst() {
        let addr: u32 = 0x0a000001; // 10.0.0.1
        let bytes = encode_set_field_u32(OxmField::Ipv4Dst, addr);
        assert_eq!(bytes.len(), 16);
        // type = 25 (SetField)
        assert_eq!(&bytes[0..2], &[0x00, 0x19]);
        // OXM header: class=0x8000, field=12 (Ipv4Dst), has_mask=0, length=4
        let expected_oxm: u32 = (0x8000 << 16) | (12 << 9) | 4;
        assert_eq!(&bytes[4..8], &expected_oxm.to_be_bytes());
        // IPv4 address
        assert_eq!(&bytes[8..12], &[0x0a, 0x00, 0x00, 0x01]);
    }

    #[test]
    fn encode_set_field_u16_vlan_vid() {
        // VLAN VID has CFI bit set (0x1000)
        let vid = 100 | 0x1000;
        let bytes = encode_set_field_u16(OxmField::VlanVid, vid);
        assert_eq!(bytes.len(), 16);
        // OXM header: class=0x8000, field=6 (VlanVid), has_mask=0, length=2
        let expected_oxm: u32 = (0x8000 << 16) | (6 << 9) | 2;
        assert_eq!(&bytes[4..8], &expected_oxm.to_be_bytes());
        // VLAN VID with CFI
        assert_eq!(&bytes[8..10], &[0x10, 0x64]);
    }

    #[test]
    fn encode_nx_resubmit_table() {
        let bytes = encode_nx_resubmit(None, Some(10));
        assert_eq!(bytes.len(), 16);
        // type = 0xffff (Experimenter)
        assert_eq!(&bytes[0..2], &[0xff, 0xff]);
        // length = 16
        assert_eq!(&bytes[2..4], &[0x00, 0x10]);
        // vendor = NICIRA (0x00002320)
        assert_eq!(&bytes[4..8], &[0x00, 0x00, 0x23, 0x20]);
        // subtype = 14 (ResubmitTable)
        assert_eq!(&bytes[8..10], &[0x00, 0x0e]);
        // in_port = 0xfff8 (IN_PORT)
        assert_eq!(&bytes[10..12], &[0xff, 0xf8]);
        // table = 10
        assert_eq!(bytes[12], 10);
    }

    #[test]
    fn encode_nx_ct_action() {
        let bytes = encode_nx_ct(0x01, 100, Some(5));
        assert_eq!(bytes.len(), 24);
        // type = 0xffff (Experimenter)
        assert_eq!(&bytes[0..2], &[0xff, 0xff]);
        // length = 24
        assert_eq!(&bytes[2..4], &[0x00, 0x18]);
        // vendor = NICIRA
        assert_eq!(&bytes[4..8], &[0x00, 0x00, 0x23, 0x20]);
        // subtype = 35 (Ct)
        assert_eq!(&bytes[8..10], &[0x00, 0x23]);
        // flags = 0x01
        assert_eq!(&bytes[10..12], &[0x00, 0x01]);
        // zone_src = 0 (4 bytes)
        assert_eq!(&bytes[12..16], &[0x00, 0x00, 0x00, 0x00]);
        // zone = 100
        assert_eq!(&bytes[16..18], &[0x00, 0x64]);
        // recirc_table = 5
        assert_eq!(bytes[18], 5);
        // alg = 0
        assert_eq!(&bytes[22..24], &[0x00, 0x00]);
    }

    #[test]
    fn encode_set_tunnel_id_action() {
        let bytes = encode_set_tunnel_id(0x1234);
        assert_eq!(bytes.len(), 24);
        // type = 0xffff (Experimenter)
        assert_eq!(&bytes[0..2], &[0xff, 0xff]);
        // vendor = NICIRA
        assert_eq!(&bytes[4..8], &[0x00, 0x00, 0x23, 0x20]);
        // subtype = 33 (RegLoad2)
        assert_eq!(&bytes[8..10], &[0x00, 0x21]);
    }

    #[test]
    fn encode_nx_reg_load_reg0() {
        let bytes = encode_nx_reg_load(0, 0x12345678, 0, 32);
        assert_eq!(bytes.len(), 24);
        // type = 0xffff
        assert_eq!(&bytes[0..2], &[0xff, 0xff]);
        // length = 24
        assert_eq!(&bytes[2..4], &[0x00, 0x18]);
        // vendor = NICIRA
        assert_eq!(&bytes[4..8], &[0x00, 0x00, 0x23, 0x20]);
        // subtype = 7 (RegLoad)
        assert_eq!(&bytes[8..10], &[0x00, 0x07]);
        // ofs_nbits = (0 << 6) | 31 = 31
        assert_eq!(&bytes[10..12], &[0x00, 0x1f]);
    }

    #[test]
    fn encode_nx_move_eth_src_to_reg() {
        // NXM headers: EthSrc = 0x80000406, Reg0 = 0x00010004
        let src = (0x8000 << 16) | (2 << 9) | 6; // EthSrc
        let dst = (1 << 16) | (0 << 9) | 4; // NXM_NX_REG0
        let bytes = encode_nx_move(src, dst, 32, 0, 0);
        assert_eq!(bytes.len(), 24);
        // subtype = 6 (Move)
        assert_eq!(&bytes[8..10], &[0x00, 0x06]);
        // n_bits = 32
        assert_eq!(&bytes[10..12], &[0x00, 0x20]);
    }

    #[test]
    fn action_output_encode() {
        let action = Action::Output(OutputPort::Port(1));
        let bytes = action.encode();
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[4..8], &[0x00, 0x00, 0x00, 0x01]);
    }

    #[test]
    fn action_controller_encode() {
        let action = Action::Controller { max_len: 65535 };
        let bytes = action.encode();
        assert_eq!(bytes.len(), 16);
        // port = CONTROLLER
        assert_eq!(&bytes[4..8], &[0xff, 0xff, 0xff, 0xfd]);
        // max_len = 65535
        assert_eq!(&bytes[8..10], &[0xff, 0xff]);
    }

    #[test]
    fn action_drop_encode_empty() {
        let action = Action::Drop;
        let bytes = action.encode();
        assert!(bytes.is_empty()); // Drop produces no bytes
    }

    #[test]
    fn action_pop_vlan_encode() {
        let action = Action::PopVlan;
        let bytes = action.encode();
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..2], &[0x00, 0x12]); // type=18
    }

    #[test]
    fn action_push_vlan_encode() {
        let action = Action::PushVlan(0x8100);
        let bytes = action.encode();
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..2], &[0x00, 0x11]); // type=17
        assert_eq!(&bytes[4..6], &[0x81, 0x00]); // ethertype
    }

    #[test]
    fn action_dec_ttl_encode() {
        let action = Action::DecTtl;
        let bytes = action.encode();
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..2], &[0x00, 0x18]); // type=24
    }

    #[test]
    fn action_group_encode() {
        let action = Action::Group(42);
        let bytes = action.encode();
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..2], &[0x00, 0x16]); // type=22
        assert_eq!(&bytes[4..8], &[0x00, 0x00, 0x00, 0x2a]); // group_id=42
    }

    #[test]
    fn action_set_eth_dst_encode() {
        let mac = MacAddr::from([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        let action = Action::SetEthDst(mac);
        let bytes = action.encode();
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[0..2], &[0x00, 0x19]); // SetField
        assert_eq!(&bytes[8..14], &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    }

    #[test]
    fn action_set_ipv4_dst_encode() {
        let addr: Ipv4Addr = "192.168.1.1".parse().unwrap();
        let action = Action::SetIpv4Dst(addr);
        let bytes = action.encode();
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[0..2], &[0x00, 0x19]); // SetField
        assert_eq!(&bytes[8..12], &[192, 168, 1, 1]);
    }

    #[test]
    fn action_list_encode_multiple() {
        let list = ActionList::new()
            .pop_vlan()
            .output(OutputPort::Port(2));
        let bytes = list.encode();
        // PopVlan (8) + Output (16) = 24 bytes (already 8-byte aligned)
        assert_eq!(bytes.len(), 24);
        // First action: PopVlan
        assert_eq!(&bytes[0..2], &[0x00, 0x12]);
        // Second action: Output
        assert_eq!(&bytes[8..10], &[0x00, 0x00]);
        assert_eq!(&bytes[12..16], &[0x00, 0x00, 0x00, 0x02]);
    }

    #[test]
    fn action_list_encode_empty() {
        let list = ActionList::new();
        let bytes = list.encode();
        assert!(bytes.is_empty());
    }

    #[test]
    fn action_list_encode_padding() {
        // Just dec_ttl (8 bytes) should be 8-byte aligned already
        let list = ActionList::new().dec_ttl();
        let bytes = list.encode();
        assert_eq!(bytes.len(), 8);
        assert_eq!(bytes.len() % 8, 0);
    }

    #[test]
    fn output_port_to_wire() {
        assert_eq!(OutputPort::Port(1).to_wire_port(), 1);
        assert_eq!(OutputPort::Controller.to_wire_port(), port::CONTROLLER);
        assert_eq!(OutputPort::Flood.to_wire_port(), port::FLOOD);
        assert_eq!(OutputPort::All.to_wire_port(), port::ALL);
        assert_eq!(OutputPort::InPort.to_wire_port(), port::IN_PORT);
        assert_eq!(OutputPort::Local.to_wire_port(), port::LOCAL);
        assert_eq!(OutputPort::Normal.to_wire_port(), port::NORMAL);
        assert_eq!(OutputPort::None.to_wire_port(), port::NONE);
    }

    // ========================================================================
    // Decode tests
    // ========================================================================

    #[test]
    fn decode_output_action() {
        let action = Action::Output(OutputPort::Port(5));
        let encoded = action.encode();
        let (decoded, len) = Action::decode(&encoded).unwrap();
        assert_eq!(len, 16);
        match decoded {
            Action::Output(port) => assert_eq!(port.to_wire_port(), 5),
            _ => panic!("expected Output action"),
        }
    }

    #[test]
    fn decode_controller_action() {
        let action = Action::Controller { max_len: 128 };
        let encoded = action.encode();
        let (decoded, len) = Action::decode(&encoded).unwrap();
        assert_eq!(len, 16);
        match decoded {
            Action::Controller { max_len } => assert_eq!(max_len, 128),
            _ => panic!("expected Controller action"),
        }
    }

    #[test]
    fn decode_pop_vlan_action() {
        let action = Action::PopVlan;
        let encoded = action.encode();
        let (decoded, len) = Action::decode(&encoded).unwrap();
        assert_eq!(len, 8);
        assert!(matches!(decoded, Action::PopVlan));
    }

    #[test]
    fn decode_push_vlan_action() {
        let action = Action::PushVlan(0x8100);
        let encoded = action.encode();
        let (decoded, len) = Action::decode(&encoded).unwrap();
        assert_eq!(len, 8);
        match decoded {
            Action::PushVlan(ethertype) => assert_eq!(ethertype, 0x8100),
            _ => panic!("expected PushVlan action"),
        }
    }

    #[test]
    fn decode_dec_ttl_action() {
        let action = Action::DecTtl;
        let encoded = action.encode();
        let (decoded, len) = Action::decode(&encoded).unwrap();
        assert_eq!(len, 8);
        assert!(matches!(decoded, Action::DecTtl));
    }

    #[test]
    fn decode_set_ttl_action() {
        let action = Action::SetTtl(64);
        let encoded = action.encode();
        let (decoded, len) = Action::decode(&encoded).unwrap();
        assert_eq!(len, 8);
        match decoded {
            Action::SetTtl(ttl) => assert_eq!(ttl, 64),
            _ => panic!("expected SetTtl action"),
        }
    }

    #[test]
    fn decode_group_action() {
        let action = Action::Group(42);
        let encoded = action.encode();
        let (decoded, len) = Action::decode(&encoded).unwrap();
        assert_eq!(len, 8);
        match decoded {
            Action::Group(group_id) => assert_eq!(group_id, 42),
            _ => panic!("expected Group action"),
        }
    }

    #[test]
    fn decode_set_eth_dst_action() {
        let mac = [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
        let action = Action::SetEthDst(mac);
        let encoded = action.encode();
        let (decoded, _) = Action::decode(&encoded).unwrap();
        match decoded {
            Action::SetEthDst(m) => assert_eq!(m, mac),
            _ => panic!("expected SetEthDst action"),
        }
    }

    #[test]
    fn decode_set_ipv4_dst_action() {
        let addr: Ipv4Addr = "10.0.0.1".parse().unwrap();
        let action = Action::SetIpv4Dst(addr);
        let encoded = action.encode();
        let (decoded, _) = Action::decode(&encoded).unwrap();
        match decoded {
            Action::SetIpv4Dst(a) => assert_eq!(a, addr),
            _ => panic!("expected SetIpv4Dst action"),
        }
    }

    #[test]
    fn decode_set_vlan_vid_action() {
        let action = Action::SetVlanVid(100);
        let encoded = action.encode();
        let (decoded, _) = Action::decode(&encoded).unwrap();
        match decoded {
            Action::SetVlanVid(vid) => assert_eq!(vid, 100),
            _ => panic!("expected SetVlanVid action"),
        }
    }

    #[test]
    fn decode_nx_resubmit_action() {
        let action = Action::NxResubmit { port: Some(1), table: Some(10) };
        let encoded = action.encode();
        let (decoded, _) = Action::decode(&encoded).unwrap();
        match decoded {
            Action::NxResubmit { port, table } => {
                assert_eq!(port, Some(1));
                assert_eq!(table, Some(10));
            }
            _ => panic!("expected NxResubmit action"),
        }
    }

    #[test]
    fn decode_nx_ct_action() {
        let action = Action::NxCt { flags: 0x01, zone: 100, table: Some(5) };
        let encoded = action.encode();
        let (decoded, _) = Action::decode(&encoded).unwrap();
        match decoded {
            Action::NxCt { flags, zone, table } => {
                assert_eq!(flags, 0x01);
                assert_eq!(zone, 100);
                assert_eq!(table, Some(5));
            }
            _ => panic!("expected NxCt action"),
        }
    }

    #[test]
    fn decode_set_tunnel_id_action() {
        let action = Action::SetTunnelId(0x1234567890);
        let encoded = action.encode();
        let (decoded, _) = Action::decode(&encoded).unwrap();
        match decoded {
            Action::SetTunnelId(tun_id) => assert_eq!(tun_id, 0x1234567890),
            _ => panic!("expected SetTunnelId action"),
        }
    }

    #[test]
    fn decode_action_list_multiple() {
        let list = ActionList::new()
            .pop_vlan()
            .output(OutputPort::Port(2))
            .dec_ttl();
        let encoded = list.encode();
        let decoded = ActionList::decode(&encoded).unwrap();
        assert_eq!(decoded.len(), 3);
        assert!(matches!(decoded.actions()[0], Action::PopVlan));
        assert!(matches!(decoded.actions()[1], Action::Output(_)));
        assert!(matches!(decoded.actions()[2], Action::DecTtl));
    }

    #[test]
    fn decode_action_list_empty() {
        let list = ActionList::new();
        let encoded = list.encode();
        let decoded = ActionList::decode(&encoded).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn roundtrip_action_list() {
        let original = ActionList::new()
            .push_vlan(0x8100)
            .set_vlan_vid(100)
            .output(OutputPort::Port(3));
        let encoded = original.encode();
        let decoded = ActionList::decode(&encoded).unwrap();

        assert_eq!(decoded.len(), 3);
        match &decoded.actions()[0] {
            Action::PushVlan(ethertype) => assert_eq!(*ethertype, 0x8100),
            _ => panic!("expected PushVlan"),
        }
        match &decoded.actions()[1] {
            Action::SetVlanVid(vid) => assert_eq!(*vid, 100),
            _ => panic!("expected SetVlanVid"),
        }
        match &decoded.actions()[2] {
            Action::Output(port) => assert_eq!(port.to_wire_port(), 3),
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn output_port_from_wire() {
        assert_eq!(OutputPort::from_wire(1).to_wire_port(), 1);
        assert_eq!(OutputPort::from_wire(port::CONTROLLER).to_wire_port(), port::CONTROLLER);
        assert!(matches!(OutputPort::from_wire(port::FLOOD), OutputPort::Flood));
        assert!(matches!(OutputPort::from_wire(port::ALL), OutputPort::All));
        assert!(matches!(OutputPort::from_wire(port::IN_PORT), OutputPort::InPort));
        assert!(matches!(OutputPort::from_wire(port::LOCAL), OutputPort::Local));
        assert!(matches!(OutputPort::from_wire(port::NORMAL), OutputPort::Normal));
        assert!(matches!(OutputPort::from_wire(port::NONE), OutputPort::None));
    }

    #[test]
    fn action_type_try_from() {
        assert_eq!(ActionType::try_from(0).unwrap(), ActionType::Output);
        assert_eq!(ActionType::try_from(17).unwrap(), ActionType::PushVlan);
        assert_eq!(ActionType::try_from(18).unwrap(), ActionType::PopVlan);
        assert_eq!(ActionType::try_from(22).unwrap(), ActionType::Group);
        assert_eq!(ActionType::try_from(24).unwrap(), ActionType::DecNwTtl);
        assert_eq!(ActionType::try_from(25).unwrap(), ActionType::SetField);
        assert_eq!(ActionType::try_from(0xffff).unwrap(), ActionType::Experimenter);
        assert!(ActionType::try_from(99).is_err());
    }
}
