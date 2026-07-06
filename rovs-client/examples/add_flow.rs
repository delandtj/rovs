//! Example: Add a simple flow to a bridge
//!
//! Run with: cargo run --example `add_flow`

use std::net::Ipv4Addr;

use rovs_client::{ActionList, Flow, Match, OvsClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to OVS
    let mut client =
        OvsClient::connect("unix:/var/run/openvswitch/db.sock", "tcp:127.0.0.1:6653").await?;

    // Create a flow that matches HTTP traffic (TCP port 80) and forwards to port 2
    let flow = Flow::add()
        .table(0)
        .priority(100)
        .match_fields(
            Match::new()
                .ipv4_dst(Ipv4Addr::new(10, 0, 0, 1), 32)
                .tcp_dst(80),
        )
        .actions(ActionList::new().output(2));

    // Add the flow to bridge br0
    client.add_flow("br0", flow).await?;

    println!("Flow added successfully");

    Ok(())
}
