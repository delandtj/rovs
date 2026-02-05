//! Example: ARP/NDP Proxy Controller with rovs-ext
//!
//! Demonstrates the controller framework with protocol handlers:
//! - `ArpProxyHandler`: Responds to ARP requests with configured MACs
//! - `NdpProxyHandler`: Responds to IPv6 Neighbor Solicitations
//!
//! This is useful for scenarios where you need to proxy addresses that
//! don't have a real host behind them (e.g., floating IPs, anycast).
//!
//! Run with:
//! ```sh
//! # Start OVS container with OpenFlow support:
//! ./scripts/test-with-ovs.sh start full
//!
//! # Configure the bridge to send packets to the controller:
//! podman exec rovs-ovsdb-test ovs-vsctl set-controller br-test tcp:host.containers.internal:6653
//! podman exec rovs-ovsdb-test ovs-ofctl del-flows br-test -O OpenFlow13
//! podman exec rovs-ovsdb-test ovs-ofctl add-flow br-test "priority=0,actions=controller" -O OpenFlow13
//!
//! # Run the controller:
//! cargo run -p rovs-ext --example arp_ndp_controller
//! ```
//!
//! In another terminal, generate ARP requests:
//! ```sh
//! podman exec rovs-ovsdb-test arping -I br-test 10.0.0.99
//! ```

use rovs_ext::controller::protocol::{ArpProxyHandler, NdpProxyHandler};
use rovs_ext::controller::{Controller, ControllerConfig};
use rovs_transport::Address;

fn get_listen_addr() -> Address {
    std::env::var("CONTROLLER_ADDR")
        .unwrap_or_else(|_| "tcp:0.0.0.0:6653".to_string())
        .parse()
        .expect("Invalid CONTROLLER_ADDR")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rovs_ext=debug".parse()?)
                .add_directive("rovs_openflow=info".parse()?),
        )
        .init();

    let addr = get_listen_addr();
    println!("ARP/NDP Proxy Controller");
    println!("========================\n");

    // Note: In a real deployment, the switch connects to us. For this example,
    // we're connecting to a switch that has passive mode enabled, or we're
    // using ptcp on the switch side.
    //
    // For testing with the OVS container:
    //   1. Configure switch to connect to us: ovs-vsctl set-controller br-test tcp:host:6653
    //   2. Or listen passively on switch: ovs-vsctl set-controller br-test ptcp:6653

    println!("Connecting to switch at {addr}...");
    println!("(Make sure the bridge is configured to connect to this controller)\n");

    // Create controller configuration
    let config = ControllerConfig::new()
        .log_unhandled(true); // Log packets we don't handle (useful for debugging)

    // Create the controller
    let mut controller = Controller::new(&addr, config).await?;
    println!("Connected to switch!");

    // ==========================================================================
    // Configure ARP Proxy
    // ==========================================================================
    //
    // Respond to ARP requests for configured IP addresses.
    // This is useful for:
    //   - Virtual IPs (VIPs) that aren't assigned to any real host
    //   - Floating IPs in HA setups
    //   - Anycast addresses

    println!("\n--- Configuring ARP Proxy ---");

    let mut arp_handler = ArpProxyHandler::new();

    // Add IP -> MAC mappings
    arp_handler.add_entry([10, 0, 0, 99], [0x02, 0x00, 0x00, 0x00, 0x00, 0x99]);
    arp_handler.add_entry([10, 0, 0, 100], [0x02, 0x00, 0x00, 0x00, 0x00, 0xaa]);
    arp_handler.add_entry([192, 168, 1, 1], [0x02, 0x00, 0x00, 0x00, 0x01, 0x01]);

    println!("  10.0.0.99   -> 02:00:00:00:00:99");
    println!("  10.0.0.100  -> 02:00:00:00:00:aa");
    println!("  192.168.1.1 -> 02:00:00:00:01:01");

    controller.register(arp_handler);

    // ==========================================================================
    // Configure NDP Proxy
    // ==========================================================================
    //
    // Respond to IPv6 Neighbor Solicitation for configured addresses.
    // Similar use cases to ARP proxy, but for IPv6.

    println!("\n--- Configuring NDP Proxy ---");

    let mut ndp_handler = NdpProxyHandler::new();

    // Add IPv6 -> MAC mappings
    ndp_handler.add_entry("fd00::99".parse()?, [0x02, 0x00, 0x00, 0x00, 0x00, 0x99]);
    ndp_handler.add_entry("fd00::100".parse()?, [0x02, 0x00, 0x00, 0x00, 0x00, 0xaa]);
    ndp_handler.add_entry(
        "2001:db8::1".parse()?,
        [0x02, 0x00, 0x00, 0x00, 0x01, 0x01],
    );

    println!("  fd00::99    -> 02:00:00:00:00:99");
    println!("  fd00::100   -> 02:00:00:00:00:aa");
    println!("  2001:db8::1 -> 02:00:00:00:01:01");

    controller.register(ndp_handler);

    // ==========================================================================
    // Run the Controller
    // ==========================================================================

    println!("\n--- Controller Running ---");
    println!("Handlers registered: 2 (ARP proxy, NDP proxy)");
    println!("\nWaiting for packets...");
    println!("(Press Ctrl+C to stop)\n");
    println!("To test ARP:");
    println!("  podman exec rovs-ovsdb-test arping -c 1 -I br-test 10.0.0.99");
    println!("\nTo test NDP:");
    println!("  podman exec rovs-ovsdb-test ndisc6 fd00::99 br-test");
    println!();

    // Run the controller event loop
    // This will:
    //   1. Receive Packet-In events from the switch
    //   2. Dispatch to registered handlers (ARP, NDP)
    //   3. Send Packet-Out responses back to the switch
    controller.run().await?;

    Ok(())
}
