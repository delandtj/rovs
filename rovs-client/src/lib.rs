#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::unused_async)]

//! High-level OVS client API.
//!
//! Provides a unified interface for:
//! - OVSDB topology management (bridges, ports, interfaces)
//! - `OpenFlow` flow programming
//!
//! # Example
//!
//! ```ignore
//! use rovs_client::OvsClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = OvsClient::connect(
//!         "unix:/var/run/openvswitch/db.sock",
//!         "tcp:127.0.0.1:6653",
//!     ).await?;
//!
//!     // List bridges
//!     let bridges = client.list_bridges().await?;
//!     for bridge in bridges {
//!         println!("Bridge: {}", bridge.name);
//!     }
//!
//!     // Add a flow
//!     use rovs_openflow::{Flow, Match, ActionList};
//!     let flow = Flow::add()
//!         .table(0)
//!         .priority(100)
//!         .match_fields(Match::new().in_port(1))
//!         .actions(ActionList::new().output(2));
//!
//!     client.add_flow("br0", flow).await?;
//!
//!     Ok(())
//! }
//! ```

mod client;
mod error;
mod topology;

pub use client::OvsClient;
pub use error::Error;
pub use topology::{Bridge, Interface, Port};

// Re-export commonly used types from other crates
pub use rovs_openflow::{ActionList, Flow, LearnSpec, Match, NxLearn, nxm};
pub use rovs_ovsdb::{Transaction, TransactionStatus};

pub type Result<T> = std::result::Result<T, Error>;
