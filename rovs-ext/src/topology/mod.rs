//! Topology builders for common OVS configurations.
//!
//! This module provides builders for common OVS topology patterns:
//!
//! - [`BridgePair`] - Two bridges connected by patch ports
//! - [`VlanTrunk`] - Bridge with VLAN-configured access and trunk ports

mod bridge_pair;
mod vlan_trunk;

pub use bridge_pair::{BridgePair, BridgePairConfig};
pub use vlan_trunk::{AccessPortConfig, TrunkPortConfig, VlanTrunk, VlanTrunkConfig};
