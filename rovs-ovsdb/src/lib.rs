//! OVSDB client implementation.
//!
//! Provides:
//! - `Client` - High-level OVSDB client with schema fetch and monitoring
//! - `Idl` - In-memory replica of OVSDB tables
//! - `Transaction` - ACID transaction builder
//! - `Schema` - Database schema parsing
//! - Row and table abstractions
//!
//! # Example
//!
//! ```ignore
//! use rovs_ovsdb::{Client, ClientConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect with default config (Open_vSwitch database)
//!     let mut client = Client::connect("unix:/var/run/openvswitch/db.sock").await?;
//!
//!     // Access the IDL to read data
//!     for row in client.idl().rows("Bridge") {
//!         println!("Bridge: {:?}", row.get_string("name"));
//!     }
//!
//!     // Wait for updates
//!     loop {
//!         client.wait().await?;
//!         println!("Got update, seqno: {}", client.idl().change_seqno());
//!     }
//! }
//! ```

mod client;
mod error;
mod idl;
mod row;
mod schema;
mod transaction;

pub use client::{Client, ClientConfig, MonitorVersion};
pub use error::Error;
pub use idl::{Idl, IdlState};
pub use row::Row;
pub use schema::{ColumnSchema, DbSchema, TableSchema};
pub use transaction::{RowRef, Transaction, TransactionStatus};

pub type Result<T> = std::result::Result<T, Error>;
