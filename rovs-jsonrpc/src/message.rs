//! JSON-RPC message types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC request/response ID.
///
/// Can be a number, string, or null. OVSDB uses both numeric IDs (for our requests)
/// and string IDs (e.g., "echo" for server-initiated echo requests).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcId {
    /// Numeric ID (used by this client)
    Number(u64),
    /// String ID (used by some server requests like "echo")
    String(String),
}

impl RpcId {
    /// Get as u64 if this is a numeric ID.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::String(_) => None,
        }
    }
}

impl From<u64> for RpcId {
    fn from(n: u64) -> Self {
        Self::Number(n)
    }
}

impl From<&str> for RpcId {
    fn from(s: &str) -> Self {
        Self::String(s.to_owned())
    }
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<RpcId>,
}

impl Request {
    /// Create a new request.
    pub fn new(method: impl Into<String>, params: Value, id: u64) -> Self {
        Self {
            method: method.into(),
            params,
            id: Some(RpcId::Number(id)),
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
    pub id: RpcId,
}

impl Response {
    /// Create a success response.
    pub fn success(id: impl Into<RpcId>, result: Value) -> Self {
        Self {
            result: Some(result),
            error: None,
            id: id.into(),
        }
    }

    /// Create an error response.
    pub fn error(id: impl Into<RpcId>, error: RpcError) -> Self {
        Self {
            result: None,
            error: Some(error),
            id: id.into(),
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
