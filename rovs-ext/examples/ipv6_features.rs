//! Example: IPv6-Specific Features
//!
//! Demonstrates IPv6-specific capabilities in Open vSwitch:
//! - NDP proxy (Neighbor Discovery Protocol)
//! - `NAT66` (IPv6-to-IPv6 Network Address Translation)
//! - IPv6 firewall with `ICMPv6` handling
//! - Dual-stack policies (different rules for IPv4 vs IPv6)
//!
//! Flow pipeline:
//! - Tables 0-1: `ICMPv6` handling (NDP sent to controller)
//! - Tables 2-3: IPv6 connection tracking
//! - Tables 4-5: IPv6 firewall policies
//! - Tables 6-7: `NAT66`
//!
//! Run with:
//! ```sh
//! ./scripts/test-with-ovs.sh start full
//! OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example ipv6_features
//! ```

// Examples are intentionally verbose for educational purposes
#![allow(clippy::too_many_lines)]

use std::net::Ipv6Addr;

use clap::Parser;
use rovs_openflow::oxm::ct_state;
use rovs_openflow::{ActionList, Flow, Match, NatConfig, VConn, CT_COMMIT};
use rovs_transport::Address;

#[derive(Parser)]
#[command(name = "ipv6_features")]
#[command(about = "IPv6-specific OpenFlow features demo")]
struct Args {
    /// Leave flows installed for inspection (don't cleanup)
    #[arg(long)]
    no_cleanup: bool,
}

fn get_openflow_addr() -> Address {
    std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6653".to_string())
        .parse()
        .expect("Invalid OPENFLOW_ADDR")
}

// Port definitions
const INTERNAL_PORT: u32 = 1;
const EXTERNAL_PORT: u32 = 2;
const CT_ZONE: u16 = 6;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = get_openflow_addr();
    println!("Connecting to OpenFlow at {addr}...");

    let mut conn = VConn::connect(&addr).await?;
    println!("Connected! OpenFlow version: {:?}\n", conn.version());

    // Internal prefix: fd00::/64 (ULA)
    // External address for NAT66: 2001:db8::1
    let external_ipv6 = Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1);

    // Clear flow tables
    println!("Clearing flow tables 0-7...");
    for table in 0..=7 {
        conn.send_flow_sync(&Flow::delete().table(table)).await?;
    }

    println!("\n=== IPv6 Features Configuration ===");
    println!("Internal port: {INTERNAL_PORT}");
    println!("External port: {EXTERNAL_PORT}");
    println!("External IPv6: {external_ipv6}");
    println!("CT zone: {CT_ZONE}");

    // ==========================================================================
    // Tables 0-1: ICMPv6 Handling (NDP)
    // ==========================================================================
    // Neighbor Discovery Protocol packets are sent to the controller for
    // proxy handling. Other ICMPv6 (echo, etc.) goes through normal processing.

    println!("\n--- Tables 0-1: ICMPv6 / NDP Handling ---");

    // Table 0: Classify IPv6 traffic
    // IPv4 bypasses ICMPv6 handling
    let ipv4_bypass = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().eth_type(0x0800))
        .actions(ActionList::new().resubmit_table(2)); // Skip to CT

    conn.send_flow_sync(&ipv4_bypass).await?;
    println!("  Table 0: IPv4 -> resubmit(2)");

    // ARP bypass
    let arp_bypass = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().eth_type(0x0806))
        .actions(ActionList::new().normal());

    conn.send_flow_sync(&arp_bypass).await?;
    println!("  Table 0: ARP -> NORMAL");

    // IPv6 goes to table 1 for ICMPv6 inspection
    let ipv6_to_icmpv6 = Flow::add()
        .table(0)
        .priority(90)
        .match_fields(Match::new().eth_type(0x86dd))
        .actions(ActionList::new().resubmit_table(1));

    conn.send_flow_sync(&ipv6_to_icmpv6).await?;
    println!("  Table 0: IPv6 -> resubmit(1)");

    // Default drop
    let drop_default_t0 = Flow::add()
        .table(0)
        .priority(0)
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_default_t0).await?;
    println!("  Table 0: default -> DROP");

    // Table 1: ICMPv6 inspection
    // Neighbor Solicitation (type 135) -> Controller for NDP proxy
    let ns_to_controller = Flow::add()
        .table(1)
        .priority(100)
        .match_fields(
            Match::new()
                .eth_type(0x86dd)
                .ip_proto(58) // ICMPv6
                .icmpv6_type(135), // Neighbor Solicitation
        )
        .actions(ActionList::new().controller(0xffff));

    conn.send_flow_sync(&ns_to_controller).await?;
    println!("  Table 1: ICMPv6 NS (type 135) -> CONTROLLER");

    // Neighbor Advertisement (type 136) -> normal forwarding
    let na_normal = Flow::add()
        .table(1)
        .priority(100)
        .match_fields(
            Match::new()
                .eth_type(0x86dd)
                .ip_proto(58)
                .icmpv6_type(136), // Neighbor Advertisement
        )
        .actions(ActionList::new().normal());

    conn.send_flow_sync(&na_normal).await?;
    println!("  Table 1: ICMPv6 NA (type 136) -> NORMAL");

    // Router Solicitation (type 133) -> Controller (optional RA proxy)
    let rs_to_controller = Flow::add()
        .table(1)
        .priority(100)
        .match_fields(
            Match::new()
                .eth_type(0x86dd)
                .ip_proto(58)
                .icmpv6_type(133), // Router Solicitation
        )
        .actions(ActionList::new().controller(0xffff));

    conn.send_flow_sync(&rs_to_controller).await?;
    println!("  Table 1: ICMPv6 RS (type 133) -> CONTROLLER");

    // Router Advertisement (type 134) -> normal forwarding
    let ra_normal = Flow::add()
        .table(1)
        .priority(100)
        .match_fields(
            Match::new()
                .eth_type(0x86dd)
                .ip_proto(58)
                .icmpv6_type(134), // Router Advertisement
        )
        .actions(ActionList::new().normal());

    conn.send_flow_sync(&ra_normal).await?;
    println!("  Table 1: ICMPv6 RA (type 134) -> NORMAL");

    // ICMPv6 Echo Request/Reply -> continue to CT
    let echo_to_ct = Flow::add()
        .table(1)
        .priority(90)
        .match_fields(
            Match::new()
                .eth_type(0x86dd)
                .ip_proto(58), // All other ICMPv6
        )
        .actions(ActionList::new().resubmit_table(2));

    conn.send_flow_sync(&echo_to_ct).await?;
    println!("  Table 1: ICMPv6 (other) -> resubmit(2)");

    // Non-ICMPv6 IPv6 traffic -> CT
    let ipv6_to_ct = Flow::add()
        .table(1)
        .priority(50)
        .match_fields(Match::new().eth_type(0x86dd))
        .actions(ActionList::new().resubmit_table(2));

    conn.send_flow_sync(&ipv6_to_ct).await?;
    println!("  Table 1: IPv6 (non-ICMPv6) -> resubmit(2)");

    // ==========================================================================
    // Tables 2-3: IPv6 Connection Tracking
    // ==========================================================================

    println!("\n--- Tables 2-3: IPv6 Connection Tracking ---");

    // Table 2: Send IPv6 traffic through CT
    let ct_ipv6 = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(Match::new().eth_type(0x86dd))
        .actions(ActionList::new().ct(0, CT_ZONE, Some(3)));

    conn.send_flow_sync(&ct_ipv6).await?;
    println!("  Table 2: IPv6 -> ct(zone={CT_ZONE}, table=3)");

    // Table 2: IPv4 CT (for dual-stack)
    let ct_ipv4 = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(Match::new().eth_type(0x0800))
        .actions(ActionList::new().ct(0, CT_ZONE, Some(3)));

    conn.send_flow_sync(&ct_ipv4).await?;
    println!("  Table 2: IPv4 -> ct(zone={CT_ZONE}, table=3)");

    // Table 2: Default drop
    let drop_default_t2 = Flow::add()
        .table(2)
        .priority(0)
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_default_t2).await?;
    println!("  Table 2: default -> DROP");

    // Table 3: CT state evaluation -> firewall
    // Invalid connections
    let drop_invalid = Flow::add()
        .table(3)
        .priority(110)
        .match_fields(
            Match::new()
                .ct_state_masked(ct_state::TRK | ct_state::INV, ct_state::TRK | ct_state::INV),
        )
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_invalid).await?;
    println!("  Table 3: ct_state=+trk+inv -> DROP");

    // Established connections -> continue to firewall
    let established_to_fw = Flow::add()
        .table(3)
        .priority(100)
        .match_fields(
            Match::new()
                .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
        )
        .actions(ActionList::new().resubmit_table(4));

    conn.send_flow_sync(&established_to_fw).await?;
    println!("  Table 3: ct_state=+trk+est -> resubmit(4)");

    // Related connections -> continue to firewall
    let related_to_fw = Flow::add()
        .table(3)
        .priority(100)
        .match_fields(
            Match::new()
                .ct_state_masked(ct_state::TRK | ct_state::REL, ct_state::TRK | ct_state::REL),
        )
        .actions(ActionList::new().resubmit_table(4));

    conn.send_flow_sync(&related_to_fw).await?;
    println!("  Table 3: ct_state=+trk+rel -> resubmit(4)");

    // New connections -> continue to firewall
    let new_to_fw = Flow::add()
        .table(3)
        .priority(90)
        .match_fields(
            Match::new()
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().resubmit_table(4));

    conn.send_flow_sync(&new_to_fw).await?;
    println!("  Table 3: ct_state=+trk+new -> resubmit(4)");

    // Default drop
    let drop_default_t3 = Flow::add()
        .table(3)
        .priority(0)
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_default_t3).await?;
    println!("  Table 3: default -> DROP");

    // ==========================================================================
    // Tables 4-5: IPv6 Firewall Policies
    // ==========================================================================
    // Demonstrates different policies for IPv4 vs IPv6 (dual-stack)

    println!("\n--- Tables 4-5: Firewall Policies ---");

    // Table 4: Firewall policy decisions

    // Allow established IPv6 bidirectionally
    let allow_est_ipv6_out = Flow::add()
        .table(4)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x86dd)
                .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
        )
        .actions(ActionList::new().resubmit_table(6)); // -> NAT

    conn.send_flow_sync(&allow_est_ipv6_out).await?;
    println!("  Table 4: in={INTERNAL_PORT}, IPv6, established -> resubmit(6)");

    let allow_est_ipv6_in = Flow::add()
        .table(4)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x86dd)
                .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
        )
        .actions(ActionList::new().resubmit_table(6));

    conn.send_flow_sync(&allow_est_ipv6_in).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, IPv6, established -> resubmit(6)");

    // Allow established IPv4 bidirectionally
    let allow_est_ipv4_out = Flow::add()
        .table(4)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x0800)
                .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
        )
        .actions(ActionList::new().output(EXTERNAL_PORT)); // IPv4: direct output

    conn.send_flow_sync(&allow_est_ipv4_out).await?;
    println!("  Table 4: in={INTERNAL_PORT}, IPv4, established -> output:{EXTERNAL_PORT}");

    let allow_est_ipv4_in = Flow::add()
        .table(4)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x0800)
                .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
        )
        .actions(ActionList::new().output(INTERNAL_PORT));

    conn.send_flow_sync(&allow_est_ipv4_in).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, IPv4, established -> output:{INTERNAL_PORT}");

    // Allow new outbound IPv6 -> NAT table
    let allow_new_ipv6_out = Flow::add()
        .table(4)
        .priority(90)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x86dd)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().resubmit_table(6));

    conn.send_flow_sync(&allow_new_ipv6_out).await?;
    println!("  Table 4: in={INTERNAL_PORT}, IPv6, new -> resubmit(6) (NAT)");

    // Allow new outbound IPv4 (commit without NAT66)
    let allow_new_ipv4_out = Flow::add()
        .table(4)
        .priority(90)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x0800)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(5)));

    conn.send_flow_sync(&allow_new_ipv4_out).await?;
    println!("  Table 4: in={INTERNAL_PORT}, IPv4, new -> ct(commit, table=5)");

    // IPv6-specific: Allow inbound ICMPv6 echo (ping6)
    let allow_ping6_in = Flow::add()
        .table(4)
        .priority(80)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x86dd)
                .ip_proto(58) // ICMPv6
                .icmpv6_type(128) // Echo Request
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().resubmit_table(6));

    conn.send_flow_sync(&allow_ping6_in).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, ICMPv6 echo, new -> resubmit(6)");

    // Block new inbound IPv6 (except allowed services)
    let block_new_ipv6_in = Flow::add()
        .table(4)
        .priority(50)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x86dd)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&block_new_ipv6_in).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, IPv6, new -> DROP");

    // Block new inbound IPv4
    let block_new_ipv4_in = Flow::add()
        .table(4)
        .priority(50)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x0800)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&block_new_ipv4_in).await?;
    println!("  Table 4: in={EXTERNAL_PORT}, IPv4, new -> DROP");

    // Default drop
    let drop_default_t4 = Flow::add()
        .table(4)
        .priority(0)
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_default_t4).await?;
    println!("  Table 4: default -> DROP");

    // Table 5: Output after IPv4 CT commit
    let output_ipv4_out = Flow::add()
        .table(5)
        .priority(100)
        .match_fields(Match::new().in_port(INTERNAL_PORT))
        .actions(ActionList::new().output(EXTERNAL_PORT));

    conn.send_flow_sync(&output_ipv4_out).await?;
    println!("  Table 5: in={INTERNAL_PORT} -> output:{EXTERNAL_PORT}");

    let output_ipv4_in = Flow::add()
        .table(5)
        .priority(100)
        .match_fields(Match::new().in_port(EXTERNAL_PORT))
        .actions(ActionList::new().output(INTERNAL_PORT));

    conn.send_flow_sync(&output_ipv4_in).await?;
    println!("  Table 5: in={EXTERNAL_PORT} -> output:{INTERNAL_PORT}");

    // ==========================================================================
    // Tables 6-7: NAT66 (IPv6-to-IPv6 NAT)
    // ==========================================================================
    // Translates internal ULA (fd00::/64) to external global (2001:db8::1)

    println!("\n--- Tables 6-7: NAT66 ---");

    // Table 6: Apply NAT66 for new outbound IPv6
    let nat66_config = NatConfig::snat_v6(external_ipv6);

    let nat66_new_out = Flow::add()
        .table(6)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x86dd)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct_nat(CT_COMMIT, CT_ZONE, Some(7), nat66_config.clone()));

    conn.send_flow_sync(&nat66_new_out).await?;
    println!("  Table 6: in={INTERNAL_PORT}, IPv6, new -> ct(commit, nat=snat:{external_ipv6}, table=7)");

    // Established/related IPv6 from internal -> forward (NAT already applied)
    let nat66_est_out = Flow::add()
        .table(6)
        .priority(90)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x86dd)
                .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
        )
        .actions(ActionList::new().output(EXTERNAL_PORT));

    conn.send_flow_sync(&nat66_est_out).await?;
    println!("  Table 6: in={INTERNAL_PORT}, IPv6, established -> output:{EXTERNAL_PORT}");

    // Established/related IPv6 from external -> forward (reverse NAT automatic)
    let nat66_est_in = Flow::add()
        .table(6)
        .priority(90)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x86dd)
                .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
        )
        .actions(ActionList::new().output(INTERNAL_PORT));

    conn.send_flow_sync(&nat66_est_in).await?;
    println!("  Table 6: in={EXTERNAL_PORT}, IPv6, established -> output:{INTERNAL_PORT}");

    // New inbound allowed traffic (e.g., ICMPv6 echo) -> commit
    let nat66_new_in = Flow::add()
        .table(6)
        .priority(80)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x86dd)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(7)));

    conn.send_flow_sync(&nat66_new_in).await?;
    println!("  Table 6: in={EXTERNAL_PORT}, IPv6, new -> ct(commit, table=7)");

    // Default drop
    let drop_default_t6 = Flow::add()
        .table(6)
        .priority(0)
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_default_t6).await?;
    println!("  Table 6: default -> DROP");

    // Table 7: Output after NAT66
    let output_nat66_out = Flow::add()
        .table(7)
        .priority(100)
        .match_fields(Match::new().in_port(INTERNAL_PORT))
        .actions(ActionList::new().output(EXTERNAL_PORT));

    conn.send_flow_sync(&output_nat66_out).await?;
    println!("  Table 7: in={INTERNAL_PORT} -> output:{EXTERNAL_PORT}");

    let output_nat66_in = Flow::add()
        .table(7)
        .priority(100)
        .match_fields(Match::new().in_port(EXTERNAL_PORT))
        .actions(ActionList::new().output(INTERNAL_PORT));

    conn.send_flow_sync(&output_nat66_in).await?;
    println!("  Table 7: in={EXTERNAL_PORT} -> output:{INTERNAL_PORT}");

    // ==========================================================================
    // Summary
    // ==========================================================================

    println!("\n--- IPv6 Features Configured ---");
    println!("\nFeatures demonstrated:");
    println!("  1. NDP Proxy: NS/RS sent to controller for proxy response");
    println!("  2. NAT66: Internal fd00::/64 -> External {external_ipv6}");
    println!("  3. IPv6 Firewall: Stateful with ICMPv6 awareness");
    println!("  4. Dual-stack: Different NAT handling for IPv4 vs IPv6");
    println!("\nPipeline:");
    println!("  Tables 0-1: ICMPv6/NDP inspection");
    println!("  Tables 2-3: Connection tracking");
    println!("  Tables 4-5: Firewall policy");
    println!("  Tables 6-7: NAT66");
    println!("\nTo verify the flows:");
    println!("  podman exec rovs-ovsdb-test ovs-ofctl dump-flows br-test -O OpenFlow13");

    if args.no_cleanup {
        println!("\nFlows left for inspection. Clean up with:");
        println!("  podman exec rovs-ovsdb-test ovs-ofctl del-flows br-test -O OpenFlow13");
    } else {
        println!("\nCleaning up flows...");
        for table in 0..=7 {
            conn.send_flow_sync(&Flow::delete().table(table)).await?;
        }
        println!("Cleanup complete.");
    }

    Ok(())
}
