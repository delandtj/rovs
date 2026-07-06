//! Example: MAC Address Translation with ARP Proxy
//!
//! This example demonstrates how to perform MAC address translation (rewriting)
//! between two ports, including an ARP proxy that responds to ARP requests
//! for the external MAC address.
//!
//! Scenario:
//!   Port 1 (internal) <---> Port 2 (external)
//!
//!   - Traffic from port 1 to port 2: Rewrite src MAC to external MAC
//!   - Traffic from port 2 to port 1: Rewrite dst MAC to internal MAC
//!   - ARP requests for external IP: Proxy responds with external MAC
//!
//! The ARP proxy uses Nicira extensions (NxMove, NxRegLoad) to:
//! 1. Match ARP requests for the external IP
//! 2. Swap sender/target addresses to form a reply
//! 3. Set the sender MAC to the proxy's external MAC
//! 4. Send the reply back to the requester
//!
//! Run with:
//!   # Start OVS container first:
//!   ./scripts/test-with-ovs.sh start full
//!
//!   # Then run the example:
//!   OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run --example mac_translation

use rovs_openflow::{ActionList, Flow, Match, VConn, nxm};
use rovs_transport::Address;

fn get_openflow_addr() -> Address {
    std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6653".to_string())
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = get_openflow_addr();
    println!("Connecting to OpenFlow at {}...", addr);

    let mut conn = VConn::connect(&addr).await?;
    println!("Connected! OpenFlow version: {:?}", conn.version());

    // Define the MAC addresses
    let internal_mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01]; // Internal host MAC
    let external_mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x99]; // Translated external MAC

    // Define the IP addresses (for ARP proxy)
    let external_ip: [u8; 4] = [10, 0, 0, 99]; // External IP that the proxy responds to

    // Define the ports
    let internal_port = 1u32;
    let external_port = 2u32;

    // Clear table 0
    println!("\nClearing table 0...");
    conn.send_flow_sync(&Flow::delete().table(0)).await?;

    // ==========================================================================
    // Rule 1: ARP Proxy - Respond to ARP requests for external IP
    // ==========================================================================
    // When we receive an ARP request (opcode=1) for the external IP on port 2,
    // transform it into an ARP reply and send it back.
    //
    // The transformation:
    // 1. Set ARP opcode to 2 (reply)
    // 2. Move sender -> target (IP and MAC)
    // 3. Set sender MAC to external_mac
    // 4. Set sender IP to external_ip
    // 5. Set Ethernet src to external_mac, dst to original sender
    // 6. Send back to input port
    println!("\nAdding ARP proxy flow...");

    let arp_proxy = Flow::add()
        .table(0)
        .priority(200) // Higher priority than MAC translation
        .match_fields(
            Match::new()
                .in_port(external_port)
                .eth_type(0x0806) // ARP
                .arp_op(1) // ARP Request
                .arp_tpa(external_ip), // Requesting our external IP
        )
        .actions(
            ActionList::new()
                // Move sender MAC to target MAC (ARP SHA -> ARP THA)
                .move_field(nxm::ARP_SHA, nxm::ARP_THA, 48, 0, 0)
                // Move sender IP to target IP (ARP SPA -> ARP TPA)
                .move_field(nxm::ARP_SPA, nxm::ARP_TPA, 32, 0, 0)
                // Set sender MAC to our external MAC
                .set_arp_sha(mac_to_u64(&external_mac))
                // Set sender IP to our external IP
                .set_arp_spa(ipv4_to_u32(&external_ip))
                // Set ARP opcode to 2 (reply)
                .set_arp_op(2)
                // Move original Ethernet src to dst (for the reply)
                .move_field(nxm::ETH_SRC, nxm::ETH_DST, 48, 0, 0)
                // Set Ethernet src to our external MAC
                .set_eth_src(external_mac)
                // Send back to input port
                .in_port(),
        );

    conn.send_flow_sync(&arp_proxy).await?;
    println!(
        "  Added: ARP proxy for {} -> responds with {}",
        format_ipv4(&external_ip),
        format_mac(&external_mac)
    );

    // ==========================================================================
    // Rule 2: Internal -> External (rewrite source MAC)
    // ==========================================================================
    // Packets from internal port with internal MAC get their source MAC
    // rewritten to the external MAC before being forwarded to external port.
    println!("\nAdding internal->external translation flow...");

    let internal_to_external = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().in_port(internal_port).eth_src(internal_mac))
        .actions(
            ActionList::new()
                .set_eth_src(external_mac)
                .output(external_port),
        );

    conn.send_flow_sync(&internal_to_external).await?;
    println!(
        "  Added: in_port={}, eth_src={} -> set_eth_src={}, output:{}",
        internal_port,
        format_mac(&internal_mac),
        format_mac(&external_mac),
        external_port
    );

    // ==========================================================================
    // Rule 3: External -> Internal (rewrite destination MAC)
    // ==========================================================================
    // Packets from external port destined to external MAC get their destination
    // MAC rewritten to the internal MAC before being forwarded to internal port.
    println!("\nAdding external->internal translation flow...");

    let external_to_internal = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(Match::new().in_port(external_port).eth_dst(external_mac))
        .actions(
            ActionList::new()
                .set_eth_dst(internal_mac)
                .output(internal_port),
        );

    conn.send_flow_sync(&external_to_internal).await?;
    println!(
        "  Added: in_port={}, eth_dst={} -> set_eth_dst={}, output:{}",
        external_port,
        format_mac(&external_mac),
        format_mac(&internal_mac),
        internal_port
    );

    // ==========================================================================
    // Rule 4: Default drop (optional, for security)
    // ==========================================================================
    println!("\nAdding default drop flow...");

    let default_drop = Flow::add().table(0).priority(1);
    // No actions = drop

    conn.send_flow_sync(&default_drop).await?;
    println!("  Added: priority=1, actions=drop");

    // ==========================================================================
    // Summary
    // ==========================================================================
    println!("\n--- MAC Translation with ARP Proxy Configured ---");
    println!(
        "Internal host ({}) on port {} appears as {} externally",
        format_mac(&internal_mac),
        internal_port,
        format_mac(&external_mac)
    );
    println!(
        "ARP proxy responds for {} with MAC {}",
        format_ipv4(&external_ip),
        format_mac(&external_mac)
    );
    println!("\nTo verify the flows:");
    println!("  podman exec rovs-ovsdb-test ovs-ofctl dump-flows br-test -O OpenFlow13");

    Ok(())
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
