//! Client error types.

use thiserror::Error;

/// Errors that can occur in the OVS client.
#[derive(Debug, Error)]
pub enum Error {
    /// Transport error
    #[error("transport error: {0}")]
    Transport(#[from] rovs_transport::Error),

    /// OVSDB error
    #[error("OVSDB error: {0}")]
    Ovsdb(#[from] rovs_ovsdb::Error),

    /// OpenFlow error
    #[error("OpenFlow error: {0}")]
    OpenFlow(#[from] rovs_openflow::Error),

    /// Bridge not found
    #[error("bridge not found: {0}")]
    BridgeNotFound(String),

    /// Port not found
    #[error("port not found: {0}")]
    PortNotFound(String),

    /// Not connected
    #[error("not connected")]
    NotConnected,

    /// Operation failed
    #[error("operation failed: {0}")]
    OperationFailed(String),
}
