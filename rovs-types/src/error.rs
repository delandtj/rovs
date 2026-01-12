//! Error types for rovs-types.

use thiserror::Error;

/// Errors that can occur in rovs-types.
#[derive(Debug, Error)]
pub enum Error {
    /// Type conversion error
    #[error("type conversion failed: {0}")]
    TypeConversion(String),

    /// Invalid UUID
    #[error("invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
