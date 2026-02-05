//! Unified error type for rovs-ext.

use thiserror::Error;

/// Unified error type for rovs-ext operations.
#[derive(Debug, Error)]
pub enum Error {
    /// OVSDB error.
    #[error("ovsdb error: {0}")]
    Ovsdb(#[from] rovs_ovsdb::Error),

    /// OpenFlow error.
    #[error("openflow error: {0}")]
    OpenFlow(#[from] rovs_openflow::Error),

    /// Transport error.
    #[error("transport error: {0}")]
    Transport(#[from] rovs_transport::Error),

    /// Invalid configuration.
    #[error("invalid configuration: {0}")]
    Config(String),

    /// Port not found.
    #[error("port not found: {0}")]
    PortNotFound(String),

    /// Bridge not found.
    #[error("bridge not found: {0}")]
    BridgeNotFound(String),

    /// Invalid MAC address.
    #[error("invalid MAC address: {0}")]
    InvalidMac(String),

    /// Invalid IP address.
    #[error("invalid IP address: {0}")]
    InvalidIp(String),

    /// Handler error.
    #[error("handler error: {0}")]
    Handler(String),

    /// Parse error.
    #[error("parse error: {0}")]
    Parse(String),

    /// IO error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for rovs-ext operations.
pub type Result<T> = std::result::Result<T, Error>;
