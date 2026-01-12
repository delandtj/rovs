#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::unused_async)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::needless_pass_by_value)]

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
