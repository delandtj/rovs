//! OpenFlow error types.

use thiserror::Error;

/// Errors that can occur in OpenFlow operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Transport error
    #[error("transport error: {0}")]
    Transport(#[from] rovs_transport::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Unsupported protocol version
    #[error("unsupported OpenFlow version: {0}")]
    UnsupportedVersion(u8),

    /// Message parsing error
    #[error("parse error: {0}")]
    Parse(String),

    /// Invalid message
    #[error("invalid message: {0}")]
    InvalidMessage(String),

    /// Connection closed
    #[error("connection closed")]
    ConnectionClosed,

    /// Timeout
    #[error("timeout")]
    Timeout,

    /// OpenFlow error from switch
    #[error("OpenFlow error: type={error_type}, code={code}")]
    OfError { error_type: u16, code: u16 },
}
