//! Transport layer for OVSDB and `OpenFlow` connections.
//!
//! Provides async stream abstractions over:
//! - Unix domain sockets
//! - TCP sockets
//! - TLS-encrypted TCP connections
//!
//! Also includes reconnection state machine logic.

mod address;
mod error;
mod reconnect;
mod stream;

pub use address::Address;
pub use error::Error;
pub use reconnect::Reconnect;
pub use stream::Stream;

pub type Result<T> = std::result::Result<T, Error>;
