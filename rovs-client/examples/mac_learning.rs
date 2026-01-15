//! Example: MAC Learning with NxLearn
//!
//! This example demonstrates how to implement MAC learning using the NxLearn
//! action (Nicira extension). MAC learning is a fundamental networking concept
//! where the switch learns source MAC addresses and their associated ports,
//! then uses that information to forward packets to the correct destination.
//!
//! The flow pipeline works as follows:
//!
//! Table 0 (Learning):
//!   - For every packet, learn the source MAC and ingress port
//!   - Store learned entries in Table 1
//!   - Then resubmit to Table 1 for forwarding
//!
//! Table 1 (Forwarding):
//!   - Match on destination MAC (learned from previous packets)
//!   - Output to the learned port
//!   - Default: flood to all ports (unknown destination)
//!
//! Run with: cargo run --example mac_learning

use rovs_client::{nxm, ActionList, Flow, NxLearn, OvsClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to OVS (adjust addresses as needed)
    // For testing with the container: "tcp:127.0.0.1:6640", "tcp:127.0.0.1:6653"
    let client = OvsClient::connect(
        "unix:/var/run/openvswitch/db.sock",
        "tcp:127.0.0.1:6653",
    )
    .await?;

    let bridge = "br-test";

    println!("Setting up MAC learning on bridge '{}'...\n", bridge);

    // Step 1: Clear existing flows in tables 0 and 1
    println!("Clearing existing flows...");
    client.add_flow(bridge, Flow::delete().table(0)).await?;
    client.add_flow(bridge, Flow::delete().table(1)).await?;

    // Step 2: Add the learning flow in Table 0
    //
    // This flow:
    // - Matches all packets
    // - Learns: source MAC -> ingress port mapping
    // - Creates entries in Table 1 that match on dst MAC and output to the learned port
    // - Then resubmits to Table 1 for forwarding decision
    println!("Adding learning flow in table 0...");

    let learn_action = NxLearn::new()
        .table(1)                    // Install learned flows in table 1
        .priority(100)               // Priority of learned flows
        .idle_timeout(300)           // Learned entries expire after 5 min idle
        .hard_timeout(0)             // No hard timeout
        // Match spec: match dst MAC in table 1 = src MAC from this packet
        // This means: when we see a packet with eth_src=X, create a flow
        // in table 1 that matches eth_dst=X
        .match_field(
            nxm::ETH_SRC,            // Source field: packet's eth_src
            nxm::ETH_DST,            // Destination field: match on eth_dst
            48,                       // 48 bits (full MAC address)
        )
        // Load spec: load the ingress port into the output action
        // This copies NXM_OF_IN_PORT to the output action's port field
        .load_field(
            nxm::IN_PORT,            // Source: packet's in_port
            nxm::REG0,               // Destination: store in reg0 (used by output)
            16,                       // 16 bits (port number)
        );

    let learning_flow = Flow::add()
        .table(0)
        .priority(100)
        .actions(
            ActionList::new()
                .learn(learn_action)
                .resubmit_table(1),   // Continue to table 1 for forwarding
        );

    client.add_flow(bridge, learning_flow).await?;

    // Step 3: Add default flood flow in Table 1
    //
    // If the destination MAC is unknown (no learned entry), flood to all ports
    println!("Adding default flood flow in table 1...");

    let flood_flow = Flow::add()
        .table(1)
        .priority(1)                  // Low priority - learned entries have priority 100
        .actions(ActionList::new().flood());

    client.add_flow(bridge, flood_flow).await?;

    // Step 4: Add table-miss flow in Table 0 (drop unmatched)
    println!("Adding table-miss flow in table 0...");

    let table_miss = Flow::add()
        .table(0)
        .priority(0)
        .actions(ActionList::new()); // Empty actions = drop

    client.add_flow(bridge, table_miss).await?;

    println!("\nMAC learning pipeline configured successfully!");
    println!("\nFlow summary:");
    println!("  Table 0: Learn src MAC -> in_port, then resubmit to table 1");
    println!("  Table 1: Forward to learned port, or flood if unknown");
    println!("\nLearned flows will appear in table 1 as traffic flows through.");
    println!("Use 'ovs-ofctl dump-flows {}' to view the flow tables.", bridge);

    Ok(())
}
