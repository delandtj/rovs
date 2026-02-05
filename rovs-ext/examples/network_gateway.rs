//! Example: Complete Network Gateway
//!
//! End-to-end example combining all OVS features into a complete gateway:
//!
//! ```text
//!   Internal Network          Gateway           External Network
//!   192.168.1.0/24    <-->   [OVS]   <-->      Internet
//!   fd00::/64                                   (SNAT to public IP)
//!      port 1                                      port 2
//! ```
//!
//! Features demonstrated:
//! - Dual-stack SNAT for outbound (IPv4 + IPv6)
//! - DNAT for exposed services (HTTP, SSH)
//! - Stateful firewall with zone policies
//! - ARP proxy for gateway IP
//! - NDP proxy for IPv6 gateway
//! - MAC learning on internal side
//!
//! Flow pipeline:
//! - Tables 0-1: L2 processing (MAC learning, ARP/NDP handling)
//! - Tables 2-3: Zone classification + CT
//! - Tables 4-5: Firewall policy
//! - Tables 6-8: NAT (SNAT + DNAT)
//! - Table 9: Output
//!
//! Run with:
//! ```sh
//! ./scripts/test-with-ovs.sh start full
//! OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example network_gateway
//! ```

// Examples are intentionally verbose for educational purposes
#![allow(clippy::too_many_lines)]
#![allow(clippy::similar_names)]

use std::net::{Ipv4Addr, Ipv6Addr};

use clap::Parser;
use rovs_openflow::oxm::ct_state;
use rovs_openflow::{ActionList, Flow, Match, NatConfig, VConn, CT_COMMIT};
use rovs_transport::Address;

#[derive(Parser)]
#[command(name = "network_gateway")]
#[command(about = "Complete network gateway demo combining all features")]
struct Args {
    /// Leave flows installed for inspection (don't cleanup)
    #[arg(long)]
    no_cleanup: bool,

    /// Skip MAC learning flows (simpler setup)
    #[arg(long)]
    no_learning: bool,
}

fn get_openflow_addr() -> Address {
    std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6653".to_string())
        .parse()
        .expect("Invalid OPENFLOW_ADDR")
}

// Network configuration
const INTERNAL_PORT: u32 = 1;
const EXTERNAL_PORT: u32 = 2;
const CT_ZONE: u16 = 100;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = get_openflow_addr();
    println!("Connecting to OpenFlow at {addr}...");

    let mut conn = VConn::connect(&addr).await?;
    println!("Connected! OpenFlow version: {:?}\n", conn.version());

    // Network addresses
    let gateway_ipv4 = Ipv4Addr::new(192, 168, 1, 1);
    // Gateway MAC would be used by ARP proxy controller: [0x02, 0x00, 0x00, 0x00, 0x00, 0x01]
    let public_ipv4 = Ipv4Addr::new(203, 0, 113, 1);
    let public_ipv6 = Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1);

    // DNAT services
    let web_server = Ipv4Addr::new(192, 168, 1, 10);
    let ssh_server = Ipv4Addr::new(192, 168, 1, 20);

    // Clear all tables
    println!("Clearing flow tables 0-9...");
    for table in 0..=9 {
        conn.send_flow_sync(&Flow::delete().table(table)).await?;
    }

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║              Complete Network Gateway Configuration           ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Internal Network: 192.168.1.0/24, fd00::/64                   ║");
    println!("║ Gateway IP: {gateway_ipv4}                               ║");
    println!("║ Public IPv4: {public_ipv4}                              ║");
    println!("║ Public IPv6: {public_ipv6}                              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Services:                                                     ║");
    println!("║   HTTP (80)  -> {web_server}:8080                        ║");
    println!("║   HTTPS(443) -> {web_server}:8443                        ║");
    println!("║   SSH (22)   -> {ssh_server}:22                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    // ==========================================================================
    // Tables 0-1: L2 Processing (MAC Learning, ARP/NDP)
    // ==========================================================================

    println!("\n--- Tables 0-1: L2 Processing ---");

    // Table 0: Initial packet classification

    // ARP requests for gateway IP -> send to controller (ARP proxy)
    let arp_proxy = Flow::add()
        .table(0)
        .priority(200)
        .match_fields(
            Match::new()
                .eth_type(0x0806)     // ARP
                .arp_op(1)            // ARP Request
                .arp_tpa(gateway_ipv4),
        )
        .actions(ActionList::new().controller(0xffff));

    conn.send_flow_sync(&arp_proxy).await?;
    println!("  Table 0: ARP Request for {gateway_ipv4} -> CONTROLLER");

    // Other ARP -> normal switching (or flood for learning)
    let arp_normal = Flow::add()
        .table(0)
        .priority(190)
        .match_fields(Match::new().eth_type(0x0806))
        .actions(ActionList::new().normal());

    conn.send_flow_sync(&arp_normal).await?;
    println!("  Table 0: ARP (other) -> NORMAL");

    // IPv6 NDP Neighbor Solicitation -> controller for NDP proxy
    let ndp_ns = Flow::add()
        .table(0)
        .priority(200)
        .match_fields(
            Match::new()
                .eth_type(0x86dd)
                .ip_proto(58)
                .icmpv6_type(135),  // Neighbor Solicitation
        )
        .actions(ActionList::new().controller(0xffff));

    conn.send_flow_sync(&ndp_ns).await?;
    println!("  Table 0: ICMPv6 NS -> CONTROLLER (NDP proxy)");

    // IPv6 NDP Neighbor Advertisement -> normal
    let ndp_na = Flow::add()
        .table(0)
        .priority(190)
        .match_fields(
            Match::new()
                .eth_type(0x86dd)
                .ip_proto(58)
                .icmpv6_type(136),  // Neighbor Advertisement
        )
        .actions(ActionList::new().normal());

    conn.send_flow_sync(&ndp_na).await?;
    println!("  Table 0: ICMPv6 NA -> NORMAL");

    if !args.no_learning {
        // MAC learning using NxLearn action
        // Learn source MAC from internal port for reverse path
        use rovs_openflow::nxm;
        use rovs_openflow::NxLearn;

        let learn_internal = NxLearn::new()
            .idle_timeout(300)   // 5 minute timeout
            .priority(100)
            .table(1)            // Install learned flows in table 1
            // Match on destination MAC = learned source MAC
            .match_field(nxm::ETH_SRC, nxm::ETH_DST, 48)
            // Output to learned input port
            .output_field(nxm::IN_PORT, 16);

        let mac_learn = Flow::add()
            .table(0)
            .priority(100)
            .match_fields(Match::new().in_port(INTERNAL_PORT))
            .actions(
                ActionList::new()
                    .learn(learn_internal)
                    .resubmit_table(2),  // Continue to zone classification
            );

        conn.send_flow_sync(&mac_learn).await?;
        println!("  Table 0: in={INTERNAL_PORT} -> learn(table=1), resubmit(2)");
    }

    // External port traffic -> zone classification (skip learning)
    let external_to_zone = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().in_port(EXTERNAL_PORT))
        .actions(ActionList::new().resubmit_table(2));

    conn.send_flow_sync(&external_to_zone).await?;
    println!("  Table 0: in={EXTERNAL_PORT} -> resubmit(2)");

    // Default: continue to L2 forwarding
    let default_to_l2 = Flow::add()
        .table(0)
        .priority(50)
        .actions(ActionList::new().resubmit_table(1).resubmit_table(2));

    conn.send_flow_sync(&default_to_l2).await?;
    println!("  Table 0: default -> resubmit(1), resubmit(2)");

    // Table 1: Learned MAC forwarding (populated by learn action)
    // Default: flood unknown unicast
    let flood_unknown = Flow::add()
        .table(1)
        .priority(0)
        .actions(ActionList::new()); // Empty = continue

    conn.send_flow_sync(&flood_unknown).await?;
    println!("  Table 1: default -> continue (learned flows here)");

    // ==========================================================================
    // Tables 2-3: Zone Classification + Connection Tracking
    // ==========================================================================

    println!("\n--- Tables 2-3: Zone Classification + CT ---");

    // Table 2: Zone classification and CT entry

    // IPv4 from internal
    let ct_ipv4_internal = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x0800),
        )
        .actions(ActionList::new().ct(0, CT_ZONE, Some(3)));

    conn.send_flow_sync(&ct_ipv4_internal).await?;
    println!("  Table 2: in={INTERNAL_PORT}, IPv4 -> ct(zone={CT_ZONE}, table=3)");

    // IPv6 from internal
    let ct_ipv6_internal = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x86dd),
        )
        .actions(ActionList::new().ct(0, CT_ZONE, Some(3)));

    conn.send_flow_sync(&ct_ipv6_internal).await?;
    println!("  Table 2: in={INTERNAL_PORT}, IPv6 -> ct(zone={CT_ZONE}, table=3)");

    // IPv4 from external
    let ct_ipv4_external = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x0800),
        )
        .actions(ActionList::new().ct(0, CT_ZONE, Some(3)));

    conn.send_flow_sync(&ct_ipv4_external).await?;
    println!("  Table 2: in={EXTERNAL_PORT}, IPv4 -> ct(zone={CT_ZONE}, table=3)");

    // IPv6 from external
    let ct_ipv6_external = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x86dd),
        )
        .actions(ActionList::new().ct(0, CT_ZONE, Some(3)));

    conn.send_flow_sync(&ct_ipv6_external).await?;
    println!("  Table 2: in={EXTERNAL_PORT}, IPv6 -> ct(zone={CT_ZONE}, table=3)");

    // Default drop
    conn.send_flow_sync(&Flow::add().table(2).priority(0).actions(ActionList::new().drop())).await?;
    println!("  Table 2: default -> DROP");

    // Table 3: CT state -> firewall
    // Invalid
    conn.send_flow_sync(
        &Flow::add()
            .table(3)
            .priority(200)
            .match_fields(
                Match::new()
                    .ct_state_masked(ct_state::TRK | ct_state::INV, ct_state::TRK | ct_state::INV),
            )
            .actions(ActionList::new().drop()),
    ).await?;
    println!("  Table 3: ct_state=+trk+inv -> DROP");

    // Established/Related -> firewall
    conn.send_flow_sync(
        &Flow::add()
            .table(3)
            .priority(150)
            .match_fields(
                Match::new()
                    .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
            )
            .actions(ActionList::new().resubmit_table(4)),
    ).await?;
    println!("  Table 3: ct_state=+trk+est -> resubmit(4)");

    conn.send_flow_sync(
        &Flow::add()
            .table(3)
            .priority(150)
            .match_fields(
                Match::new()
                    .ct_state_masked(ct_state::TRK | ct_state::REL, ct_state::TRK | ct_state::REL),
            )
            .actions(ActionList::new().resubmit_table(4)),
    ).await?;
    println!("  Table 3: ct_state=+trk+rel -> resubmit(4)");

    // New -> firewall
    conn.send_flow_sync(
        &Flow::add()
            .table(3)
            .priority(100)
            .match_fields(
                Match::new()
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().resubmit_table(4)),
    ).await?;
    println!("  Table 3: ct_state=+trk+new -> resubmit(4)");

    // Default drop
    conn.send_flow_sync(&Flow::add().table(3).priority(0).actions(ActionList::new().drop())).await?;
    println!("  Table 3: default -> DROP");

    // ==========================================================================
    // Tables 4-5: Firewall Policy
    // ==========================================================================

    println!("\n--- Tables 4-5: Firewall Policy ---");

    // Table 4: Policy decisions

    // Established connections -> NAT table
    conn.send_flow_sync(
        &Flow::add()
            .table(4)
            .priority(150)
            .match_fields(
                Match::new()
                    .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
            )
            .actions(ActionList::new().resubmit_table(6)),
    ).await?;
    println!("  Table 4: established -> resubmit(6) (NAT)");

    // Related connections -> NAT table
    conn.send_flow_sync(
        &Flow::add()
            .table(4)
            .priority(150)
            .match_fields(
                Match::new()
                    .ct_state_masked(ct_state::TRK | ct_state::REL, ct_state::TRK | ct_state::REL),
            )
            .actions(ActionList::new().resubmit_table(6)),
    ).await?;
    println!("  Table 4: related -> resubmit(6) (NAT)");

    // New outbound from internal -> allow
    conn.send_flow_sync(
        &Flow::add()
            .table(4)
            .priority(100)
            .match_fields(
                Match::new()
                    .in_port(INTERNAL_PORT)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().resubmit_table(6)),
    ).await?;
    println!("  Table 4: in={INTERNAL_PORT}, new -> resubmit(6) (allow outbound)");

    // New inbound: HTTP (80) -> allow
    conn.send_flow_sync(
        &Flow::add()
            .table(4)
            .priority(90)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x0800)
                    .ip_proto(6)
                    .tcp_dst(80)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().resubmit_table(6)),
    ).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, tcp_dst=80, new -> ALLOW");

    // New inbound: HTTPS (443) -> allow
    conn.send_flow_sync(
        &Flow::add()
            .table(4)
            .priority(90)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x0800)
                    .ip_proto(6)
                    .tcp_dst(443)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().resubmit_table(6)),
    ).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, tcp_dst=443, new -> ALLOW");

    // New inbound: SSH (22) -> allow
    conn.send_flow_sync(
        &Flow::add()
            .table(4)
            .priority(90)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x0800)
                    .ip_proto(6)
                    .tcp_dst(22)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().resubmit_table(6)),
    ).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, tcp_dst=22, new -> ALLOW");

    // New inbound: ICMP echo -> allow
    conn.send_flow_sync(
        &Flow::add()
            .table(4)
            .priority(85)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x0800)
                    .ip_proto(1)
                    .icmp_type(8)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().resubmit_table(6)),
    ).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, ICMP echo, new -> ALLOW");

    // New inbound: ICMPv6 echo -> allow
    conn.send_flow_sync(
        &Flow::add()
            .table(4)
            .priority(85)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x86dd)
                    .ip_proto(58)
                    .icmpv6_type(128)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().resubmit_table(6)),
    ).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, ICMPv6 echo, new -> ALLOW");

    // Block other new inbound
    conn.send_flow_sync(
        &Flow::add()
            .table(4)
            .priority(50)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().drop()),
    ).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, new (other) -> DROP");

    // Default drop
    conn.send_flow_sync(&Flow::add().table(4).priority(0).actions(ActionList::new().drop())).await?;
    println!("  Table 4: default -> DROP");

    // ==========================================================================
    // Tables 6-8: NAT (SNAT + DNAT)
    // ==========================================================================

    println!("\n--- Tables 6-8: NAT ---");

    // Table 6: NAT policy decisions

    // Established outbound -> forward (NAT already applied on original direction)
    conn.send_flow_sync(
        &Flow::add()
            .table(6)
            .priority(150)
            .match_fields(
                Match::new()
                    .in_port(INTERNAL_PORT)
                    .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
            )
            .actions(ActionList::new().output(EXTERNAL_PORT)),
    ).await?;
    println!("  Table 6: in={INTERNAL_PORT}, established -> output:{EXTERNAL_PORT}");

    // Established inbound -> forward
    conn.send_flow_sync(
        &Flow::add()
            .table(6)
            .priority(150)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
            )
            .actions(ActionList::new().output(INTERNAL_PORT)),
    ).await?;
    println!("  Table 6: in={EXTERNAL_PORT}, established -> output:{INTERNAL_PORT}");

    // New outbound IPv4 -> SNAT
    let snat_v4 = NatConfig::snat(public_ipv4).port_range(10000, 65000).random();
    conn.send_flow_sync(
        &Flow::add()
            .table(6)
            .priority(100)
            .match_fields(
                Match::new()
                    .in_port(INTERNAL_PORT)
                    .eth_type(0x0800)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct_nat(CT_COMMIT, CT_ZONE, Some(9), snat_v4)),
    ).await?;
    println!("  Table 6: in={INTERNAL_PORT}, IPv4, new -> SNAT to {public_ipv4}");

    // New outbound IPv6 -> SNAT (NAT66)
    let snat_v6 = NatConfig::snat_v6(public_ipv6);
    conn.send_flow_sync(
        &Flow::add()
            .table(6)
            .priority(100)
            .match_fields(
                Match::new()
                    .in_port(INTERNAL_PORT)
                    .eth_type(0x86dd)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct_nat(CT_COMMIT, CT_ZONE, Some(9), snat_v6)),
    ).await?;
    println!("  Table 6: in={INTERNAL_PORT}, IPv6, new -> SNAT to {public_ipv6}");

    // New inbound HTTP (80) -> DNAT to web server
    let dnat_http = NatConfig::dnat(web_server).port(8080);
    conn.send_flow_sync(
        &Flow::add()
            .table(6)
            .priority(90)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x0800)
                    .ip_proto(6)
                    .tcp_dst(80)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct_nat(CT_COMMIT, CT_ZONE, Some(9), dnat_http)),
    ).await?;
    println!("  Table 6: in={EXTERNAL_PORT}, tcp_dst=80, new -> DNAT to {web_server}:8080");

    // New inbound HTTPS (443) -> DNAT to web server
    let dnat_https = NatConfig::dnat(web_server).port(8443);
    conn.send_flow_sync(
        &Flow::add()
            .table(6)
            .priority(90)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x0800)
                    .ip_proto(6)
                    .tcp_dst(443)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct_nat(CT_COMMIT, CT_ZONE, Some(9), dnat_https)),
    ).await?;
    println!("  Table 6: in={EXTERNAL_PORT}, tcp_dst=443, new -> DNAT to {web_server}:8443");

    // New inbound SSH (22) -> DNAT to SSH server
    let dnat_ssh = NatConfig::dnat(ssh_server).port(22);
    conn.send_flow_sync(
        &Flow::add()
            .table(6)
            .priority(90)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x0800)
                    .ip_proto(6)
                    .tcp_dst(22)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct_nat(CT_COMMIT, CT_ZONE, Some(9), dnat_ssh)),
    ).await?;
    println!("  Table 6: in={EXTERNAL_PORT}, tcp_dst=22, new -> DNAT to {ssh_server}:22");

    // New inbound ICMP -> commit without NAT
    conn.send_flow_sync(
        &Flow::add()
            .table(6)
            .priority(85)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x0800)
                    .ip_proto(1)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(9))),
    ).await?;
    println!("  Table 6: in={EXTERNAL_PORT}, ICMP, new -> ct(commit, table=9)");

    // New inbound ICMPv6 -> commit without NAT
    conn.send_flow_sync(
        &Flow::add()
            .table(6)
            .priority(85)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x86dd)
                    .ip_proto(58)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(9))),
    ).await?;
    println!("  Table 6: in={EXTERNAL_PORT}, ICMPv6, new -> ct(commit, table=9)");

    // Default drop
    conn.send_flow_sync(&Flow::add().table(6).priority(0).actions(ActionList::new().drop())).await?;
    println!("  Table 6: default -> DROP");

    // ==========================================================================
    // Table 9: Output
    // ==========================================================================

    println!("\n--- Table 9: Output ---");

    conn.send_flow_sync(
        &Flow::add()
            .table(9)
            .priority(100)
            .match_fields(Match::new().in_port(INTERNAL_PORT))
            .actions(ActionList::new().output(EXTERNAL_PORT)),
    ).await?;
    println!("  Table 9: in={INTERNAL_PORT} -> output:{EXTERNAL_PORT}");

    conn.send_flow_sync(
        &Flow::add()
            .table(9)
            .priority(100)
            .match_fields(Match::new().in_port(EXTERNAL_PORT))
            .actions(ActionList::new().output(INTERNAL_PORT)),
    ).await?;
    println!("  Table 9: in={EXTERNAL_PORT} -> output:{INTERNAL_PORT}");

    // ==========================================================================
    // Summary
    // ==========================================================================

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                   Gateway Configuration Complete              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Pipeline:                                                     ║");
    println!("║   Tables 0-1: L2 (MAC learning, ARP/NDP proxy)                ║");
    println!("║   Tables 2-3: Zone classification + Connection Tracking       ║");
    println!("║   Tables 4-5: Firewall policy                                 ║");
    println!("║   Tables 6-8: NAT (SNAT outbound, DNAT inbound)               ║");
    println!("║   Table 9: Output                                             ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Features Active:                                              ║");
    println!("║   ✓ Dual-stack SNAT (IPv4 + IPv6)                             ║");
    println!("║   ✓ DNAT for HTTP/HTTPS/SSH                                   ║");
    println!("║   ✓ Stateful firewall                                         ║");
    println!("║   ✓ ARP/NDP proxy (via controller)                            ║");
    if args.no_learning {
        println!("║   ✗ MAC learning (disabled)                                   ║");
    } else {
        println!("║   ✓ MAC learning (internal side)                              ║");
    }
    println!("╚══════════════════════════════════════════════════════════════╝");

    println!("\nTo verify the flows:");
    println!("  podman exec rovs-ovsdb-test ovs-ofctl dump-flows br-test -O OpenFlow13");
    println!("\nTo test ARP proxy, run a controller:");
    println!("  CONTROLLER_ADDR=tcp:0.0.0.0:6653 cargo run -p rovs-ext --example arp_ndp_controller");

    if args.no_cleanup {
        println!("\nFlows left for inspection. Clean up with:");
        println!("  podman exec rovs-ovsdb-test ovs-ofctl del-flows br-test -O OpenFlow13");
    } else {
        println!("\nCleaning up flows...");
        for table in 0..=9 {
            conn.send_flow_sync(&Flow::delete().table(table)).await?;
        }
        println!("Cleanup complete.");
    }

    Ok(())
}
