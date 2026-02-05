//! Flow template builders for common OVS patterns.
//!
//! This module provides pre-built flow templates for common scenarios:
//!
//! - [`MacNatFlows`] - MAC address translation between ports
//! - [`ArpProxyFlows`] - ARP proxy using Nicira extensions
//! - [`NdpProxyFlows`] - NDP proxy (requires controller handler)
//! - [`LearningSwitchFlows`] - MAC learning switch with NxLearn
//! - [`SnatGateway`] - SNAT for outbound traffic (like iptables MASQUERADE)
//! - [`DnatService`] - DNAT for port forwarding to internal servers
//! - VLAN helpers for push/pop/translate operations

mod arp_proxy;
mod learning;
mod mac_nat;
mod nat;
mod ndp_proxy;
mod vlan;

pub use arp_proxy::{ArpProxyBuilder, ArpProxyConfig, ArpProxyFlows};
pub use learning::{LearningConfig, LearningSwitchFlows};
pub use mac_nat::{MacNatConfig, MacNatFlows};
pub use nat::{DnatConfig, DnatRule, DnatService, SnatConfig, SnatGateway};
pub use ndp_proxy::{NdpProxyBuilder, NdpProxyConfig, NdpProxyFlows};
pub use vlan::{
    forward_vlan_flow, pop_vlan_flow, push_vlan_flow, translate_vlan_flow, VlanAccessPort,
    VlanConfig,
};
