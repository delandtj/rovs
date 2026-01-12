//! JSON-RPC error types.

use thiserror::Error;

use crate::message::RpcError;

/// Errors that can occur in JSON-RPC communication.
#[derive(Debug, Error)]
pub enum Error {
    /// Transport error
    #[error("transport error: {0}")]
    Transport(#[from] rovs_transport::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// RPC error returned by server
    #[error("RPC error: {0}")]
    Rpc(RpcError),

    /// Unexpected response ID
    #[error("unexpected response ID: expected {expected}, got {got}")]
    UnexpectedId { expected: u64, got: u64 },

    /// Connection closed
    #[error("connection closed")]
    ConnectionClosed,
}
