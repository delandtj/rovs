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

use std::net::Ipv4Addr;

use rovs_openflow::{ActionList, Flow, Match, NatConfig, VConn, CT_COMMIT};
use rovs_openflow::oxm::ct_state;

/// Configuration for SNAT gateway.
#[derive(Debug, Clone)]
pub struct SnatConfig {
    /// External (public) IP address for SNAT
    pub external_ip: Ipv4Addr,
    /// Optional: external IP range end (for multiple IPs)
    pub external_ip_max: Option<Ipv4Addr>,
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
    /// Create a simple SNAT configuration.
    pub fn new(external_ip: Ipv4Addr, internal_port: u32, external_port: u32) -> Self {
        Self {
            external_ip,
            external_ip_max: None,
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

    /// Set an IP range for SNAT (multiple external IPs).
    #[must_use]
    pub fn ip_range(mut self, max: Ipv4Addr) -> Self {
        self.external_ip_max = Some(max);
        self
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

    /// Build the NAT configuration from settings.
    fn build_nat_config(&self) -> NatConfig {
        let mut nat = if let Some(max) = self.config.external_ip_max {
            NatConfig::snat_range(self.config.external_ip, max)
        } else {
            NatConfig::snat(self.config.external_ip)
        };

        if let Some((min, max)) = self.config.port_range {
            nat = nat.port_range(min, max);
        }

        if self.config.random_ports {
            nat = nat.random();
        }

        nat
    }

    /// Generate all flows for the SNAT gateway.
    pub fn all_flows(&self, base_table: u8, priority: u16) -> Vec<Flow> {
        let mut flows = Vec::new();
        let ct_table = base_table;
        let policy_table = base_table + 1;
        let output_table = base_table + 2;
        let zone = self.config.zone;

        // Table N: Connection tracking (initial)
        // IPv4 -> ct(zone, table=N+1)
        flows.push(
            Flow::add()
                .table(ct_table)
                .priority(priority)
                .match_fields(Match::new().eth_type(0x0800))
                .actions(ActionList::new().ct(0, zone, Some(policy_table))),
        );

        // IPv6 -> ct(zone, table=N+1)
        flows.push(
            Flow::add()
                .table(ct_table)
                .priority(priority)
                .match_fields(Match::new().eth_type(0x86dd))
                .actions(ActionList::new().ct(0, zone, Some(policy_table))),
        );

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
                    Match::new()
                        .ct_state_masked(ct_state::TRK | ct_state::INV, ct_state::TRK | ct_state::INV),
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
                        .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
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
                        .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
                )
                .actions(ActionList::new().output(self.config.internal_port)),
        );

        // New outbound IPv4 -> SNAT + commit
        let nat_config = self.build_nat_config();
        flows.push(
            Flow::add()
                .table(policy_table)
                .priority(priority)
                .match_fields(
                    Match::new()
                        .in_port(self.config.internal_port)
                        .eth_type(0x0800)
                        .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
                )
                .actions(ActionList::new().ct_nat(CT_COMMIT, zone, Some(output_table), nat_config)),
        );

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
        conn.send_flow_sync(&Flow::delete().table(base_table)).await?;
        conn.send_flow_sync(&Flow::delete().table(base_table + 1)).await?;
        conn.send_flow_sync(&Flow::delete().table(base_table + 2)).await?;
        Ok(())
    }
}

/// Configuration for DNAT service (port forwarding).
#[derive(Debug, Clone)]
pub struct DnatRule {
    /// External port to match
    pub external_port: u16,
    /// Protocol (6 = TCP, 17 = UDP)
    pub protocol: u8,
    /// Internal destination IP
    pub internal_ip: Ipv4Addr,
    /// Internal destination port (None = same as external)
    pub internal_port: Option<u16>,
}

impl DnatRule {
    /// Create a TCP port forwarding rule.
    pub fn tcp(external_port: u16, internal_ip: Ipv4Addr, internal_port: u16) -> Self {
        Self {
            external_port,
            protocol: 6,
            internal_ip,
            internal_port: Some(internal_port),
        }
    }

    /// Create a UDP port forwarding rule.
    pub fn udp(external_port: u16, internal_ip: Ipv4Addr, internal_port: u16) -> Self {
        Self {
            external_port,
            protocol: 17,
            internal_ip,
            internal_port: Some(internal_port),
        }
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
    pub fn forward_tcp(mut self, external_port: u16, internal_ip: Ipv4Addr, internal_port: u16) -> Self {
        self.rules.push(DnatRule::tcp(external_port, internal_ip, internal_port));
        self
    }

    /// Add a UDP port forwarding rule.
    #[must_use]
    pub fn forward_udp(mut self, external_port: u16, internal_ip: Ipv4Addr, internal_port: u16) -> Self {
        self.rules.push(DnatRule::udp(external_port, internal_ip, internal_port));
        self
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
    pub fn all_flows(&self, base_table: u8, priority: u16) -> Vec<Flow> {
        let mut flows = Vec::new();
        let ct_table = base_table;
        let policy_table = base_table + 1;
        let output_table = base_table + 2;
        let zone = self.config.zone;

        // Table N: Connection tracking
        flows.push(
            Flow::add()
                .table(ct_table)
                .priority(priority)
                .match_fields(Match::new().eth_type(0x0800))
                .actions(ActionList::new().ct(0, zone, Some(policy_table))),
        );

        // Table N+1: Policy

        // Drop invalid
        flows.push(
            Flow::add()
                .table(policy_table)
                .priority(priority + 10)
                .match_fields(
                    Match::new()
                        .ct_state_masked(ct_state::TRK | ct_state::INV, ct_state::TRK | ct_state::INV),
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
                        .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
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
                        .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
                )
                .actions(ActionList::new().output(self.config.external_port)),
        );

        // DNAT rules for new inbound connections
        for rule in &self.config.rules {
            let nat_config = if let Some(port) = rule.internal_port {
                NatConfig::dnat(rule.internal_ip).port(port)
            } else {
                NatConfig::dnat(rule.internal_ip)
            };

            let mut match_fields = Match::new()
                .in_port(self.config.external_port)
                .eth_type(0x0800)
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
                    .actions(ActionList::new().ct_nat(CT_COMMIT, zone, Some(output_table), nat_config)),
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
        let config = SnatConfig::new(
            Ipv4Addr::new(10, 0, 0, 1),
            1,
            2,
        )
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
        let config = DnatConfig::new(2, 1)
            .forward_tcp(80, Ipv4Addr::new(192, 168, 1, 10), 8080);
        let service = DnatService::new(config);
        let flows = service.all_flows(0, 100);

        // Should have flows for ct, policy, and output
        assert!(flows.len() >= 5);
    }
}
