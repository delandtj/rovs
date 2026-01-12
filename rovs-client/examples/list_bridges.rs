//! Example: List all OVS bridges
//!
//! Run with: cargo run --example list_bridges

use rovs_client::OvsClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to OVS
    let client =
        OvsClient::connect("unix:/var/run/openvswitch/db.sock", "tcp:127.0.0.1:6653").await?;

    // List bridges
    let bridges = client.list_bridges().await?;

    if bridges.is_empty() {
        println!("No bridges found");
    } else {
        for bridge in bridges {
            println!("Bridge: {}", bridge.name);
            if let Some(dpid) = &bridge.datapath_id {
                println!("  Datapath ID: {dpid}");
            }
            println!("  Datapath type: {}", bridge.datapath_type);
            println!("  Ports: {}", bridge.ports.len());
        }
    }

    Ok(())
}
