//! NAT (Network Address Translation) flow templates.
//!
//! Provides high-level abstractions for common NAT scenarios:
//! - [`SnatGateway`] - Masquerade outbound traffic (like iptables MASQUERADE)
//! - [`DnatService`] - Port forwarding to internal servers
//! - [`FullNat`] - Combined SNAT + DNAT for complete NAT gateway
//!
//! # Flow Pipeline
//!
//! NAT flows use a 3-table pipeline:
//! - Table N: Initial connection tracking (populate ct_state)
//! - Table N+1: NAT policy decisions based on ct_state
//! - Table N+2: Output after NAT commit
//!
//! # Example
//!
//! ```ignore
//! use rovs_ext::flows::{SnatGateway, SnatConfig};
//! use std::net::Ipv4Addr;
//!
//! // Create SNAT gateway for outbound traffic
//! let snat = SnatGateway::new(SnatConfig {
//!     external_ip: Ipv4Addr::new(203, 0, 113, 1),
//!     internal_port: 1,
//!     external_port: 2,
//!     zone: 1,
//! });
//!
//! // Install flows starting at table 0
//! snat.install(&mut conn, 0, 100).await?;
//! ```

use std::net::{Ipv4Addr, Ipv6Addr};

use rovs_openflow::oxm::ct_state;
use rovs_openflow::{ActionList, CT_COMMIT, Flow, Match, NatConfig, VConn};

/// Configuration for SNAT gateway.
#[derive(Debug, Clone)]
pub struct SnatConfig {
    /// External (public) IPv4 address for SNAT
    pub external_ip: Option<Ipv4Addr>,
    /// Optional: external IPv4 range end (for multiple IPs)
    pub external_ip_max: Option<Ipv4Addr>,
    /// External (public) IPv6 address for SNAT
    pub external_ip_v6: Option<Ipv6Addr>,
    /// Optional: external IPv6 range end (for multiple IPs)
    pub external_ip_v6_max: Option<Ipv6Addr>,
    /// Internal-facing port number
    pub internal_port: u32,
    /// External-facing port number
    pub external_port: u32,
    /// Connection tracking zone
    pub zone: u16,
    /// Optional: port range for SNAT
    pub port_range: Option<(u16, u16)>,
    /// Use random port selection
    pub random_ports: bool,
}

impl SnatConfig {
    /// Create a SNAT configuration with IPv4 address.
    pub fn new(external_ip: Ipv4Addr, internal_port: u32, external_port: u32) -> Self {
        Self {
            external_ip: Some(external_ip),
            external_ip_max: None,
            external_ip_v6: None,
            external_ip_v6_max: None,
            internal_port,
            external_port,
            zone: 1,
            port_range: None,
            random_ports: false,
        }
    }

    /// Create a SNAT configuration with IPv6 address.
    pub fn new_v6(external_ip: Ipv6Addr, internal_port: u32, external_port: u32) -> Self {
        Self {
            external_ip: None,
            external_ip_max: None,
            external_ip_v6: Some(external_ip),
            external_ip_v6_max: None,
            internal_port,
            external_port,
            zone: 1,
            port_range: None,
            random_ports: false,
        }
    }

    /// Create a dual-stack SNAT configuration with both IPv4 and IPv6 addresses.
    pub fn dual_stack(
        external_ip_v4: Ipv4Addr,
        external_ip_v6: Ipv6Addr,
        internal_port: u32,
        external_port: u32,
    ) -> Self {
        Self {
            external_ip: Some(external_ip_v4),
            external_ip_max: None,
            external_ip_v6: Some(external_ip_v6),
            external_ip_v6_max: None,
            internal_port,
            external_port,
            zone: 1,
            port_range: None,
            random_ports: false,
        }
    }

    /// Set the connection tracking zone.
    #[must_use]
    pub fn zone(mut self, zone: u16) -> Self {
        self.zone = zone;
        self
    }

    /// Set a port range for SNAT.
    #[must_use]
    pub fn port_range(mut self, min: u16, max: u16) -> Self {
        self.port_range = Some((min, max));
        self
    }

    /// Use random port selection.
    #[must_use]
    pub fn random(mut self) -> Self {
        self.random_ports = true;
        self
    }

    /// Set an IPv4 range for SNAT (multiple external IPs).
    #[must_use]
    pub fn ip_range(mut self, max: Ipv4Addr) -> Self {
        self.external_ip_max = Some(max);
        self
    }

    /// Set an IPv6 range for SNAT (multiple external IPs).
    #[must_use]
    pub fn ip_v6_range(mut self, max: Ipv6Addr) -> Self {
        self.external_ip_v6_max = Some(max);
        self
    }

    /// Check if IPv4 SNAT is configured.
    pub fn has_ipv4(&self) -> bool {
        self.external_ip.is_some()
    }

    /// Check if IPv6 SNAT is configured.
    pub fn has_ipv6(&self) -> bool {
        self.external_ip_v6.is_some()
    }
}

/// SNAT Gateway flow builder.
///
/// Creates flows for masquerading outbound traffic, similar to
/// `iptables -t nat -A POSTROUTING -o eth0 -j MASQUERADE`.
///
/// # Flow Pipeline
///
/// - Table N: Track IPv4/IPv6 traffic, recirculate to N+1
/// - Table N+1:
///   - Established/related: forward to external
///   - New outbound: SNAT + commit, recirculate to N+2
///   - New inbound (reply): forward to internal
/// - Table N+2: Output after NAT
#[derive(Debug, Clone)]
pub struct SnatGateway {
    config: SnatConfig,
}

impl SnatGateway {
    /// Create a new SNAT gateway.
    pub fn new(config: SnatConfig) -> Self {
        Self { config }
    }

    /// Build the IPv4 NAT configuration from settings.
    fn build_nat_config_v4(&self) -> Option<NatConfig> {
        let ip = self.config.external_ip?;
        let mut nat = if let Some(max) = self.config.external_ip_max {
            NatConfig::snat_range(ip, max)
        } else {
            NatConfig::snat(ip)
        };

        if let Some((min, max)) = self.config.port_range {
            nat = nat.port_range(min, max);
        }

        if self.config.random_ports {
            nat = nat.random();
        }

        Some(nat)
    }

    /// Build the IPv6 NAT configuration from settings.
    fn build_nat_config_v6(&self) -> Option<NatConfig> {
        let ip = self.config.external_ip_v6?;
        let mut nat = if let Some(max) = self.config.external_ip_v6_max {
            NatConfig::snat_v6_range(ip, max)
        } else {
            NatConfig::snat_v6(ip)
        };

        if let Some((min, max)) = self.config.port_range {
            nat = nat.port_range(min, max);
        }

        if self.config.random_ports {
            nat = nat.random();
        }

        Some(nat)
    }

    /// Generate all flows for the SNAT gateway.
    #[allow(clippy::too_many_lines)]
    pub fn all_flows(&self, base_table: u8, priority: u16) -> Vec<Flow> {
        let mut flows = Vec::new();
        let ct_table = base_table;
        let policy_table = base_table + 1;
        let output_table = base_table + 2;
        let zone = self.config.zone;

        // Table N: Connection tracking (initial)
        // IPv4 -> ct(zone, table=N+1)
        if self.config.has_ipv4() {
            flows.push(
                Flow::add()
                    .table(ct_table)
                    .priority(priority)
                    .match_fields(Match::new().eth_type(0x0800))
                    .actions(ActionList::new().ct(0, zone, Some(policy_table))),
            );
        }

        // IPv6 -> ct(zone, table=N+1)
        if self.config.has_ipv6() {
            flows.push(
                Flow::add()
                    .table(ct_table)
                    .priority(priority)
                    .match_fields(Match::new().eth_type(0x86dd))
                    .actions(ActionList::new().ct(0, zone, Some(policy_table))),
            );
        }

        // ARP passthrough
        flows.push(
            Flow::add()
                .table(ct_table)
                .priority(priority)
                .match_fields(Match::new().eth_type(0x0806))
                .actions(ActionList::new().normal()),
        );

        // Table N+1: Policy decisions

        // Drop invalid
        flows.push(
            Flow::add()
                .table(policy_table)
                .priority(priority + 10)
                .match_fields(
                    Match::new().ct_state_masked(
                        ct_state::TRK | ct_state::INV,
                        ct_state::TRK | ct_state::INV,
                    ),
                )
                .actions(ActionList::new().drop()),
        );

        // Established/related from internal -> forward to external
        flows.push(
            Flow::add()
                .table(policy_table)
                .priority(priority + 5)
                .match_fields(
                    Match::new()
                        .in_port(self.config.internal_port)
                        .ct_state_masked(
                            ct_state::TRK | ct_state::EST,
                            ct_state::TRK | ct_state::EST,
                        ),
                )
                .actions(ActionList::new().output(self.config.external_port)),
        );

        // Established/related from external -> forward to internal
        flows.push(
            Flow::add()
                .table(policy_table)
                .priority(priority + 5)
                .match_fields(
                    Match::new()
                        .in_port(self.config.external_port)
                        .ct_state_masked(
                            ct_state::TRK | ct_state::EST,
                            ct_state::TRK | ct_state::EST,
                        ),
                )
                .actions(ActionList::new().output(self.config.internal_port)),
        );

        // New outbound IPv4 -> SNAT + commit
        if let Some(nat_config) = self.build_nat_config_v4() {
            flows.push(
                Flow::add()
                    .table(policy_table)
                    .priority(priority)
                    .match_fields(
                        Match::new()
                            .in_port(self.config.internal_port)
                            .eth_type(0x0800)
                            .ct_state_masked(
                                ct_state::TRK | ct_state::NEW,
                                ct_state::TRK | ct_state::NEW,
                            ),
                    )
                    .actions(ActionList::new().ct_nat(
                        CT_COMMIT,
                        zone,
                        Some(output_table),
                        nat_config,
                    )),
            );
        }

        // New outbound IPv6 -> SNAT + commit
        if let Some(nat_config) = self.build_nat_config_v6() {
            flows.push(
                Flow::add()
                    .table(policy_table)
                    .priority(priority)
                    .match_fields(
                        Match::new()
                            .in_port(self.config.internal_port)
                            .eth_type(0x86dd)
                            .ct_state_masked(
                                ct_state::TRK | ct_state::NEW,
                                ct_state::TRK | ct_state::NEW,
                            ),
                    )
                    .actions(ActionList::new().ct_nat(
                        CT_COMMIT,
                        zone,
                        Some(output_table),
                        nat_config,
                    )),
            );
        }

        // Table N+2: Output after NAT
        flows.push(
            Flow::add()
                .table(output_table)
                .priority(priority)
                .match_fields(Match::new().in_port(self.config.internal_port))
                .actions(ActionList::new().output(self.config.external_port)),
        );

        flows.push(
            Flow::add()
                .table(output_table)
                .priority(priority)
                .match_fields(Match::new().in_port(self.config.external_port))
                .actions(ActionList::new().output(self.config.internal_port)),
        );

        flows
    }

    /// Install all SNAT gateway flows.
    pub async fn install(
        &self,
        conn: &mut VConn,
        base_table: u8,
        priority: u16,
    ) -> rovs_openflow::Result<()> {
        for flow in self.all_flows(base_table, priority) {
            conn.send_flow_sync(&flow).await?;
        }
        Ok(())
    }

    /// Delete all SNAT gateway flows from the tables.
    pub async fn delete(&self, conn: &mut VConn, base_table: u8) -> rovs_openflow::Result<()> {
        conn.send_flow_sync(&Flow::delete().table(base_table))
            .await?;
        conn.send_flow_sync(&Flow::delete().table(base_table + 1))
            .await?;
        conn.send_flow_sync(&Flow::delete().table(base_table + 2))
            .await?;
        Ok(())
    }
}

/// Internal destination address (IPv4 or IPv6).
#[derive(Debug, Clone, Copy)]
pub enum DnatTarget {
    /// IPv4 destination
    V4(Ipv4Addr),
    /// IPv6 destination
    V6(Ipv6Addr),
}

impl From<Ipv4Addr> for DnatTarget {
    fn from(addr: Ipv4Addr) -> Self {
        DnatTarget::V4(addr)
    }
}

impl From<Ipv6Addr> for DnatTarget {
    fn from(addr: Ipv6Addr) -> Self {
        DnatTarget::V6(addr)
    }
}

/// Configuration for DNAT service (port forwarding).
#[derive(Debug, Clone)]
pub struct DnatRule {
    /// External port to match
    pub external_port: u16,
    /// Protocol (6 = TCP, 17 = UDP)
    pub protocol: u8,
    /// Internal destination address
    pub internal_target: DnatTarget,
    /// Internal destination port (None = same as external)
    pub internal_port: Option<u16>,
}

impl DnatRule {
    /// Create a TCP port forwarding rule (IPv4).
    pub fn tcp(external_port: u16, internal_ip: Ipv4Addr, internal_port: u16) -> Self {
        Self {
            external_port,
            protocol: 6,
            internal_target: DnatTarget::V4(internal_ip),
            internal_port: Some(internal_port),
        }
    }

    /// Create a UDP port forwarding rule (IPv4).
    pub fn udp(external_port: u16, internal_ip: Ipv4Addr, internal_port: u16) -> Self {
        Self {
            external_port,
            protocol: 17,
            internal_target: DnatTarget::V4(internal_ip),
            internal_port: Some(internal_port),
        }
    }

    /// Create a TCP port forwarding rule (IPv6).
    pub fn tcp_v6(external_port: u16, internal_ip: Ipv6Addr, internal_port: u16) -> Self {
        Self {
            external_port,
            protocol: 6,
            internal_target: DnatTarget::V6(internal_ip),
            internal_port: Some(internal_port),
        }
    }

    /// Create a UDP port forwarding rule (IPv6).
    pub fn udp_v6(external_port: u16, internal_ip: Ipv6Addr, internal_port: u16) -> Self {
        Self {
            external_port,
            protocol: 17,
            internal_target: DnatTarget::V6(internal_ip),
            internal_port: Some(internal_port),
        }
    }

    /// Check if this rule is for IPv4.
    pub fn is_ipv4(&self) -> bool {
        matches!(self.internal_target, DnatTarget::V4(_))
    }

    /// Check if this rule is for IPv6.
    pub fn is_ipv6(&self) -> bool {
        matches!(self.internal_target, DnatTarget::V6(_))
    }
}

/// Configuration for DNAT service.
#[derive(Debug, Clone)]
pub struct DnatConfig {
    /// Port on which external traffic arrives
    pub external_port: u32,
    /// Port to forward traffic to internal network
    pub internal_port: u32,
    /// Connection tracking zone
    pub zone: u16,
    /// DNAT rules (port forwarding entries)
    pub rules: Vec<DnatRule>,
}

impl DnatConfig {
    /// Create a new DNAT configuration.
    pub fn new(external_port: u32, internal_port: u32) -> Self {
        Self {
            external_port,
            internal_port,
            zone: 1,
            rules: Vec::new(),
        }
    }

    /// Set the connection tracking zone.
    #[must_use]
    pub fn zone(mut self, zone: u16) -> Self {
        self.zone = zone;
        self
    }

    /// Add a port forwarding rule.
    #[must_use]
    pub fn add_rule(mut self, rule: DnatRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Add a TCP port forwarding rule.
    #[must_use]
    pub fn forward_tcp(
        mut self,
        external_port: u16,
        internal_ip: Ipv4Addr,
        internal_port: u16,
    ) -> Self {
        self.rules
            .push(DnatRule::tcp(external_port, internal_ip, internal_port));
        self
    }

    /// Add a UDP port forwarding rule.
    #[must_use]
    pub fn forward_udp(
        mut self,
        external_port: u16,
        internal_ip: Ipv4Addr,
        internal_port: u16,
    ) -> Self {
        self.rules
            .push(DnatRule::udp(external_port, internal_ip, internal_port));
        self
    }

    /// Add a TCP port forwarding rule (IPv6).
    #[must_use]
    pub fn forward_tcp_v6(
        mut self,
        external_port: u16,
        internal_ip: Ipv6Addr,
        internal_port: u16,
    ) -> Self {
        self.rules
            .push(DnatRule::tcp_v6(external_port, internal_ip, internal_port));
        self
    }

    /// Add a UDP port forwarding rule (IPv6).
    #[must_use]
    pub fn forward_udp_v6(
        mut self,
        external_port: u16,
        internal_ip: Ipv6Addr,
        internal_port: u16,
    ) -> Self {
        self.rules
            .push(DnatRule::udp_v6(external_port, internal_ip, internal_port));
        self
    }

    /// Check if any IPv4 rules are configured.
    pub fn has_ipv4_rules(&self) -> bool {
        self.rules.iter().any(DnatRule::is_ipv4)
    }

    /// Check if any IPv6 rules are configured.
    pub fn has_ipv6_rules(&self) -> bool {
        self.rules.iter().any(DnatRule::is_ipv6)
    }
}

/// DNAT Service flow builder.
///
/// Creates flows for port forwarding to internal servers.
#[derive(Debug, Clone)]
pub struct DnatService {
    config: DnatConfig,
}

impl DnatService {
    /// Create a new DNAT service.
    pub fn new(config: DnatConfig) -> Self {
        Self { config }
    }

    /// Generate all flows for the DNAT service.
    #[allow(clippy::too_many_lines)]
    pub fn all_flows(&self, base_table: u8, priority: u16) -> Vec<Flow> {
        let mut flows = Vec::new();
        let ct_table = base_table;
        let policy_table = base_table + 1;
        let output_table = base_table + 2;
        let zone = self.config.zone;

        // Table N: Connection tracking
        // Add IPv4 CT if any IPv4 rules exist
        if self.config.has_ipv4_rules() {
            flows.push(
                Flow::add()
                    .table(ct_table)
                    .priority(priority)
                    .match_fields(Match::new().eth_type(0x0800))
                    .actions(ActionList::new().ct(0, zone, Some(policy_table))),
            );
        }

        // Add IPv6 CT if any IPv6 rules exist
        if self.config.has_ipv6_rules() {
            flows.push(
                Flow::add()
                    .table(ct_table)
                    .priority(priority)
                    .match_fields(Match::new().eth_type(0x86dd))
                    .actions(ActionList::new().ct(0, zone, Some(policy_table))),
            );
        }

        // Table N+1: Policy

        // Drop invalid
        flows.push(
            Flow::add()
                .table(policy_table)
                .priority(priority + 10)
                .match_fields(
                    Match::new().ct_state_masked(
                        ct_state::TRK | ct_state::INV,
                        ct_state::TRK | ct_state::INV,
                    ),
                )
                .actions(ActionList::new().drop()),
        );

        // Established -> forward
        flows.push(
            Flow::add()
                .table(policy_table)
                .priority(priority + 5)
                .match_fields(
                    Match::new()
                        .in_port(self.config.external_port)
                        .ct_state_masked(
                            ct_state::TRK | ct_state::EST,
                            ct_state::TRK | ct_state::EST,
                        ),
                )
                .actions(ActionList::new().output(self.config.internal_port)),
        );

        flows.push(
            Flow::add()
                .table(policy_table)
                .priority(priority + 5)
                .match_fields(
                    Match::new()
                        .in_port(self.config.internal_port)
                        .ct_state_masked(
                            ct_state::TRK | ct_state::EST,
                            ct_state::TRK | ct_state::EST,
                        ),
                )
                .actions(ActionList::new().output(self.config.external_port)),
        );

        // DNAT rules for new inbound connections
        for rule in &self.config.rules {
            let (nat_config, eth_type) = match rule.internal_target {
                DnatTarget::V4(ip) => {
                    let nat = if let Some(port) = rule.internal_port {
                        NatConfig::dnat(ip).port(port)
                    } else {
                        NatConfig::dnat(ip)
                    };
                    (nat, 0x0800u16)
                }
                DnatTarget::V6(ip) => {
                    let nat = if let Some(port) = rule.internal_port {
                        NatConfig::dnat_v6(ip).port(port)
                    } else {
                        NatConfig::dnat_v6(ip)
                    };
                    (nat, 0x86ddu16)
                }
            };

            let mut match_fields = Match::new()
                .in_port(self.config.external_port)
                .eth_type(eth_type)
                .ip_proto(rule.protocol)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW);

            // Add port match based on protocol
            if rule.protocol == 6 {
                match_fields = match_fields.tcp_dst(rule.external_port);
            } else if rule.protocol == 17 {
                match_fields = match_fields.udp_dst(rule.external_port);
            }

            flows.push(
                Flow::add()
                    .table(policy_table)
                    .priority(priority)
                    .match_fields(match_fields)
                    .actions(ActionList::new().ct_nat(
                        CT_COMMIT,
                        zone,
                        Some(output_table),
                        nat_config,
                    )),
            );
        }

        // Table N+2: Output after NAT
        flows.push(
            Flow::add()
                .table(output_table)
                .priority(priority)
                .match_fields(Match::new().in_port(self.config.external_port))
                .actions(ActionList::new().output(self.config.internal_port)),
        );

        flows.push(
            Flow::add()
                .table(output_table)
                .priority(priority)
                .match_fields(Match::new().in_port(self.config.internal_port))
                .actions(ActionList::new().output(self.config.external_port)),
        );

        flows
    }

    /// Install all DNAT service flows.
    pub async fn install(
        &self,
        conn: &mut VConn,
        base_table: u8,
        priority: u16,
    ) -> rovs_openflow::Result<()> {
        for flow in self.all_flows(base_table, priority) {
            conn.send_flow_sync(&flow).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snat_config_builder() {
        let config = SnatConfig::new(Ipv4Addr::new(10, 0, 0, 1), 1, 2)
            .zone(5)
            .port_range(5000, 6000)
            .random();

        assert_eq!(config.zone, 5);
        assert_eq!(config.port_range, Some((5000, 6000)));
        assert!(config.random_ports);
    }

    #[test]
    fn snat_gateway_generates_flows() {
        let config = SnatConfig::new(Ipv4Addr::new(10, 0, 0, 1), 1, 2);
        let gateway = SnatGateway::new(config);
        let flows = gateway.all_flows(0, 100);

        // Should have: 3 ct flows + 3 policy flows + 2 output flows = 8+
        assert!(flows.len() >= 8);
    }

    #[test]
    fn dnat_config_builder() {
        let config = DnatConfig::new(2, 1)
            .zone(3)
            .forward_tcp(80, Ipv4Addr::new(192, 168, 1, 10), 8080)
            .forward_tcp(443, Ipv4Addr::new(192, 168, 1, 10), 8443);

        assert_eq!(config.zone, 3);
        assert_eq!(config.rules.len(), 2);
    }

    #[test]
    fn dnat_service_generates_flows() {
        let config = DnatConfig::new(2, 1).forward_tcp(80, Ipv4Addr::new(192, 168, 1, 10), 8080);
        let service = DnatService::new(config);
        let flows = service.all_flows(0, 100);

        // Should have flows for ct, policy, and output
        assert!(flows.len() >= 5);
    }

    #[test]
    fn snat_config_v6_builder() {
        let config = SnatConfig::new_v6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1), 1, 2)
            .zone(5)
            .port_range(5000, 6000);

        assert!(config.has_ipv6());
        assert!(!config.has_ipv4());
        assert_eq!(config.zone, 5);
    }

    #[test]
    fn snat_dual_stack_config() {
        let config = SnatConfig::dual_stack(
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
            1,
            2,
        );

        assert!(config.has_ipv4());
        assert!(config.has_ipv6());
    }

    #[test]
    fn snat_gateway_dual_stack_generates_flows() {
        let config = SnatConfig::dual_stack(
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
            1,
            2,
        );
        let gateway = SnatGateway::new(config);
        let flows = gateway.all_flows(0, 100);

        // Should have: 2 ct flows (v4+v6) + 1 arp + 1 drop + 2 established + 2 new (v4+v6) + 2 output = 10
        assert!(flows.len() >= 10);
    }

    #[test]
    fn dnat_config_v6_builder() {
        let config = DnatConfig::new(2, 1).zone(3).forward_tcp_v6(
            80,
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 10),
            8080,
        );

        assert!(config.has_ipv6_rules());
        assert!(!config.has_ipv4_rules());
        assert_eq!(config.rules.len(), 1);
    }

    #[test]
    fn dnat_service_dual_stack_generates_flows() {
        let config = DnatConfig::new(2, 1)
            .forward_tcp(80, Ipv4Addr::new(192, 168, 1, 10), 8080)
            .forward_tcp_v6(80, Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 10), 8080);
        let service = DnatService::new(config);
        let flows = service.all_flows(0, 100);

        // Should have: 2 ct flows (v4+v6) + 1 drop + 2 established + 2 dnat rules + 2 output = 9
        assert!(flows.len() >= 9);
    }
}
