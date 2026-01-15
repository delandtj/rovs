// OpenFlow implementation is work-in-progress
#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::unused_async)]
#![allow(clippy::bool_to_int_with_if)]
#![allow(clippy::cast_lossless)]

//! `OpenFlow` protocol implementation for OVS.
//!
//! Provides:
//! - `OpenFlow` message encoding/decoding
//! - Match field builder
//! - Action types
//! - Flow modification
//! - Virtual connection (`VConn`) abstraction

mod action;
mod error;
mod flow;
mod instruction;
mod match_fields;
mod message;
mod multipart;
mod oxm;
mod vconn;

pub use action::{Action, ActionList, LearnSpec, NxLearn, CT_COMMIT};
pub use error::{Error, OfError, OfErrorType};
pub use flow::{Flow, FlowCommand, FlowFlags, FlowStats};
pub use instruction::{Instruction, InstructionList};
pub use match_fields::Match;
pub use message::{Header, Message, MessageType};
pub use multipart::{FlowStatsEntry, FlowStatsRequest, MultipartType};
pub use vconn::VConn;

pub type Result<T> = std::result::Result<T, Error>;

/// OpenFlow protocol versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Version {
    /// OpenFlow 1.0
    Of10 = 0x01,
    /// OpenFlow 1.1
    Of11 = 0x02,
    /// OpenFlow 1.2
    Of12 = 0x03,
    /// OpenFlow 1.3
    Of13 = 0x04,
    /// OpenFlow 1.4
    Of14 = 0x05,
    /// OpenFlow 1.5
    Of15 = 0x06,
}

impl Version {
    /// Get the wire format version number.
    pub fn wire_version(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for Version {
    type Error = Error;

    fn try_from(v: u8) -> Result<Self> {
        match v {
            0x01 => Ok(Self::Of10),
            0x02 => Ok(Self::Of11),
            0x03 => Ok(Self::Of12),
            0x04 => Ok(Self::Of13),
            0x05 => Ok(Self::Of14),
            0x06 => Ok(Self::Of15),
            _ => Err(Error::UnsupportedVersion(v)),
        }
    }
}
