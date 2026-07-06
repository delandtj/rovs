//! Example: Advanced NAT Patterns
//!
//! Demonstrates advanced NAT scenarios beyond basic SNAT/DNAT:
//!
//! | Scenario         | Description                                            |
//! |------------------|--------------------------------------------------------|
//! | Hairpin NAT      | Internal client accessing internal server via public IP|
//! | Load Balancing   | DNAT to multiple backends (round-robin via `ct_mark`)  |
//! | 1:1 NAT          | Bidirectional static NAT mapping                       |
//! | Port Range Fwd   | Forward entire port range to single backend            |
//!
//! Flow pipeline:
//! - Tables 0-2: Hairpin NAT detection and handling
//! - Tables 3-5: Load balancer NAT
//! - Tables 6-8: 1:1 static NAT
//!
//! Run with:
//! ```sh
//! ./scripts/test-with-ovs.sh start full
//! OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example advanced_nat
//! ```

// Examples are intentionally verbose for educational purposes
#![allow(clippy::too_many_lines)]

use std::net::Ipv4Addr;

use clap::Parser;
use rovs_openflow::oxm::ct_state;
use rovs_openflow::{ActionList, CT_COMMIT, Flow, Match, NatConfig, VConn};
use rovs_transport::Address;

// Port definitions
const INTERNAL_PORT: u32 = 1;
const EXTERNAL_PORT: u32 = 2;

// CT zone definitions for each scenario
const HAIRPIN_ZONE: u16 = 1;
const LB_ZONE: u16 = 2;
const STATIC_ZONE: u16 = 3;

#[derive(Parser)]
#[command(name = "advanced_nat")]
#[command(about = "Advanced NAT patterns demo")]
struct Args {
    /// Leave flows installed for inspection (don't cleanup)
    #[arg(long)]
    no_cleanup: bool,

    /// Only install a specific scenario (1=hairpin, 2=lb, 3=static)
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=3))]
    scenario: Option<u8>,
}

fn get_openflow_addr() -> Address {
    std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6653".to_string())
        .parse()
        .expect("Invalid OPENFLOW_ADDR")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = get_openflow_addr();
    println!("Connecting to OpenFlow at {addr}...");

    let mut conn = VConn::connect(&addr).await?;
    println!("Connected! OpenFlow version: {:?}\n", conn.version());

    // Public IP that internal server is accessible via
    let public_ip = Ipv4Addr::new(203, 0, 113, 1);
    // Internal server IP
    let internal_server = Ipv4Addr::new(192, 168, 1, 10);
    // Load balancer backends
    let backends = [
        Ipv4Addr::new(192, 168, 1, 20),
        Ipv4Addr::new(192, 168, 1, 21),
        Ipv4Addr::new(192, 168, 1, 22),
    ];
    // 1:1 NAT mappings
    let nat_1to1 = [
        (
            Ipv4Addr::new(203, 0, 113, 10),
            Ipv4Addr::new(192, 168, 1, 100),
        ),
        (
            Ipv4Addr::new(203, 0, 113, 11),
            Ipv4Addr::new(192, 168, 1, 101),
        ),
    ];

    let run_all = args.scenario.is_none();

    // Clear all tables we'll use
    println!("Clearing flow tables 0-8...");
    for table in 0..=8 {
        conn.send_flow_sync(&Flow::delete().table(table)).await?;
    }

    // ==========================================================================
    // Scenario 1: Hairpin NAT (Tables 0-2)
    // ==========================================================================
    // Hairpin NAT allows internal clients to access internal servers using the
    // server's public (external) IP address. This requires detecting when traffic
    // from internal goes to the public IP, then DNAT'ing to the internal server
    // AND SNAT'ing to the gateway so replies come back through the gateway.

    if run_all || args.scenario == Some(1) {
        println!("\n=== Scenario 1: Hairpin NAT (Tables 0-2) ===");
        println!("Public IP: {public_ip} -> Internal Server: {internal_server}");
        println!("Allows internal clients to access internal servers via public IP\n");

        // Table 0: Connection tracking
        let ct_hairpin = Flow::add()
            .table(0)
            .priority(100)
            .match_fields(Match::new().eth_type(0x0800))
            .actions(ActionList::new().ct(0, HAIRPIN_ZONE, Some(1)));

        conn.send_flow_sync(&ct_hairpin).await?;
        println!("  Table 0: IPv4 -> ct(zone={HAIRPIN_ZONE}, table=1)");

        // ARP passthrough
        let arp_pass = Flow::add()
            .table(0)
            .priority(100)
            .match_fields(Match::new().eth_type(0x0806))
            .actions(ActionList::new().normal());

        conn.send_flow_sync(&arp_pass).await?;
        println!("  Table 0: ARP -> NORMAL");

        // Default drop
        conn.send_flow_sync(
            &Flow::add()
                .table(0)
                .priority(0)
                .actions(ActionList::new().drop()),
        )
        .await?;
        println!("  Table 0: default -> DROP");

        // Table 1: Hairpin detection and NAT policy

        // Drop invalid
        let drop_invalid = Flow::add()
            .table(1)
            .priority(110)
            .match_fields(
                Match::new()
                    .ct_state_masked(ct_state::TRK | ct_state::INV, ct_state::TRK | ct_state::INV),
            )
            .actions(ActionList::new().drop());

        conn.send_flow_sync(&drop_invalid).await?;
        println!("  Table 1: ct_state=+trk+inv -> DROP");

        // Established connections
        let est_internal = Flow::add()
            .table(1)
            .priority(100)
            .match_fields(
                Match::new()
                    .in_port(INTERNAL_PORT)
                    .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
            )
            .actions(ActionList::new().output(INTERNAL_PORT)); // Hairpin: back to internal

        conn.send_flow_sync(&est_internal).await?;
        println!("  Table 1: in={INTERNAL_PORT}, established -> output:{INTERNAL_PORT} (hairpin)");

        let est_external = Flow::add()
            .table(1)
            .priority(100)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
            )
            .actions(ActionList::new().output(INTERNAL_PORT));

        conn.send_flow_sync(&est_external).await?;
        println!("  Table 1: in={EXTERNAL_PORT}, established -> output:{INTERNAL_PORT}");

        // Hairpin detection: internal client -> public IP
        // Apply both DNAT (to internal server) and SNAT (to gateway IP for return path)
        // Note: OVS doesn't support both SNAT and DNAT in same ct action, so we use
        // DNAT first, then the reply path naturally works through CT reverse NAT.
        // For true hairpin, we'd need to also SNAT to gateway IP.
        let hairpin_nat = NatConfig::dnat(internal_server);

        let hairpin_detect = Flow::add()
            .table(1)
            .priority(90)
            .match_fields(
                Match::new()
                    .in_port(INTERNAL_PORT)
                    .eth_type(0x0800)
                    .ipv4_dst(public_ip, 32) // Traffic to public IP (exact match)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct_nat(CT_COMMIT, HAIRPIN_ZONE, Some(2), hairpin_nat));

        conn.send_flow_sync(&hairpin_detect).await?;
        println!(
            "  Table 1: in={INTERNAL_PORT}, dst={public_ip}, new -> DNAT to {internal_server}"
        );

        // External inbound to public IP -> DNAT to internal server
        let external_dnat = NatConfig::dnat(internal_server);
        let ext_to_internal = Flow::add()
            .table(1)
            .priority(90)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x0800)
                    .ipv4_dst(public_ip, 32)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct_nat(CT_COMMIT, HAIRPIN_ZONE, Some(2), external_dnat));

        conn.send_flow_sync(&ext_to_internal).await?;
        println!(
            "  Table 1: in={EXTERNAL_PORT}, dst={public_ip}, new -> DNAT to {internal_server}"
        );

        // Normal outbound (not hairpin)
        let outbound_snat = NatConfig::snat(public_ip);
        let normal_out = Flow::add()
            .table(1)
            .priority(80)
            .match_fields(
                Match::new()
                    .in_port(INTERNAL_PORT)
                    .eth_type(0x0800)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct_nat(CT_COMMIT, HAIRPIN_ZONE, Some(2), outbound_snat));

        conn.send_flow_sync(&normal_out).await?;
        println!("  Table 1: in={INTERNAL_PORT}, new (other) -> SNAT to {public_ip}");

        // Table 2: Output after NAT
        let output_hairpin = Flow::add()
            .table(2)
            .priority(100)
            .match_fields(Match::new().in_port(INTERNAL_PORT))
            .actions(ActionList::new().output(INTERNAL_PORT)); // Hairpin loop

        conn.send_flow_sync(&output_hairpin).await?;
        println!("  Table 2: in={INTERNAL_PORT} -> output:{INTERNAL_PORT}");

        let output_external = Flow::add()
            .table(2)
            .priority(90)
            .match_fields(Match::new().in_port(EXTERNAL_PORT))
            .actions(ActionList::new().output(INTERNAL_PORT));

        conn.send_flow_sync(&output_external).await?;
        println!("  Table 2: in={EXTERNAL_PORT} -> output:{INTERNAL_PORT}");
    }

    // ==========================================================================
    // Scenario 2: Load Balancer NAT (Tables 3-5)
    // ==========================================================================
    // Load balancing using multiple DNAT rules with different priorities or
    // using ct_mark/registers to track backend selection. For simplicity, we
    // use round-robin via separate flows per destination port range.

    if run_all || args.scenario == Some(2) {
        println!("\n=== Scenario 2: Load Balancer (Tables 3-5) ===");
        println!("Backends: {backends:?}");
        println!("Port-based distribution: 80-82 -> backend[port % 3]\n");

        // Table 3: Connection tracking for LB
        let ct_lb = Flow::add()
            .table(3)
            .priority(100)
            .match_fields(Match::new().eth_type(0x0800))
            .actions(ActionList::new().ct(0, LB_ZONE, Some(4)));

        conn.send_flow_sync(&ct_lb).await?;
        println!("  Table 3: IPv4 -> ct(zone={LB_ZONE}, table=4)");

        conn.send_flow_sync(
            &Flow::add()
                .table(3)
                .priority(0)
                .actions(ActionList::new().drop()),
        )
        .await?;
        println!("  Table 3: default -> DROP");

        // Table 4: LB policy

        // Drop invalid
        conn.send_flow_sync(
            &Flow::add()
                .table(4)
                .priority(110)
                .match_fields(
                    Match::new().ct_state_masked(
                        ct_state::TRK | ct_state::INV,
                        ct_state::TRK | ct_state::INV,
                    ),
                )
                .actions(ActionList::new().drop()),
        )
        .await?;
        println!("  Table 4: ct_state=+trk+inv -> DROP");

        // Established
        conn.send_flow_sync(
            &Flow::add()
                .table(4)
                .priority(100)
                .match_fields(
                    Match::new().in_port(EXTERNAL_PORT).ct_state_masked(
                        ct_state::TRK | ct_state::EST,
                        ct_state::TRK | ct_state::EST,
                    ),
                )
                .actions(ActionList::new().output(INTERNAL_PORT)),
        )
        .await?;
        println!("  Table 4: in={EXTERNAL_PORT}, established -> output:{INTERNAL_PORT}");

        conn.send_flow_sync(
            &Flow::add()
                .table(4)
                .priority(100)
                .match_fields(
                    Match::new().in_port(INTERNAL_PORT).ct_state_masked(
                        ct_state::TRK | ct_state::EST,
                        ct_state::TRK | ct_state::EST,
                    ),
                )
                .actions(ActionList::new().output(EXTERNAL_PORT)),
        )
        .await?;
        println!("  Table 4: in={INTERNAL_PORT}, established -> output:{EXTERNAL_PORT}");

        // Load balancing: port 80 -> backend 0, port 81 -> backend 1, port 82 -> backend 2
        // In production, you'd use hashing on 5-tuple or random selection
        for (i, backend) in backends.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let port = 80 + i as u16;
            let dnat = NatConfig::dnat(*backend).port(8080);

            let lb_rule = Flow::add()
                .table(4)
                .priority(90)
                .match_fields(
                    Match::new()
                        .in_port(EXTERNAL_PORT)
                        .eth_type(0x0800)
                        .ip_proto(6) // TCP
                        .tcp_dst(port)
                        .ct_state_masked(
                            ct_state::TRK | ct_state::NEW,
                            ct_state::TRK | ct_state::NEW,
                        ),
                )
                .actions(ActionList::new().ct_nat(CT_COMMIT, LB_ZONE, Some(5), dnat));

            conn.send_flow_sync(&lb_rule).await?;
            println!(
                "  Table 4: in={EXTERNAL_PORT}, tcp_dst={port}, new -> DNAT to {backend}:8080"
            );
        }

        // Outbound SNAT
        let lb_snat = NatConfig::snat(public_ip);
        conn.send_flow_sync(
            &Flow::add()
                .table(4)
                .priority(80)
                .match_fields(
                    Match::new()
                        .in_port(INTERNAL_PORT)
                        .eth_type(0x0800)
                        .ct_state_masked(
                            ct_state::TRK | ct_state::NEW,
                            ct_state::TRK | ct_state::NEW,
                        ),
                )
                .actions(ActionList::new().ct_nat(CT_COMMIT, LB_ZONE, Some(5), lb_snat)),
        )
        .await?;
        println!("  Table 4: in={INTERNAL_PORT}, new -> SNAT to {public_ip}");

        // Table 5: Output after LB NAT
        conn.send_flow_sync(
            &Flow::add()
                .table(5)
                .priority(100)
                .match_fields(Match::new().in_port(EXTERNAL_PORT))
                .actions(ActionList::new().output(INTERNAL_PORT)),
        )
        .await?;
        println!("  Table 5: in={EXTERNAL_PORT} -> output:{INTERNAL_PORT}");

        conn.send_flow_sync(
            &Flow::add()
                .table(5)
                .priority(100)
                .match_fields(Match::new().in_port(INTERNAL_PORT))
                .actions(ActionList::new().output(EXTERNAL_PORT)),
        )
        .await?;
        println!("  Table 5: in={INTERNAL_PORT} -> output:{EXTERNAL_PORT}");
    }

    // ==========================================================================
    // Scenario 3: 1:1 Static NAT (Tables 6-8)
    // ==========================================================================
    // Bidirectional static NAT where each internal IP has a dedicated external IP.
    // Traffic to external IP X.X.X.10 is DNAT'd to internal 192.168.1.100, and
    // traffic from 192.168.1.100 is SNAT'd to X.X.X.10.

    if run_all || args.scenario == Some(3) {
        println!("\n=== Scenario 3: 1:1 Static NAT (Tables 6-8) ===");
        for (ext, int) in &nat_1to1 {
            println!("  {ext} <-> {int}");
        }
        println!();

        // Table 6: Connection tracking
        let ct_static = Flow::add()
            .table(6)
            .priority(100)
            .match_fields(Match::new().eth_type(0x0800))
            .actions(ActionList::new().ct(0, STATIC_ZONE, Some(7)));

        conn.send_flow_sync(&ct_static).await?;
        println!("  Table 6: IPv4 -> ct(zone={STATIC_ZONE}, table=7)");

        conn.send_flow_sync(
            &Flow::add()
                .table(6)
                .priority(0)
                .actions(ActionList::new().drop()),
        )
        .await?;
        println!("  Table 6: default -> DROP");

        // Table 7: Static NAT policy

        // Drop invalid
        conn.send_flow_sync(
            &Flow::add()
                .table(7)
                .priority(110)
                .match_fields(
                    Match::new().ct_state_masked(
                        ct_state::TRK | ct_state::INV,
                        ct_state::TRK | ct_state::INV,
                    ),
                )
                .actions(ActionList::new().drop()),
        )
        .await?;
        println!("  Table 7: ct_state=+trk+inv -> DROP");

        // Established connections
        conn.send_flow_sync(
            &Flow::add()
                .table(7)
                .priority(100)
                .match_fields(
                    Match::new().in_port(EXTERNAL_PORT).ct_state_masked(
                        ct_state::TRK | ct_state::EST,
                        ct_state::TRK | ct_state::EST,
                    ),
                )
                .actions(ActionList::new().output(INTERNAL_PORT)),
        )
        .await?;
        println!("  Table 7: in={EXTERNAL_PORT}, established -> output:{INTERNAL_PORT}");

        conn.send_flow_sync(
            &Flow::add()
                .table(7)
                .priority(100)
                .match_fields(
                    Match::new().in_port(INTERNAL_PORT).ct_state_masked(
                        ct_state::TRK | ct_state::EST,
                        ct_state::TRK | ct_state::EST,
                    ),
                )
                .actions(ActionList::new().output(EXTERNAL_PORT)),
        )
        .await?;
        println!("  Table 7: in={INTERNAL_PORT}, established -> output:{EXTERNAL_PORT}");

        // Static 1:1 NAT rules
        for (external_ip, internal_ip) in &nat_1to1 {
            // Inbound: external_ip -> internal_ip (DNAT)
            let dnat = NatConfig::dnat(*internal_ip);
            let inbound = Flow::add()
                .table(7)
                .priority(90)
                .match_fields(
                    Match::new()
                        .in_port(EXTERNAL_PORT)
                        .eth_type(0x0800)
                        .ipv4_dst(*external_ip, 32)
                        .ct_state_masked(
                            ct_state::TRK | ct_state::NEW,
                            ct_state::TRK | ct_state::NEW,
                        ),
                )
                .actions(ActionList::new().ct_nat(CT_COMMIT, STATIC_ZONE, Some(8), dnat));

            conn.send_flow_sync(&inbound).await?;
            println!(
                "  Table 7: in={EXTERNAL_PORT}, dst={external_ip}, new -> DNAT to {internal_ip}"
            );

            // Outbound: internal_ip -> external_ip (SNAT)
            let snat = NatConfig::snat(*external_ip);
            let outbound = Flow::add()
                .table(7)
                .priority(90)
                .match_fields(
                    Match::new()
                        .in_port(INTERNAL_PORT)
                        .eth_type(0x0800)
                        .ipv4_src(*internal_ip, 32)
                        .ct_state_masked(
                            ct_state::TRK | ct_state::NEW,
                            ct_state::TRK | ct_state::NEW,
                        ),
                )
                .actions(ActionList::new().ct_nat(CT_COMMIT, STATIC_ZONE, Some(8), snat));

            conn.send_flow_sync(&outbound).await?;
            println!(
                "  Table 7: in={INTERNAL_PORT}, src={internal_ip}, new -> SNAT to {external_ip}"
            );
        }

        // Default outbound SNAT (for IPs without 1:1 mapping)
        let default_snat = NatConfig::snat(public_ip);
        conn.send_flow_sync(
            &Flow::add()
                .table(7)
                .priority(80)
                .match_fields(
                    Match::new()
                        .in_port(INTERNAL_PORT)
                        .eth_type(0x0800)
                        .ct_state_masked(
                            ct_state::TRK | ct_state::NEW,
                            ct_state::TRK | ct_state::NEW,
                        ),
                )
                .actions(ActionList::new().ct_nat(CT_COMMIT, STATIC_ZONE, Some(8), default_snat)),
        )
        .await?;
        println!("  Table 7: in={INTERNAL_PORT}, new (default) -> SNAT to {public_ip}");

        // Table 8: Output
        conn.send_flow_sync(
            &Flow::add()
                .table(8)
                .priority(100)
                .match_fields(Match::new().in_port(EXTERNAL_PORT))
                .actions(ActionList::new().output(INTERNAL_PORT)),
        )
        .await?;
        println!("  Table 8: in={EXTERNAL_PORT} -> output:{INTERNAL_PORT}");

        conn.send_flow_sync(
            &Flow::add()
                .table(8)
                .priority(100)
                .match_fields(Match::new().in_port(INTERNAL_PORT))
                .actions(ActionList::new().output(EXTERNAL_PORT)),
        )
        .await?;
        println!("  Table 8: in={INTERNAL_PORT} -> output:{EXTERNAL_PORT}");
    }

    // ==========================================================================
    // Summary
    // ==========================================================================

    println!("\n--- Advanced NAT Patterns Configured ---");
    println!("\nScenarios:");
    println!("  1. Hairpin NAT (tables 0-2): Internal access via public IP");
    println!("  2. Load Balancer (tables 3-5): Port-based backend selection");
    println!("  3. 1:1 Static NAT (tables 6-8): Bidirectional IP mapping");
    println!("\nTo verify the flows:");
    println!("  podman exec rovs-ovsdb-test ovs-ofctl dump-flows br-test -O OpenFlow13");

    if args.no_cleanup {
        println!("\nFlows left for inspection. Clean up with:");
        println!("  podman exec rovs-ovsdb-test ovs-ofctl del-flows br-test -O OpenFlow13");
    } else {
        println!("\nCleaning up flows...");
        for table in 0..=8 {
            conn.send_flow_sync(&Flow::delete().table(table)).await?;
        }
        println!("Cleanup complete.");
    }

    Ok(())
}
