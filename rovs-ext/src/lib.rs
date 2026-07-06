//! High-level OVS automation extensions.
//!
//! `rovs-ext` provides higher-level abstractions built on top of `rovs-openflow`
//! and `rovs-ovsdb` for common OVS automation scenarios.
//!
//! # Modules
//!
//! - [`flows`] - Pre-built flow templates for common patterns
//! - [`topology`] - OVSDB topology builders for bridges and ports
//! - [`controller`] - OpenFlow controller framework with protocol handlers
//! - [`util`] - Utility functions for MAC/IP conversion and port mapping
//!
//! # Flow Templates
//!
//! The [`flows`] module provides ready-to-use flow patterns:
//!
//! | Template | Description |
//! |----------|-------------|
//! | [`MacNatFlows`] | MAC address translation between internal/external ports |
//! | [`ArpProxyFlows`] | ARP proxy responding on behalf of other hosts |
//! | [`NdpProxyFlows`] | NDP (IPv6) proxy with controller-based responses |
//! | [`LearningSwitchFlows`] | MAC learning switch using NxLearn action |
//! | [`SnatGateway`] | Source NAT for outbound traffic (masquerade) |
//! | [`DnatService`] | Destination NAT for port forwarding |
//! | VLAN helpers | Push/pop/translate VLAN tags |
//!
//! ## Example: MAC NAT
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
//! ## Example: SNAT Gateway
//!
//! ```ignore
//! use rovs_ext::flows::{SnatConfig, SnatGateway};
//! use std::net::Ipv4Addr;
//!
//! let gateway = SnatGateway::new(
//!     SnatConfig::new(Ipv4Addr::new(203, 0, 113, 1), 1, 2)
//!         .zone(1)
//!         .port_range(10000, 65000)
//!         .random()
//! );
//!
//! gateway.install(&mut conn, 0, 100).await?;  // Uses tables 0, 1, 2
//! ```
//!
//! ## Example: DNAT Port Forwarding
//!
//! ```ignore
//! use rovs_ext::flows::{DnatConfig, DnatService};
//! use std::net::Ipv4Addr;
//!
//! let service = DnatService::new(
//!     DnatConfig::new(2, 1)  // external port 2, internal port 1
//!         .forward_tcp(80, Ipv4Addr::new(192, 168, 1, 10), 8080)
//!         .forward_tcp(443, Ipv4Addr::new(192, 168, 1, 10), 8443)
//! );
//!
//! service.install(&mut conn, 0, 100).await?;
//! ```
//!
//! # Topology Builders
//!
//! The [`topology`] module provides builders for complex OVSDB configurations:
//!
//! | Builder | Description |
//! |---------|-------------|
//! | [`BridgePair`] | Two bridges connected by patch ports |
//! | [`VlanTrunk`] | Bridge with VLAN access and trunk ports |
//!
//! ## Example: Bridge Pair
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
//! ## Example: VLAN Trunk
//!
//! ```ignore
//! use rovs_ext::topology::{VlanTrunk, AccessPortConfig, TrunkPortConfig};
//!
//! let trunk = VlanTrunk::new("br-vlan")
//!     .add_access_port(AccessPortConfig::new("eth0", 100))
//!     .add_access_port(AccessPortConfig::new("eth1", 200))
//!     .add_trunk_port(TrunkPortConfig::new("uplink").vlans(vec![100, 200]));
//!
//! trunk.create(&mut client).await?;
//! ```
//!
//! # Controller Framework
//!
//! The [`controller`] module provides an event-driven OpenFlow controller:
//!
//! | Type | Description |
//! |------|-------------|
//! | [`Controller`] | Main controller with packet-in event loop |
//! | [`PacketHandler`] | Trait for custom packet handlers |
//! | [`controller::protocol::ArpProxyHandler`] | Built-in ARP proxy handler |
//! | [`controller::protocol::NdpProxyHandler`] | Built-in NDP (IPv6 neighbor) proxy handler |
//!
//! ## Example: ARP Proxy Controller
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
//!
//! # Utilities
//!
//! The [`util`] module provides helper functions:
//!
//! | Function | Description |
//! |----------|-------------|
//! | [`parse_mac`] | Parse MAC address from string ("aa:bb:cc:dd:ee:ff") |
//! | [`format_mac`] | Format MAC address to string |
//! | [`mac_to_u64`] | Convert MAC bytes to u64 for NxRegLoad |
//! | [`parse_ipv4`] | Parse IPv4 address from string |
//! | [`format_ipv4`] | Format IPv4 bytes to string |
//! | [`ipv4_to_u32`] | Convert IPv4 bytes to u32 |
//! | [`PortMapper`] | Map port names to OpenFlow port numbers |
//!
//! ## Example: Port Mapping
//!
//! ```ignore
//! use rovs_ext::util::PortMapper;
//!
//! let mut mapper = PortMapper::new();
//! mapper.insert("eth0", 1);
//! mapper.insert("eth1", 2);
//!
//! let port = mapper.require("eth0")?;  // Returns 1
//! ```

#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::redundant_closure_for_method_calls)]

pub mod appctl;
pub mod controller;
mod error;
pub mod flows;
pub mod ovsdb;
pub mod topology;
pub mod util;

pub use error::{Error, Result};

// Re-export commonly used types from flows
pub use flows::{
    ArpProxyBuilder, ArpProxyConfig, ArpProxyFlows, DnatConfig, DnatRule, DnatService, DnatTarget,
    LearningConfig, LearningSwitchFlows, MacNatConfig, MacNatFlows, NdpProxyBuilder,
    NdpProxyConfig, NdpProxyFlows, SnatConfig, SnatGateway, VlanAccessPort, VlanConfig,
};

// Re-export commonly used types from topology
pub use topology::{AccessPortConfig, BridgePair, TrunkPortConfig, VlanTrunk};

// Re-export commonly used types from controller
pub use controller::{Controller, ControllerConfig, HandlerAction, PacketHandler};

// Re-export commonly used utilities
pub use util::{
    PortMapper, format_ipv4, format_mac, ipv4_to_u32, mac_to_u64, parse_ipv4, parse_mac,
};

// Re-export appctl types
pub use appctl::{AppCtl, ConntrackEntry, DpifFlow};

// Re-export shared OVSDB handle
pub use ovsdb::OvsdbHandle;
