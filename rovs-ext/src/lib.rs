//! High-level OVS automation extensions.
//!
//! `rovs-ext` provides higher-level abstractions for OVS automation:
//!
//! - **Controller Framework** - Event-driven packet processing with protocol handlers
//! - **Flow Templates** - Pre-built flows for MAC NAT, ARP/NDP proxy, learning switch
//! - **Topology Builders** - Bridge pairs, VLAN trunks with automatic wiring
//! - **Utilities** - MAC/IP conversion, port mapping
//!
//! # Quick Start
//!
//! ## Flow Templates
//!
//! ```ignore
//! use rovs_ext::flows::{MacNatConfig, MacNatFlows};
//!
//! let config = MacNatConfig::new(
//!     [0x02, 0x00, 0x00, 0x00, 0x00, 0x01],  // internal MAC
//!     [0x02, 0x00, 0x00, 0x00, 0x00, 0x99],  // external MAC
//!     1,  // internal port
//!     2,  // external port
//! );
//!
//! let flows = MacNatFlows::new(config);
//! flows.install(&mut conn, 0, 100).await?;
//! ```
//!
//! ## Topology Builders
//!
//! ```ignore
//! use rovs_ext::topology::BridgePair;
//!
//! let pair = BridgePair::new("br-int", "br-ext")
//!     .vlans(vec![100, 200]);
//!
//! pair.create(&mut client).await?;
//! ```
//!
//! ## Controller Framework
//!
//! ```ignore
//! use rovs_ext::controller::{Controller, ControllerConfig};
//! use rovs_ext::controller::protocol::ArpProxyHandler;
//!
//! let mut controller = Controller::new(&addr, ControllerConfig::default()).await?;
//!
//! let mut arp_handler = ArpProxyHandler::new();
//! arp_handler.add_entry([10, 0, 0, 99], [0x02, 0x00, 0x00, 0x00, 0x00, 0x99]);
//! controller.register(arp_handler);
//!
//! controller.run().await?;
//! ```

#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::redundant_closure_for_method_calls)]

pub mod controller;
mod error;
pub mod flows;
pub mod topology;
pub mod util;

pub use error::{Error, Result};

// Re-export commonly used types
pub use controller::{Controller, ControllerConfig, HandlerAction, PacketHandler};
pub use flows::{
    ArpProxyBuilder, ArpProxyConfig, ArpProxyFlows, LearningConfig, LearningSwitchFlows,
    MacNatConfig, MacNatFlows, NdpProxyBuilder, NdpProxyConfig, NdpProxyFlows,
};
pub use topology::{AccessPortConfig, BridgePair, TrunkPortConfig, VlanTrunk};
pub use util::{format_ipv4, format_mac, ipv4_to_u32, mac_to_u64, parse_ipv4, parse_mac, PortMapper};
