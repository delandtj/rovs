//! Example: Flow Templates with rovs-ext
//!
//! Demonstrates using the high-level flow template builders:
//! - `MacNatFlows`: MAC address translation between internal/external networks
//! - `LearningSwitchFlows`: Dynamic MAC learning switch
//!
//! Run with:
//! ```sh
//! # Start OVS container first:
//! ./scripts/test-with-ovs.sh start full
//!
//! # Then run the example:
//! OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example flow_templates
//! ```

use rovs_ext::flows::{LearningConfig, LearningSwitchFlows, MacNatConfig, MacNatFlows};
use rovs_openflow::{Flow, VConn};
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

    // Clear all tables
    println!("\nClearing flow tables...");
    for table in 0..3 {
        conn.send_flow_sync(&Flow::delete().table(table)).await?;
    }

    // ==========================================================================
    // Example 1: MAC NAT Flows
    // ==========================================================================
    //
    // MAC NAT translates MAC addresses between internal and external networks.
    // Useful for scenarios where internal hosts need to appear with a different
    // MAC address externally (like a router's external MAC).
    //
    // Internal host (port 1): MAC 02:00:00:00:00:01
    // External gateway (port 2): Translates to MAC 02:00:00:00:00:99

    println!("\n=== MAC NAT Flows ===");

    let mac_nat_config = MacNatConfig::new(
        [0x02, 0x00, 0x00, 0x00, 0x00, 0x01], // internal MAC
        [0x02, 0x00, 0x00, 0x00, 0x00, 0x99], // external MAC (what external peers see)
        1, // internal port
        2, // external port
    );

    let mac_nat = MacNatFlows::new(mac_nat_config);

    // You can install all flows at once:
    println!("Installing MAC NAT flows in table 0...");
    mac_nat.install(&mut conn, 0, 100).await?;

    // Or get individual flows for more control:
    let all_flows = mac_nat.all_flows(0, 100);
    println!("  Created {} MAC NAT flows:", all_flows.len());
    println!("    - IPv4 outbound: port 1 -> 2, rewrite src MAC");
    println!("    - IPv4 inbound:  port 2 -> 1, rewrite dst MAC");
    println!("    - IPv6 outbound: port 1 -> 2, rewrite src MAC");
    println!("    - IPv6 inbound:  port 2 -> 1, rewrite dst MAC");
    println!("    - ARP outbound:  port 1 -> 2, rewrite src MAC + ARP SHA");
    println!("    - ARP inbound:   port 2 -> 1, rewrite dst MAC + ARP THA");

    // ==========================================================================
    // Example 2: Learning Switch Flows
    // ==========================================================================
    //
    // A learning switch dynamically learns MAC->port mappings from traffic.
    // When a packet arrives:
    //   1. Learn: src_mac -> in_port (stored in forward table)
    //   2. Forward: Look up dst_mac. If known, output to that port; else flood.
    //
    // Uses the NxLearn action for hardware-accelerated learning.

    println!("\n=== Learning Switch Flows ===");

    let learning_config = LearningConfig::new()
        .learn_table(1)     // Learning happens in table 1
        .forward_table(2)   // Forwarding decisions in table 2
        .idle_timeout(300); // Learned entries expire after 5 minutes

    let learning_switch = LearningSwitchFlows::new(learning_config);

    println!("Installing learning switch flows...");
    learning_switch.install(&mut conn).await?;

    let all_learning_flows = learning_switch.all_flows();
    println!("  Created {} flows:", all_learning_flows.len());
    println!("    - Table 1: Learn src_mac->in_port, goto table 2");
    println!("    - Table 2: Flood (low priority, catches unknown MACs)");
    println!("    - Table 2: Learned entries (created dynamically by NxLearn)");

    // ==========================================================================
    // Verification
    // ==========================================================================

    println!("\n--- Flow Templates Configured ---");
    println!("\nTo verify the flows:");
    println!("  podman exec rovs-ovsdb-test ovs-ofctl dump-flows br-test -O OpenFlow13");
    println!("\nExpected output:");
    println!("  Table 0: MAC NAT flows (priority 100)");
    println!("  Table 1: Learning flow (priority 100)");
    println!("  Table 2: Flood flow (priority 1)");

    Ok(())
}
