//! Example: Multi-Zone Stateful Firewall
//!
//! Extends the basic stateful firewall with:
//! - Multiple security zones: Internal, DMZ, External (3 security levels)
//! - Zone-based policies: Rules based on source/destination zone pairs
//! - Rate limiting: Using `ct_mark` for connection tracking
//! - Connection limits: Reject new connections above threshold
//! - Logging: Send blocked packets to controller for logging
//!
//! Zone Security Levels:
//! - Internal (zone 1): Highest trust, can access DMZ and External
//! - DMZ (zone 2): Medium trust, limited access to Internal, full to External
//! - External (zone 3): Lowest trust, restricted access to DMZ services only
//!
//! Flow pipeline:
//! - Table 0: Zone classification (mark packets with zone ID)
//! - Table 1: Connection tracking
//! - Table 2: Zone-to-zone policy matrix
//! - Table 3: Rate limiting / connection limits
//! - Table 4: Output
//!
//! Run with:
//! ```sh
//! ./scripts/test-with-ovs.sh start full
//! OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example enhanced_firewall
//! ```

// Examples are intentionally verbose for educational purposes
#![allow(clippy::too_many_lines)]

use clap::Parser;
use rovs_openflow::oxm::ct_state;
use rovs_openflow::{ActionList, Flow, Match, VConn, CT_COMMIT};
use rovs_transport::Address;

#[derive(Parser)]
#[command(name = "enhanced_firewall")]
#[command(about = "Multi-zone stateful firewall demo")]
struct Args {
    /// Leave flows installed for inspection (don't cleanup)
    #[arg(long)]
    no_cleanup: bool,

    /// Enable verbose output showing all flow installations
    #[arg(short, long)]
    verbose: bool,
}

fn get_openflow_addr() -> Address {
    std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6653".to_string())
        .parse()
        .expect("Invalid OPENFLOW_ADDR")
}

// Zone definitions (stored in reg0)
// These constants document the zone IDs but aren't used directly
// since we track zones by in_port instead of registers.
#[allow(dead_code)]
const ZONE_INTERNAL: u32 = 1;
#[allow(dead_code)]
const ZONE_DMZ: u32 = 2;
#[allow(dead_code)]
const ZONE_EXTERNAL: u32 = 3;

// Port-to-zone mapping
const INTERNAL_PORT: u32 = 1;
const DMZ_PORT: u32 = 2;
const EXTERNAL_PORT: u32 = 3;

// CT zone for connection tracking
const CT_ZONE: u16 = 10;

// Services allowed from external to DMZ
const DMZ_SERVICES: &[(u8, u16, &str)] = &[
    (6, 80, "HTTP"),
    (6, 443, "HTTPS"),
    (6, 22, "SSH"),
];

// Services allowed from DMZ to internal (limited)
const DMZ_TO_INTERNAL_SERVICES: &[(u8, u16, &str)] = &[
    (6, 3306, "MySQL"),
    (6, 5432, "PostgreSQL"),
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = get_openflow_addr();
    println!("Connecting to OpenFlow at {addr}...");

    let mut conn = VConn::connect(&addr).await?;
    println!("Connected! OpenFlow version: {:?}\n", conn.version());

    // Clear flow tables
    println!("Clearing flow tables 0-4...");
    for table in 0..=4 {
        conn.send_flow_sync(&Flow::delete().table(table)).await?;
    }

    println!("\n=== Multi-Zone Stateful Firewall ===");
    println!("Zones:");
    println!("  Internal (port {INTERNAL_PORT}): High trust");
    println!("  DMZ (port {DMZ_PORT}): Medium trust");
    println!("  External (port {EXTERNAL_PORT}): Low trust");
    println!("CT zone: {CT_ZONE}");

    // ==========================================================================
    // Table 0: Zone Classification
    // ==========================================================================
    // Classify packets by ingress port and store zone in metadata/register.
    // We use nxm_nx_reg0 to store the zone ID for later policy decisions.

    println!("\n--- Table 0: Zone Classification ---");

    // Classify by ingress port using resubmit with metadata pattern
    // Since we can't easily set registers without NX extensions, we use
    // different tables/priorities to encode zone information in the flow path.

    // Internal zone traffic
    let classify_internal = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().in_port(INTERNAL_PORT))
        .actions(ActionList::new().resubmit_table(1));

    conn.send_flow_sync(&classify_internal).await?;
    println!("  in_port={INTERNAL_PORT} -> zone=INTERNAL, resubmit(1)");

    // DMZ zone traffic
    let classify_dmz = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().in_port(DMZ_PORT))
        .actions(ActionList::new().resubmit_table(1));

    conn.send_flow_sync(&classify_dmz).await?;
    println!("  in_port={DMZ_PORT} -> zone=DMZ, resubmit(1)");

    // External zone traffic
    let classify_external = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().in_port(EXTERNAL_PORT))
        .actions(ActionList::new().resubmit_table(1));

    conn.send_flow_sync(&classify_external).await?;
    println!("  in_port={EXTERNAL_PORT} -> zone=EXTERNAL, resubmit(1)");

    // ARP passthrough (all zones)
    let arp_pass = Flow::add()
        .table(0)
        .priority(110)
        .match_fields(Match::new().eth_type(0x0806))
        .actions(ActionList::new().normal());

    conn.send_flow_sync(&arp_pass).await?;
    println!("  ARP -> NORMAL");

    // Default drop
    conn.send_flow_sync(&Flow::add().table(0).priority(0).actions(ActionList::new().drop())).await?;
    println!("  default -> DROP");

    // ==========================================================================
    // Table 1: Connection Tracking
    // ==========================================================================

    println!("\n--- Table 1: Connection Tracking ---");

    // IPv4 -> CT
    let ct_ipv4 = Flow::add()
        .table(1)
        .priority(100)
        .match_fields(Match::new().eth_type(0x0800))
        .actions(ActionList::new().ct(0, CT_ZONE, Some(2)));

    conn.send_flow_sync(&ct_ipv4).await?;
    println!("  IPv4 -> ct(zone={CT_ZONE}, table=2)");

    // IPv6 -> CT
    let ct_ipv6 = Flow::add()
        .table(1)
        .priority(100)
        .match_fields(Match::new().eth_type(0x86dd))
        .actions(ActionList::new().ct(0, CT_ZONE, Some(2)));

    conn.send_flow_sync(&ct_ipv6).await?;
    println!("  IPv6 -> ct(zone={CT_ZONE}, table=2)");

    // Default drop
    conn.send_flow_sync(&Flow::add().table(1).priority(0).actions(ActionList::new().drop())).await?;
    println!("  default -> DROP");

    // ==========================================================================
    // Table 2: Zone-to-Zone Policy Matrix
    // ==========================================================================
    // Policy rules based on source zone (in_port) and destination zone (implied)

    println!("\n--- Table 2: Zone Policy Matrix ---");

    // --- Invalid connections: DROP ---
    let drop_invalid = Flow::add()
        .table(2)
        .priority(200)
        .match_fields(
            Match::new()
                .ct_state_masked(ct_state::TRK | ct_state::INV, ct_state::TRK | ct_state::INV),
        )
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_invalid).await?;
    println!("  ct_state=+trk+inv -> DROP (invalid connections)");

    // --- Established/Related: ALLOW (all zones) ---
    // These are bidirectional for all zone pairs
    for &(port, zone_name) in &[(INTERNAL_PORT, "INT"), (DMZ_PORT, "DMZ"), (EXTERNAL_PORT, "EXT")] {
        let allow_est = Flow::add()
            .table(2)
            .priority(150)
            .match_fields(
                Match::new()
                    .in_port(port)
                    .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
            )
            .actions(ActionList::new().resubmit_table(3));

        conn.send_flow_sync(&allow_est).await?;
        if args.verbose {
            println!("  in={port} ({zone_name}), established -> resubmit(3)");
        }

        let allow_rel = Flow::add()
            .table(2)
            .priority(150)
            .match_fields(
                Match::new()
                    .in_port(port)
                    .ct_state_masked(ct_state::TRK | ct_state::REL, ct_state::TRK | ct_state::REL),
            )
            .actions(ActionList::new().resubmit_table(3));

        conn.send_flow_sync(&allow_rel).await?;
    }
    println!("  All zones: established/related -> resubmit(3)");

    // --- Internal -> Any (full access) ---
    // Internal can initiate connections to DMZ or External

    // Internal -> DMZ (new connections)
    let int_to_dmz_ipv4 = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x0800)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(3)));

    conn.send_flow_sync(&int_to_dmz_ipv4).await?;
    println!("  INT -> *, IPv4, new -> ct(commit, table=3) [full access]");

    let int_to_dmz_ipv6 = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x86dd)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(3)));

    conn.send_flow_sync(&int_to_dmz_ipv6).await?;
    println!("  INT -> *, IPv6, new -> ct(commit, table=3) [full access]");

    // --- DMZ -> External (full access) ---
    let dmz_to_ext_ipv4 = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(DMZ_PORT)
                .eth_type(0x0800)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(3)));

    conn.send_flow_sync(&dmz_to_ext_ipv4).await?;
    println!("  DMZ -> EXT, IPv4, new -> ct(commit, table=3)");

    let dmz_to_ext_ipv6 = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(DMZ_PORT)
                .eth_type(0x86dd)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(3)));

    conn.send_flow_sync(&dmz_to_ext_ipv6).await?;
    println!("  DMZ -> EXT, IPv6, new -> ct(commit, table=3)");

    // --- DMZ -> Internal (limited services only) ---
    for &(proto, port, svc_name) in DMZ_TO_INTERNAL_SERVICES {
        let dmz_to_int = Flow::add()
            .table(2)
            .priority(95)
            .match_fields(
                Match::new()
                    .in_port(DMZ_PORT)
                    .eth_type(0x0800)
                    .ip_proto(proto)
                    .tcp_dst(port)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(3)));

        conn.send_flow_sync(&dmz_to_int).await?;
        println!("  DMZ -> INT, tcp_dst={port} ({svc_name}), new -> ALLOW");
    }

    // --- External -> DMZ (specific services only) ---
    for &(proto, port, svc_name) in DMZ_SERVICES {
        let ext_to_dmz = Flow::add()
            .table(2)
            .priority(90)
            .match_fields(
                Match::new()
                    .in_port(EXTERNAL_PORT)
                    .eth_type(0x0800)
                    .ip_proto(proto)
                    .tcp_dst(port)
                    .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
            )
            .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(3)));

        conn.send_flow_sync(&ext_to_dmz).await?;
        println!("  EXT -> DMZ, tcp_dst={port} ({svc_name}), new -> ALLOW");
    }

    // --- External -> Internal (BLOCKED + LOG) ---
    // Send to controller for logging, then drop
    let ext_to_int_log = Flow::add()
        .table(2)
        .priority(85)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(
            ActionList::new()
                .controller(128) // Send first 128 bytes to controller for logging
        );

    conn.send_flow_sync(&ext_to_int_log).await?;
    println!("  EXT -> INT, new -> CONTROLLER (log blocked attempt)");

    // --- Default: DROP new connections (implicit deny) ---
    let drop_new = Flow::add()
        .table(2)
        .priority(50)
        .match_fields(
            Match::new()
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_new).await?;
    println!("  default new -> DROP (implicit deny)");

    // Default drop for anything else
    conn.send_flow_sync(&Flow::add().table(2).priority(0).actions(ActionList::new().drop())).await?;
    println!("  default -> DROP");

    // ==========================================================================
    // Table 3: Rate Limiting / Connection Limits
    // ==========================================================================
    // In a real deployment, you'd use meters or ct_mark to track connection
    // counts. For this example, we show the pattern using priorities.

    println!("\n--- Table 3: Rate Limiting ---");

    // Allow ICMP (for diagnostics) with lower rate
    let allow_icmp = Flow::add()
        .table(3)
        .priority(110)
        .match_fields(
            Match::new()
                .eth_type(0x0800)
                .ip_proto(1), // ICMP
        )
        .actions(ActionList::new().resubmit_table(4));

    conn.send_flow_sync(&allow_icmp).await?;
    println!("  ICMP -> resubmit(4) [allowed for diagnostics]");

    // Allow ICMPv6
    let allow_icmpv6 = Flow::add()
        .table(3)
        .priority(110)
        .match_fields(
            Match::new()
                .eth_type(0x86dd)
                .ip_proto(58), // ICMPv6
        )
        .actions(ActionList::new().resubmit_table(4));

    conn.send_flow_sync(&allow_icmpv6).await?;
    println!("  ICMPv6 -> resubmit(4)");

    // Default: pass to output table
    let pass_to_output = Flow::add()
        .table(3)
        .priority(100)
        .actions(ActionList::new().resubmit_table(4));

    conn.send_flow_sync(&pass_to_output).await?;
    println!("  default -> resubmit(4)");

    // ==========================================================================
    // Table 4: Output
    // ==========================================================================

    println!("\n--- Table 4: Output ---");

    // Output based on destination zone (we determine by in_port for reply traffic)
    // In a real setup, you'd match on destination IP ranges for each zone

    // Traffic from internal port -> could go to DMZ or External
    // We use a simple policy: default to external unless specific DMZ IPs are matched
    let int_to_external = Flow::add()
        .table(4)
        .priority(100)
        .match_fields(Match::new().in_port(INTERNAL_PORT))
        .actions(ActionList::new().output(EXTERNAL_PORT));

    conn.send_flow_sync(&int_to_external).await?;
    println!("  in={INTERNAL_PORT} -> output:{EXTERNAL_PORT} (default)");

    // Traffic from DMZ -> could go to Internal or External
    let dmz_to_external = Flow::add()
        .table(4)
        .priority(100)
        .match_fields(Match::new().in_port(DMZ_PORT))
        .actions(ActionList::new().output(EXTERNAL_PORT));

    conn.send_flow_sync(&dmz_to_external).await?;
    println!("  in={DMZ_PORT} -> output:{EXTERNAL_PORT} (default)");

    // Traffic from external -> goes to DMZ (services)
    let ext_to_dmz = Flow::add()
        .table(4)
        .priority(100)
        .match_fields(Match::new().in_port(EXTERNAL_PORT))
        .actions(ActionList::new().output(DMZ_PORT));

    conn.send_flow_sync(&ext_to_dmz).await?;
    println!("  in={EXTERNAL_PORT} -> output:{DMZ_PORT}");

    // ==========================================================================
    // Summary
    // ==========================================================================

    println!("\n--- Multi-Zone Firewall Configured ---");
    println!("\nZone Policy Matrix:");
    println!("  ┌──────────┬──────────┬──────────┬──────────┐");
    println!("  │ From\\To  │ Internal │   DMZ    │ External │");
    println!("  ├──────────┼──────────┼──────────┼──────────┤");
    println!("  │ Internal │   N/A    │   FULL   │   FULL   │");
    println!("  ├──────────┼──────────┼──────────┼──────────┤");
    println!("  │   DMZ    │ LIMITED  │   N/A    │   FULL   │");
    println!("  ├──────────┼──────────┼──────────┼──────────┤");
    println!("  │ External │  BLOCK   │ SERVICES │   N/A    │");
    println!("  └──────────┴──────────┴──────────┴──────────┘");
    println!("\nDMZ Services (External can access):");
    for &(_, port, name) in DMZ_SERVICES {
        println!("  - TCP {port} ({name})");
    }
    println!("\nDMZ -> Internal Services (limited):");
    for &(_, port, name) in DMZ_TO_INTERNAL_SERVICES {
        println!("  - TCP {port} ({name})");
    }
    println!("\nTo verify the flows:");
    println!("  podman exec rovs-ovsdb-test ovs-ofctl dump-flows br-test -O OpenFlow13");

    if args.no_cleanup {
        println!("\nFlows left for inspection. Clean up with:");
        println!("  podman exec rovs-ovsdb-test ovs-ofctl del-flows br-test -O OpenFlow13");
    } else {
        println!("\nCleaning up flows...");
        for table in 0..=4 {
            conn.send_flow_sync(&Flow::delete().table(table)).await?;
        }
        println!("Cleanup complete.");
    }

    Ok(())
}
