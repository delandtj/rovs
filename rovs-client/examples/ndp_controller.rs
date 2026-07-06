//! Example: NDP Proxy Controller
//!
//! This example implements a simple OpenFlow controller that responds to
//! ICMPv6 Neighbor Solicitation messages with Neighbor Advertisement replies,
//! providing NDP proxy functionality.
//!
//! The controller:
//! 1. Connects to an OpenFlow switch
//! 2. Installs a flow to send NDP NS packets to the controller
//! 3. Waits for Packet-In messages
//! 4. For each NS targeting our IPv6 address, constructs and sends back an NA
//!
//! Usage:
//!   # Start OVS with OpenFlow enabled on a bridge
//!   OPENFLOW_ADDR=tcp:127.0.0.1:6654 cargo run --example ndp_controller
//!
//! In another terminal, test with:
//!   ping6 -I br-nat fd00::100

use std::net::Ipv6Addr;

use rovs_openflow::ndp::{build_na_reply, parse_neighbor_solicitation};
use rovs_openflow::{ActionList, Flow, Match, PacketOut, VConn};
use rovs_transport::Address;

/// Our external IPv6 address that we proxy for.
const EXTERNAL_IPV6: Ipv6Addr = Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 0x100);
/// Our external MAC address.
const EXTERNAL_MAC: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x99, 0x00];

fn get_openflow_addr() -> Address {
    std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6654".to_string())
        .parse()
        .expect("Invalid OPENFLOW_ADDR")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = get_openflow_addr();
    println!("NDP Proxy Controller");
    println!("====================");
    println!("Proxying for IPv6: {}", EXTERNAL_IPV6);
    println!("         with MAC: {}", format_mac(&EXTERNAL_MAC));
    println!();
    println!("Connecting to OpenFlow at {}...", addr);

    let mut conn = VConn::connect(&addr).await?;
    println!("Connected! OpenFlow version: {:?}", conn.version());

    // Install flow to send ICMPv6 Neighbor Solicitation to controller
    println!("\nInstalling NDP proxy flow...");

    let ndp_to_controller = Flow::add()
        .table(0)
        .priority(300)
        .match_fields(
            Match::new().icmpv6_type(135), // Neighbor Solicitation
        )
        .actions(
            ActionList::new().controller(0xffff), // Send full packet to controller
        );

    conn.send_flow_sync(&ndp_to_controller).await?;
    println!("Flow installed: ICMPv6 NS (type 135) -> CONTROLLER");

    println!("\nWaiting for Neighbor Solicitation packets...");
    println!("(Test with: ping6 -c 1 {})\n", EXTERNAL_IPV6);

    // Main controller loop
    let mut ns_count = 0u64;
    let mut na_count = 0u64;

    loop {
        // Wait for Packet-In
        let packet_in = conn.recv_packet_in().await?;

        ns_count += 1;
        println!(
            "[{}] Received Packet-In: {} bytes from table {}, reason {:?}",
            ns_count,
            packet_in.data.len(),
            packet_in.table_id,
            packet_in.reason
        );

        // Parse the packet
        let Some((eth, ipv6, ns)) = parse_neighbor_solicitation(&packet_in.data) else {
            println!("    Not a valid Neighbor Solicitation, ignoring");
            continue;
        };

        println!(
            "    NS from {} for target {}",
            ipv6.src_addr, ns.target_addr
        );

        // Check if this NS is for our address
        if ns.target_addr != EXTERNAL_IPV6 {
            println!(
                "    Target {} is not ours ({}), ignoring",
                ns.target_addr, EXTERNAL_IPV6
            );
            continue;
        }

        // Build Neighbor Advertisement reply
        let na_packet = build_na_reply(&eth, &ipv6, &ns, EXTERNAL_MAC, EXTERNAL_IPV6);

        println!(
            "    Sending NA: {} -> {} ({} bytes)",
            EXTERNAL_IPV6,
            ipv6.src_addr,
            na_packet.len()
        );

        // Get the input port from the Packet-In match
        let in_port = packet_in.in_port();

        // Send the NA back out the same port
        let packet_out = PacketOut::new()
            .in_port(in_port)
            .actions(ActionList::new().in_port()) // Output to IN_PORT
            .data(na_packet);

        conn.send_packet_out(&packet_out).await?;
        na_count += 1;

        println!(
            "    NA sent! (total: {} NS received, {} NA sent)",
            ns_count, na_count
        );
    }
}

fn format_mac(mac: &[u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}
