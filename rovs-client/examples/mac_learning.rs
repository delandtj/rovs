//! Example: MAC Learning with `NxLearn`
//!
//! This example demonstrates how to implement a basic MAC learning switch
//! using the `NxLearn` action. When a packet arrives:
//!
//! 1. Table 0: Learn the source MAC -> input port mapping, then go to table 1
//! 2. Table 1: If destination MAC is known, output to learned port; else flood
//!
//! Run with:
//!   # Start OVS container first:
//!   ./scripts/test-with-ovs.sh start full
//!
//!   # Then run the example:
//!   `OPENFLOW_ADDR=tcp:127.0.0.1:6653` cargo run --example `mac_learning`

use rovs_openflow::{ActionList, Flow, NxLearn, VConn, nxm};
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

    // Clear tables 0 and 1
    println!("\nClearing tables 0 and 1...");
    conn.send_flow_sync(&Flow::delete().table(0)).await?;
    conn.send_flow_sync(&Flow::delete().table(1)).await?;

    // ==========================================================================
    // Table 0: MAC Learning
    // ==========================================================================
    // For every packet, learn the mapping: src_mac -> in_port
    // Then go to table 1 for forwarding decision
    //
    // The learn action creates entries in table 1 that match on dst_mac
    // (copied from the current packet's src_mac) and output to the
    // current packet's in_port.
    println!("\nAdding MAC learning flow to table 0...");

    // Learn action: create flow in table 1 that matches dst_mac and outputs to in_port
    let learn = NxLearn::new()
        .table(1)
        .idle_timeout(300) // Learned entries expire after 5 minutes of inactivity
        .priority(100)
        // Match on destination MAC = current packet's source MAC
        .match_field(nxm::ETH_DST, nxm::ETH_SRC, 48)
        // Output to the port where this packet came from
        .output_field(nxm::IN_PORT, 16);

    let learning_flow = Flow::add()
        .table(0)
        .priority(100)
        .actions(ActionList::new().learn(learn).goto_table(1));

    conn.send_flow_sync(&learning_flow).await?;
    println!("  Added: table=0, priority=100, actions=learn(...),goto_table:1");

    // ==========================================================================
    // Table 1: Forwarding
    // ==========================================================================
    // Default rule: flood unknown destinations
    // Learned rules (created by learn action): output to specific port
    println!("\nAdding default flood flow to table 1...");

    let flood_flow = Flow::add()
        .table(1)
        .priority(1) // Low priority, learned flows will have priority 100
        .actions(ActionList::new().flood());

    conn.send_flow_sync(&flood_flow).await?;
    println!("  Added: table=1, priority=1, actions=flood");

    // ==========================================================================
    // Summary
    // ==========================================================================
    println!("\n--- MAC Learning Switch Configured ---");
    println!("Table 0: Learn src_mac -> in_port, then goto table 1");
    println!("Table 1: Output to learned port, or flood if unknown");
    println!("\nTo verify the flows:");
    println!("  podman exec rovs-ovsdb-test ovs-ofctl dump-flows br-test -O OpenFlow13");
    println!("\nWhen traffic flows through the bridge, learned entries will appear in table 1.");

    Ok(())
}
