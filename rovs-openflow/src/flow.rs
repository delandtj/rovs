//! OpenFlow flow entries and modifications.

use crate::{ActionList, Match};

/// A flow entry.
#[derive(Debug, Clone)]
pub struct Flow {
    /// Table ID
    pub table_id: u8,
    /// Priority (higher = more specific)
    pub priority: u16,
    /// Cookie (opaque identifier)
    pub cookie: u64,
    /// Match fields
    pub match_fields: Match,
    /// Actions to apply
    pub actions: ActionList,
    /// Idle timeout (seconds, 0 = no timeout)
    pub idle_timeout: u16,
    /// Hard timeout (seconds, 0 = no timeout)
    pub hard_timeout: u16,
    /// Packet count
    pub packet_count: u64,
    /// Byte count
    pub byte_count: u64,
}

/// Flow modification command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FlowModCommand {
    /// Add a new flow
    Add = 0,
    /// Modify matching flows
    Modify = 1,
    /// Modify matching flows (strict match)
    ModifyStrict = 2,
    /// Delete matching flows
    Delete = 3,
    /// Delete matching flows (strict match)
    DeleteStrict = 4,
}

/// Flow modification flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct FlowModFlags {
    /// Send flow removed message
    pub send_flow_rem: bool,
    /// Check for overlapping entries
    pub check_overlap: bool,
    /// Reset flow counters
    pub reset_counts: bool,
    /// Don't keep track of packet count
    pub no_pkt_counts: bool,
    /// Don't keep track of byte count
    pub no_byte_counts: bool,
}

/// A flow modification request.
#[derive(Debug, Clone)]
pub struct FlowMod {
    /// Command (add, modify, delete)
    pub command: FlowModCommand,
    /// Table ID (0xff for all tables on delete)
    pub table_id: u8,
    /// Priority
    pub priority: u16,
    /// Cookie
    pub cookie: u64,
    /// Cookie mask (for modify/delete)
    pub cookie_mask: u64,
    /// Match fields
    pub match_fields: Match,
    /// Actions
    pub actions: ActionList,
    /// Idle timeout
    pub idle_timeout: u16,
    /// Hard timeout
    pub hard_timeout: u16,
    /// Flags
    pub flags: FlowModFlags,
    /// Output port (for delete commands)
    pub out_port: Option<u32>,
    /// Output group (for delete commands)
    pub out_group: Option<u32>,
    /// Buffer ID (for packet-out)
    pub buffer_id: Option<u32>,
}

impl FlowMod {
    /// Create a new flow add command.
    pub fn add() -> Self {
        Self {
            command: FlowModCommand::Add,
            table_id: 0,
            priority: 0,
            cookie: 0,
            cookie_mask: 0,
            match_fields: Match::new(),
            actions: ActionList::new(),
            idle_timeout: 0,
            hard_timeout: 0,
            flags: FlowModFlags::default(),
            out_port: None,
            out_group: None,
            buffer_id: None,
        }
    }

    /// Create a new flow delete command.
    pub fn delete() -> Self {
        Self {
            command: FlowModCommand::Delete,
            table_id: 0xff, // All tables
            priority: 0,
            cookie: 0,
            cookie_mask: 0,
            match_fields: Match::new(),
            actions: ActionList::new(),
            idle_timeout: 0,
            hard_timeout: 0,
            flags: FlowModFlags::default(),
            out_port: None,
            out_group: None,
            buffer_id: None,
        }
    }

    /// Set the table ID.
    pub fn table(mut self, id: u8) -> Self {
        self.table_id = id;
        self
    }

    /// Set the priority.
    pub fn priority(mut self, priority: u16) -> Self {
        self.priority = priority;
        self
    }

    /// Set the cookie.
    pub fn cookie(mut self, cookie: u64) -> Self {
        self.cookie = cookie;
        self
    }

    /// Set the match fields.
    pub fn match_fields(mut self, m: Match) -> Self {
        self.match_fields = m;
        self
    }

    /// Set the actions.
    pub fn actions(mut self, actions: ActionList) -> Self {
        self.actions = actions;
        self
    }

    /// Set the idle timeout.
    pub fn idle_timeout(mut self, timeout: u16) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Set the hard timeout.
    pub fn hard_timeout(mut self, timeout: u16) -> Self {
        self.hard_timeout = timeout;
        self
    }
}
