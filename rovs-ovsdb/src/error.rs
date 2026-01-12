//! OVSDB error types.

use thiserror::Error;

/// Errors that can occur in OVSDB operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Transport error
    #[error("transport error: {0}")]
    Transport(#[from] rovs_transport::Error),

    /// JSON-RPC error
    #[error("JSON-RPC error: {0}")]
    JsonRpc(#[from] rovs_jsonrpc::Error),

    /// Schema error
    #[error("schema error: {0}")]
    Schema(String),

    /// Transaction error
    #[error("transaction error: {0}")]
    Transaction(String),

    /// Row not found
    #[error("row not found: {0}")]
    RowNotFound(String),

    /// Table not found
    #[error("table not found: {0}")]
    TableNotFound(String),

    /// Column not found
    #[error("column not found: {0}")]
    ColumnNotFound(String),

    /// Type conversion error
    #[error("type error: {0}")]
    Type(#[from] rovs_types::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Lock contention
    #[error("lock contended")]
    LockContended,

    /// Need to retry transaction
    #[error("transaction conflict, retry needed")]
    TryAgain,
}
