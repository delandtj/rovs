//! JSON-RPC protocol implementation for OVSDB.
//!
//! Provides message types and connection handling for the JSON-RPC 1.0
//! protocol used by OVSDB (RFC 7047).

mod connection;
mod error;
mod message;

pub use connection::Connection;
pub use error::Error;
pub use message::{Message, Request, Response};

pub type Result<T> = std::result::Result<T, Error>;
