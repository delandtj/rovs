//! JSON-RPC message types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A JSON-RPC message (request, response, or notification).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    /// A request expecting a response
    Request(Request),
    /// A response to a request
    Response(Response),
}

/// A JSON-RPC request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// The method name
    pub method: String,
    /// The parameters
    pub params: Value,
    /// The request ID (None for notifications)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
}

impl Request {
    /// Create a new request.
    pub fn new(method: impl Into<String>, params: Value, id: u64) -> Self {
        Self {
            method: method.into(),
            params,
            id: Some(id),
        }
    }

    /// Create a notification (no response expected).
    pub fn notification(method: impl Into<String>, params: Value) -> Self {
        Self {
            method: method.into(),
            params,
            id: None,
        }
    }

    /// Check if this is a notification.
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// A JSON-RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// The result (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// The error (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    /// The request ID this responds to
    pub id: u64,
}

impl Response {
    /// Create a success response.
    pub fn success(id: u64, result: Value) -> Self {
        Self {
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response.
    pub fn error(id: u64, error: RpcError) -> Self {
        Self {
            result: None,
            error: Some(error),
            id,
        }
    }

    /// Check if this response is an error.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// A JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Error message
    pub error: String,
    /// Optional additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)?;
        if let Some(details) = &self.details {
            write!(f, ": {details}")?;
        }
        Ok(())
    }
}
