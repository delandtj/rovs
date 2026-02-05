//! Example: Stateful Firewall with Connection Tracking
//!
//! Demonstrates using `OpenFlow` connection tracking (conntrack) for
//! stateful packet filtering. This example creates a simple firewall
//! that allows established connections and new outbound connections,
//! but blocks unsolicited inbound traffic.
//!
//! Flow pipeline:
//! - Table 0: Send all packets through connection tracking, recirculate to table 1
//! - Table 1: Match on `ct_state` to allow/deny traffic
//! - Table 2: Output flows after ct(commit) recirculation
//!
//! Note: When matching ct_state AND committing, you must use ct(commit, zone, Some(table))
//! which recirculates to another table for output. Using ct_commit(zone).output(port)
//! directly doesn't work with ct_state matching.
//! Run with:
//! ```sh
//! # Start OVS container with OpenFlow support:
//! ./scripts/test-with-ovs.sh start full
//!
//! # Run the example:
//! OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example stateful_firewall
//! ```

use rovs_openflow::oxm::ct_state;
use rovs_openflow::{ActionList, Flow, Match, VConn, CT_COMMIT};
use rovs_transport::Address;

fn get_openflow_addr() -> Address {
    std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6653".to_string())
        .parse()
        .expect("Invalid OPENFLOW_ADDR")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = get_openflow_addr();
    println!("Connecting to OpenFlow at {addr}...");

    let mut conn = VConn::connect(&addr).await?;
    println!("Connected! OpenFlow version: {:?}", conn.version());

    // Clear flow tables
    println!("\nClearing flow tables...");
    conn.send_flow_sync(&Flow::delete().table(0)).await?;
    conn.send_flow_sync(&Flow::delete().table(1)).await?;
    conn.send_flow_sync(&Flow::delete().table(2)).await?;

    // Define ports
    const INTERNAL_PORT: u32 = 1; // Internal network (trusted)
    const EXTERNAL_PORT: u32 = 2; // External network (untrusted)
    const CT_ZONE: u16 = 1; // Connection tracking zone

    println!("\n=== Stateful Firewall Configuration ===");
    println!("Internal port: {INTERNAL_PORT}");
    println!("External port: {EXTERNAL_PORT}");
    println!("CT zone: {CT_ZONE}");

    // ==========================================================================
    // Table 0: Connection Tracking
    // ==========================================================================
    // All packets go through connection tracking first, then recirculate to
    // table 1 for policy decisions.

    println!("\n--- Table 0: Connection Tracking ---");

    // Track all IPv4 traffic
    let ct_ipv4 = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().eth_type(0x0800)) // IPv4
        .actions(ActionList::new().ct(0, CT_ZONE, Some(1))); // ct(zone=1, table=1)

    conn.send_flow_sync(&ct_ipv4).await?;
    println!("  Added: IPv4 -> ct(zone={CT_ZONE}, table=1)");

    // Track all IPv6 traffic
    let ct_ipv6 = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().eth_type(0x86dd)) // IPv6
        .actions(ActionList::new().ct(0, CT_ZONE, Some(1)));

    conn.send_flow_sync(&ct_ipv6).await?;
    println!("  Added: IPv6 -> ct(zone={CT_ZONE}, table=1)");

    // Allow ARP without tracking (needed for L2 connectivity)
    let allow_arp = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().eth_type(0x0806)) // ARP
        .actions(ActionList::new().normal());

    conn.send_flow_sync(&allow_arp).await?;
    println!("  Added: ARP -> NORMAL");

    // Default: drop
    let drop_default_t0 = Flow::add()
        .table(0)
        .priority(0)
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_default_t0).await?;
    println!("  Added: default -> DROP");

    // ==========================================================================
    // Table 1: Firewall Policy
    // ==========================================================================
    // Now packets have ct_state set, we can make policy decisions.

    println!("\n--- Table 1: Firewall Policy ---");

    // Rule 1: Drop invalid connections
    // ct_state=+trk+inv means tracked but invalid (e.g., TCP RST for unknown conn)
    let drop_invalid = Flow::add()
        .table(1)
        .priority(100)
        .match_fields(
            Match::new()
                .ct_state_masked(ct_state::TRK | ct_state::INV, ct_state::TRK | ct_state::INV),
        )
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_invalid).await?;
    println!("  Added: ct_state=+trk+inv -> DROP (invalid connections)");

    // Rule 2: Allow established/related connections (both directions)
    // ct_state=+trk+est means tracked and established
    let allow_established = Flow::add()
        .table(1)
        .priority(90)
        .match_fields(
            Match::new()
                .ct_state_masked(ct_state::TRK | ct_state::EST, ct_state::TRK | ct_state::EST),
        )
        .actions(ActionList::new().normal());

    conn.send_flow_sync(&allow_established).await?;
    println!("  Added: ct_state=+trk+est -> NORMAL (established connections)");

    // Rule 3: Allow related connections (e.g., FTP data, ICMP errors)
    let allow_related = Flow::add()
        .table(1)
        .priority(90)
        .match_fields(
            Match::new()
                .ct_state_masked(ct_state::TRK | ct_state::REL, ct_state::TRK | ct_state::REL),
        )
        .actions(ActionList::new().normal());

    conn.send_flow_sync(&allow_related).await?;
    println!("  Added: ct_state=+trk+rel -> NORMAL (related connections)");

    // Rule 4: Allow new outbound connections (internal -> external)
    // ct_state=+trk+new means tracked and new connection
    // Note: ct(commit) requires eth_type in match - OVS validates this at flow install time
    // We need separate flows for IPv4 and IPv6

    // 4a: IPv4 outbound
    let allow_outbound_new_ipv4 = Flow::add()
        .table(1)
        .priority(80)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x0800) // IPv4 - required for ct(commit)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(2)));

    conn.send_flow_sync(&allow_outbound_new_ipv4).await?;
    println!("  Added: in_port={INTERNAL_PORT}, eth_type=IPv4, ct_state=+trk+new -> ct(commit, table=2)");

    // 4b: IPv6 outbound
    let allow_outbound_new_ipv6 = Flow::add()
        .table(1)
        .priority(80)
        .match_fields(
            Match::new()
                .in_port(INTERNAL_PORT)
                .eth_type(0x86dd) // IPv6 - required for ct(commit)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(2)));

    conn.send_flow_sync(&allow_outbound_new_ipv6).await?;
    println!("  Added: in_port={INTERNAL_PORT}, eth_type=IPv6, ct_state=+trk+new -> ct(commit, table=2)");

    // Rule 5: Allow specific inbound services (example: SSH on port 22)
    let allow_ssh_inbound = Flow::add()
        .table(1)
        .priority(70)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x0800)
                .ip_proto(6) // TCP
                .tcp_dst(22)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(2)));

    conn.send_flow_sync(&allow_ssh_inbound).await?;
    println!("  Added: in_port={EXTERNAL_PORT}, tcp_dst=22, ct_state=+trk+new -> ct(commit, zone={CT_ZONE}, table=2)");

    // Rule 6: Allow ICMP echo requests (ping) inbound
    let allow_ping_inbound = Flow::add()
        .table(1)
        .priority(70)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .eth_type(0x0800)
                .ip_proto(1) // ICMP
                .icmp_type(8) // Echo request
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, CT_ZONE, Some(2)));

    conn.send_flow_sync(&allow_ping_inbound).await?;
    println!("  Added: in_port={EXTERNAL_PORT}, icmp_type=8, ct_state=+trk+new -> ct(commit, zone={CT_ZONE}, table=2)");

    // Default: drop all other new inbound connections
    let drop_inbound_new = Flow::add()
        .table(1)
        .priority(50)
        .match_fields(
            Match::new()
                .in_port(EXTERNAL_PORT)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_inbound_new).await?;
    println!("  Added: in_port={EXTERNAL_PORT}, ct_state=+trk+new -> DROP (block unsolicited inbound)");

    // Default table 1: drop
    let drop_default_t1 = Flow::add()
        .table(1)
        .priority(0)
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_default_t1).await?;
    println!("  Added: default -> DROP");

    // ==========================================================================
    // Table 2: Output After Commit
    // ==========================================================================
    // After ct(commit, zone, table=2) recirculates here, output based on in_port.
    // The packet retains its original in_port after recirculation.

    println!("\n--- Table 2: Output After Commit ---");

    // Outbound: packets from internal port -> output to external
    let output_outbound = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(Match::new().in_port(INTERNAL_PORT))
        .actions(ActionList::new().output(EXTERNAL_PORT));

    conn.send_flow_sync(&output_outbound).await?;
    println!("  Added: in_port={INTERNAL_PORT} -> output:{EXTERNAL_PORT}");

    // Inbound: packets from external port -> output to internal
    let output_inbound = Flow::add()
        .table(2)
        .priority(100)
        .match_fields(Match::new().in_port(EXTERNAL_PORT))
        .actions(ActionList::new().output(INTERNAL_PORT));

    conn.send_flow_sync(&output_inbound).await?;
    println!("  Added: in_port={EXTERNAL_PORT} -> output:{INTERNAL_PORT}");

    // Default table 2: drop (shouldn't reach here)
    let drop_default_t2 = Flow::add()
        .table(2)
        .priority(0)
        .actions(ActionList::new().drop());

    conn.send_flow_sync(&drop_default_t2).await?;
    println!("  Added: default -> DROP");

    // ==========================================================================
    // Summary
    // ==========================================================================

    println!("\n--- Stateful Firewall Configured ---");
    println!("\nPolicy summary:");
    println!("  - Invalid connections: DROP");
    println!("  - Established/related: ALLOW");
    println!("  - New outbound (port {INTERNAL_PORT} -> {EXTERNAL_PORT}): ALLOW");
    println!("  - New inbound SSH (port 22): ALLOW");
    println!("  - New inbound ICMP echo: ALLOW");
    println!("  - Other new inbound: DROP");
    println!("\nTo verify the flows:");
    println!("  podman exec rovs-ovsdb-test ovs-ofctl dump-flows br-test -O OpenFlow13");
    println!("\nTo view connection tracking table:");
    println!("  podman exec rovs-ovsdb-test ovs-appctl dpctl/dump-conntrack");

    Ok(())
}
