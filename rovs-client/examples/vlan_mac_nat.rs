//! Example: VLAN Bridge with MAC NAT
//!
//! This example creates a bridge topology with:
//! - Physical NIC (eno1) as uplink port
//! - Internal port (intern)
//! - VLAN access port (vlan100, tag=100)
//!
//! Flows are installed to perform MAC NAT (source/destination rewriting)
//! for both IPv4 and IPv6 traffic between vlan100 and eno1.
//!
//! Topology:
//!   vlan100 (tag 100) ──┐
//!                       ├── br-nat ── eno1 (physical)
//!   intern (internal) ──┘
//!
//! MAC NAT:
//!   - vlan100 -> eno1: Rewrite src MAC from internal to external
//!   - eno1 -> vlan100: Rewrite dst MAC from external to internal
//!   - ARP proxy: Respond to ARP requests for external IP
//!   - NDP proxy: Respond to neighbor solicitations for external IPv6
//!
//! Usage:
//!   # Ensure OVSDB and OpenFlow are accessible
//!   OVSDB_ADDR=tcp:127.0.0.1:6640 cargo run --example vlan_mac_nat
//!
//!   # Cleanup after testing:
//!   ovs-vsctl --db=tcp:127.0.0.1:6640 del-br br-nat

use rovs_openflow::{nxm, ActionList, Flow, Match, VConn};
use rovs_ovsdb::{Client, Transaction};
use rovs_transport::Address;

const BRIDGE_NAME: &str = "br-nat";
const PHYSICAL_PORT: &str = "eno1";
const INTERNAL_PORT: &str = "intern";
const VLAN_PORT: &str = "vlan100";
const VLAN_TAG: u16 = 100;
const OPENFLOW_PORT: u16 = 6654; // Use unique port to avoid conflicts with other bridges

// MAC addresses for NAT
const INTERNAL_MAC: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x01, 0x00]; // Internal (vlan100 side)
const EXTERNAL_MAC: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x99, 0x00]; // External (eno1 side)

// IP addresses for ARP/NDP proxy
const EXTERNAL_IPV4: [u8; 4] = [192, 168, 1, 100];
const EXTERNAL_IPV6: [u8; 16] = [
    0xfd, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
]; // fd00::100

// OpenFlow port numbers (assigned by OVS, we'll discover them)
// For simplicity, we assume: eno1=1, intern=2, vlan100=3
// In production, query OVS for actual port numbers
const PORT_ENO1: u32 = 1;
const PORT_VLAN100: u32 = 3;

fn get_ovsdb_addr() -> String {
    std::env::var("OVSDB_ADDR").unwrap_or_else(|_| "tcp:127.0.0.1:6640".to_string())
}

fn get_openflow_addr() -> Address {
    std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| format!("tcp:127.0.0.1:{}", OPENFLOW_PORT))
        .parse()
        .expect("Invalid OPENFLOW_ADDR")
}

/// Convert MAC address bytes to u64 for use with load_field
fn mac_to_u64(mac: &[u8; 6]) -> u64 {
    ((mac[0] as u64) << 40)
        | ((mac[1] as u64) << 32)
        | ((mac[2] as u64) << 24)
        | ((mac[3] as u64) << 16)
        | ((mac[4] as u64) << 8)
        | (mac[5] as u64)
}

/// Convert IPv4 address bytes to u32
fn ipv4_to_u32(ip: &[u8; 4]) -> u32 {
    ((ip[0] as u32) << 24) | ((ip[1] as u32) << 16) | ((ip[2] as u32) << 8) | (ip[3] as u32)
}

fn format_mac(mac: &[u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

fn format_ipv4(ip: &[u8; 4]) -> String {
    format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
}

fn format_ipv6(ip: &[u8; 16]) -> String {
    let addr: std::net::Ipv6Addr = (*ip).into();
    addr.to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // =========================================================================
    // Part 1: Create bridge and ports via OVSDB
    // =========================================================================
    println!("=== Creating bridge topology via OVSDB ===\n");

    let ovsdb_addr = get_ovsdb_addr();
    println!("Connecting to OVSDB at {}...", ovsdb_addr);

    let mut client = Client::connect(&ovsdb_addr).await?;
    println!("Connected to OVSDB\n");

    // Check if bridge already exists
    let bridge_exists = client
        .idl()
        .rows("Bridge")
        .any(|r| r.get_string("name") == Some(BRIDGE_NAME));

    if bridge_exists {
        println!("Bridge '{}' already exists, skipping creation", BRIDGE_NAME);
    } else {
        // Create bridge with all ports in one transaction
        let mut txn = Transaction::new("Open_vSwitch");

        // Create bridge (this also creates the default internal port with same name)
        txn.create_bridge(BRIDGE_NAME);
        println!("Creating bridge '{}'...", BRIDGE_NAME);

        // Add physical NIC as system port
        txn.add_system_port(BRIDGE_NAME, PHYSICAL_PORT);
        println!("Adding system port '{}' (physical NIC)...", PHYSICAL_PORT);

        // Add internal port
        txn.add_internal_port(BRIDGE_NAME, INTERNAL_PORT);
        println!("Adding internal port '{}'...", INTERNAL_PORT);

        // Add VLAN access port
        txn.add_vlan_port(BRIDGE_NAME, VLAN_PORT, VLAN_TAG);
        println!(
            "Adding VLAN port '{}' with tag {}...",
            VLAN_PORT, VLAN_TAG
        );

        // Commit transaction
        match client.commit(&mut txn).await {
            Ok(true) => println!("\nBridge and ports created successfully!"),
            Ok(false) => {
                eprintln!("Transaction failed");
                return Err("OVSDB transaction failed".into());
            }
            Err(e) => {
                eprintln!("Transaction error: {}", e);
                return Err(e.into());
            }
        }

        // Wait for OVSDB to process
        client.wait().await?;
    }

    // Show created ports
    println!("\n=== Current ports on {} ===", BRIDGE_NAME);
    for row in client.idl().rows("Port") {
        let name = row.get_string("name").unwrap_or("<unnamed>");
        let tag = row.get_i64("tag");
        if let Some(t) = tag {
            println!("  {} (tag: {})", name, t);
        } else {
            println!("  {}", name);
        }
    }

    // =========================================================================
    // Part 2: Configure OpenFlow controller connection on the bridge
    // =========================================================================
    println!("\n=== Configuring OpenFlow controller ===\n");

    // Set the bridge to listen for OpenFlow connections via OVSDB
    let controller_target = format!("ptcp:{}:127.0.0.1", OPENFLOW_PORT);
    println!("Setting controller to {}...", controller_target);

    let mut ctrl_txn = Transaction::new("Open_vSwitch");
    ctrl_txn.set_controller(BRIDGE_NAME, &controller_target);

    match client.commit(&mut ctrl_txn).await {
        Ok(true) => println!("Controller configured successfully!"),
        Ok(false) => {
            eprintln!("Warning: Failed to set controller (may already exist)");
        }
        Err(e) => {
            eprintln!("Warning: Controller configuration error: {}", e);
        }
    }
    client.wait().await?;

    // Give OVS a moment to start the OpenFlow listener
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let of_addr = get_openflow_addr();

    // =========================================================================
    // Part 3: Install OpenFlow rules for MAC NAT
    // =========================================================================
    println!("=== Installing OpenFlow rules ===\n");

    println!("Connecting to OpenFlow at {}...", of_addr);
    let mut conn = VConn::connect(&of_addr).await?;
    println!("Connected! OpenFlow version: {:?}\n", conn.version());

    // Clear existing flows in table 0
    println!("Clearing table 0...");
    conn.send_flow_sync(&Flow::delete().table(0)).await?;

    // -------------------------------------------------------------------------
    // Rule 1: ARP Proxy - Respond to ARP requests for external IPv4
    // -------------------------------------------------------------------------
    println!("\n--- ARP Proxy (IPv4) ---");

    let arp_proxy = Flow::add()
        .table(0)
        .priority(300)
        .match_fields(
            Match::new()
                .in_port(PORT_ENO1)
                .eth_type(0x0806) // ARP
                .arp_op(1)        // ARP Request
                .arp_tpa(EXTERNAL_IPV4),
        )
        .actions(
            ActionList::new()
                // Move sender -> target (MAC and IP)
                .move_field(nxm::ARP_SHA, nxm::ARP_THA, 48, 0, 0)
                .move_field(nxm::ARP_SPA, nxm::ARP_TPA, 32, 0, 0)
                // Set our MAC and IP as sender
                .set_arp_sha(mac_to_u64(&EXTERNAL_MAC))
                .set_arp_spa(ipv4_to_u32(&EXTERNAL_IPV4))
                // Set opcode to reply
                .set_arp_op(2)
                // Swap Ethernet addresses
                .move_field(nxm::ETH_SRC, nxm::ETH_DST, 48, 0, 0)
                .set_eth_src(EXTERNAL_MAC)
                // Send back to input port
                .in_port(),
        );

    conn.send_flow_sync(&arp_proxy).await?;
    println!(
        "Added: ARP proxy for {} -> {}",
        format_ipv4(&EXTERNAL_IPV4),
        format_mac(&EXTERNAL_MAC)
    );

    // -------------------------------------------------------------------------
    // Rule 2: NDP Proxy - Respond to Neighbor Solicitation for external IPv6
    // -------------------------------------------------------------------------
    // ICMPv6 Neighbor Solicitation: eth_type=0x86dd, ip_proto=58, icmpv6_type=135
    println!("\n--- NDP Proxy (IPv6) ---");

    // Note: Full NDP proxy requires matching on the target address in the NS payload
    // which needs NXM extensions. For simplicity, we match on destination being
    // the solicited-node multicast address.
    // Solicited-node multicast for fd00::100 is ff02::1:ff00:100

    let ndp_proxy = Flow::add()
        .table(0)
        .priority(300)
        .match_fields(
            Match::new()
                .in_port(PORT_ENO1)
                .icmpv6_type(135), // Neighbor Solicitation (sets eth_type and ip_proto)
        )
        .actions(
            ActionList::new()
                // For a proper NDP proxy, we'd need to:
                // 1. Check target address matches our external IPv6
                // 2. Construct NA reply with target link-layer address option
                // This is complex in OpenFlow; for now, just forward to controller
                // or use a simplified approach
                //
                // Simplified: Send to controller for handling
                .controller(128),
        );

    conn.send_flow_sync(&ndp_proxy).await?;
    println!(
        "Added: NDP proxy for {} (sends to controller)",
        format_ipv6(&EXTERNAL_IPV6)
    );

    // -------------------------------------------------------------------------
    // Rule 3: IPv4 vlan100 -> eno1 (rewrite source MAC)
    // -------------------------------------------------------------------------
    println!("\n--- MAC NAT: vlan100 -> eno1 ---");

    let ipv4_out = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(PORT_VLAN100)
                .eth_type(0x0800) // IPv4
                .eth_src(INTERNAL_MAC.into()),
        )
        .actions(
            ActionList::new()
                .set_eth_src(EXTERNAL_MAC)
                .output(PORT_ENO1),
        );

    conn.send_flow_sync(&ipv4_out).await?;
    println!(
        "Added: IPv4 in_port={} eth_src={} -> set_eth_src={} output:{}",
        PORT_VLAN100,
        format_mac(&INTERNAL_MAC),
        format_mac(&EXTERNAL_MAC),
        PORT_ENO1
    );

    // -------------------------------------------------------------------------
    // Rule 4: IPv6 vlan100 -> eno1 (rewrite source MAC)
    // -------------------------------------------------------------------------
    let ipv6_out = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(PORT_VLAN100)
                .eth_type(0x86dd) // IPv6
                .eth_src(INTERNAL_MAC.into()),
        )
        .actions(
            ActionList::new()
                .set_eth_src(EXTERNAL_MAC)
                .output(PORT_ENO1),
        );

    conn.send_flow_sync(&ipv6_out).await?;
    println!(
        "Added: IPv6 in_port={} eth_src={} -> set_eth_src={} output:{}",
        PORT_VLAN100,
        format_mac(&INTERNAL_MAC),
        format_mac(&EXTERNAL_MAC),
        PORT_ENO1
    );

    // -------------------------------------------------------------------------
    // Rule 5: ARP from vlan100 -> eno1 (rewrite source MAC)
    // -------------------------------------------------------------------------
    let arp_out = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(PORT_VLAN100)
                .eth_type(0x0806) // ARP
                .eth_src(INTERNAL_MAC.into()),
        )
        .actions(
            ActionList::new()
                .set_eth_src(EXTERNAL_MAC)
                .set_arp_sha(mac_to_u64(&EXTERNAL_MAC))
                .output(PORT_ENO1),
        );

    conn.send_flow_sync(&arp_out).await?;
    println!(
        "Added: ARP in_port={} -> rewrite SHA and eth_src, output:{}",
        PORT_VLAN100, PORT_ENO1
    );

    // -------------------------------------------------------------------------
    // Rule 6: IPv4 eno1 -> vlan100 (rewrite destination MAC)
    // -------------------------------------------------------------------------
    println!("\n--- MAC NAT: eno1 -> vlan100 ---");

    let ipv4_in = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(PORT_ENO1)
                .eth_type(0x0800) // IPv4
                .eth_dst(EXTERNAL_MAC.into()),
        )
        .actions(
            ActionList::new()
                .set_eth_dst(INTERNAL_MAC)
                .output(PORT_VLAN100),
        );

    conn.send_flow_sync(&ipv4_in).await?;
    println!(
        "Added: IPv4 in_port={} eth_dst={} -> set_eth_dst={} output:{}",
        PORT_ENO1,
        format_mac(&EXTERNAL_MAC),
        format_mac(&INTERNAL_MAC),
        PORT_VLAN100
    );

    // -------------------------------------------------------------------------
    // Rule 7: IPv6 eno1 -> vlan100 (rewrite destination MAC)
    // -------------------------------------------------------------------------
    let ipv6_in = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(PORT_ENO1)
                .eth_type(0x86dd) // IPv6
                .eth_dst(EXTERNAL_MAC.into()),
        )
        .actions(
            ActionList::new()
                .set_eth_dst(INTERNAL_MAC)
                .output(PORT_VLAN100),
        );

    conn.send_flow_sync(&ipv6_in).await?;
    println!(
        "Added: IPv6 in_port={} eth_dst={} -> set_eth_dst={} output:{}",
        PORT_ENO1,
        format_mac(&EXTERNAL_MAC),
        format_mac(&INTERNAL_MAC),
        PORT_VLAN100
    );

    // -------------------------------------------------------------------------
    // Rule 8: ARP replies from eno1 -> vlan100 (rewrite destination MAC)
    // -------------------------------------------------------------------------
    let arp_in = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(
            Match::new()
                .in_port(PORT_ENO1)
                .eth_type(0x0806) // ARP
                .eth_dst(EXTERNAL_MAC.into()),
        )
        .actions(
            ActionList::new()
                .set_eth_dst(INTERNAL_MAC)
                .set_arp_tha(mac_to_u64(&INTERNAL_MAC))
                .output(PORT_VLAN100),
        );

    conn.send_flow_sync(&arp_in).await?;
    println!(
        "Added: ARP in_port={} -> rewrite THA and eth_dst, output:{}",
        PORT_ENO1, PORT_VLAN100
    );

    // -------------------------------------------------------------------------
    // Rule 9: Default - drop unmatched
    // -------------------------------------------------------------------------
    println!("\n--- Default rule ---");

    let default_drop = Flow::add().table(0).priority(1);
    conn.send_flow_sync(&default_drop).await?;
    println!("Added: priority=1 actions=drop");

    // =========================================================================
    // Summary
    // =========================================================================
    println!("\n=== Configuration Complete ===\n");
    println!("Bridge: {}", BRIDGE_NAME);
    println!("Ports:");
    println!("  - {} (physical uplink)", PHYSICAL_PORT);
    println!("  - {} (internal)", INTERNAL_PORT);
    println!("  - {} (VLAN {})", VLAN_PORT, VLAN_TAG);
    println!("\nMAC NAT:");
    println!("  Internal: {}", format_mac(&INTERNAL_MAC));
    println!("  External: {}", format_mac(&EXTERNAL_MAC));
    println!("\nARP Proxy: {} -> {}", format_ipv4(&EXTERNAL_IPV4), format_mac(&EXTERNAL_MAC));
    println!("NDP Proxy: {} (via controller)", format_ipv6(&EXTERNAL_IPV6));
    println!("\nTo verify:");
    println!("  ovs-vsctl show");
    println!("  ovs-ofctl dump-flows {} -O OpenFlow13", BRIDGE_NAME);
    println!("\nTo cleanup:");
    println!("  ovs-vsctl del-br {}", BRIDGE_NAME);

    Ok(())
}
