//! Transport layer errors.

use thiserror::Error;

/// Errors that can occur in the transport layer.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid address format
    #[error("invalid address: {0}")]
    InvalidAddress(String),

    /// TLS error
    #[error("TLS error: {0}")]
    Tls(String),

    /// Connection closed
    #[error("connection closed")]
    ConnectionClosed,

    /// Connection timeout
    #[error("connection timeout")]
    Timeout,
}
