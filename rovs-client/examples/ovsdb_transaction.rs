//! OVSDB transaction example - creating bridges and ports.
//!
//! Usage:
//!   cargo run --example ovsdb_transaction

use rovs_ovsdb::{Client, Transaction};
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("rovs=info".parse()?))
        .init();

    // Default to user-space test socket
    let addr =
        std::env::var("OVSDB_ADDR").unwrap_or_else(|_| "unix:/tmp/ovs-test/db.sock".to_owned());

    println!("Connecting to OVSDB at: {}", addr);

    let mut client = Client::connect(&addr).await?;
    println!("Connected!");

    // Show current state
    println!("\n=== Current bridges ===");
    for row in client.idl().rows("Bridge") {
        println!("  - {}", row.get_string("name").unwrap_or("<unnamed>"));
    }

    // Create a test bridge with internal port
    println!("\n=== Creating bridge 'test-br0' ===");
    let mut txn = Transaction::new("Open_vSwitch");
    let (bridge_ref, _port_ref, _iface_ref) = txn.create_bridge("test-br0");
    println!("Bridge row ref: {:?}", bridge_ref);

    // Add an internal port
    let (port_ref, _iface_ref) = txn.add_internal_port("test-br0", "test-port1");
    println!("Port row ref: {:?}", port_ref);

    // Add a VLAN port
    let (vlan_port_ref, _) = txn.add_vlan_port("test-br0", "test-vlan100", 100);
    println!("VLAN port row ref: {:?}", vlan_port_ref);

    // Commit the transaction
    println!("\nCommitting transaction...");
    match client.commit(&mut txn).await {
        Ok(true) => {
            println!("Transaction succeeded!");
            println!("UUID map:");
            for (name, uuid) in txn.uuid_map() {
                println!("  {} -> {}", name, uuid);
            }
        }
        Ok(false) => {
            println!("Transaction failed!");
            return Err("Transaction failed".into());
        }
        Err(e) => {
            println!("Transaction error: {}", e);
            return Err(e.into());
        }
    }

    // Wait for update notification
    println!("\nWaiting for update...");
    client.wait().await?;

    println!("\n=== Bridges after transaction ===");
    for row in client.idl().rows("Bridge") {
        let name = row.get_string("name").unwrap_or("<unnamed>");
        println!("  Bridge: {}", name);
    }

    println!("\n=== Ports after transaction ===");
    for row in client.idl().rows("Port") {
        let name = row.get_string("name").unwrap_or("<unnamed>");
        let tag = row.get("tag");
        println!("  Port: {} (tag: {:?})", name, tag);
    }

    // Create a second bridge and patch ports
    println!("\n=== Creating bridge 'test-br1' and patch ports ===");
    let mut txn2 = Transaction::new("Open_vSwitch");
    txn2.create_bridge("test-br1");
    let (patch1_ref, _, patch2_ref, _) = txn2.add_patch_ports("test-br0", "test-br1", None, None);
    println!("Patch port refs: {:?}, {:?}", patch1_ref, patch2_ref);

    println!("\nCommitting patch port transaction...");
    match client.commit(&mut txn2).await {
        Ok(true) => println!("Patch ports created successfully!"),
        Ok(false) => println!("Failed to create patch ports"),
        Err(e) => println!("Error: {}", e),
    }

    // Wait for update
    client.wait().await?;

    println!("\n=== Interfaces after patch ports ===");
    for row in client.idl().rows("Interface") {
        let name = row.get_string("name").unwrap_or("<unnamed>");
        let iface_type = row.get_string("type").unwrap_or("");
        let options = row.get("options");
        if iface_type == "patch" {
            println!(
                "  Interface: {} (type: {}, options: {:?})",
                name, iface_type, options
            );
        }
    }

    // Verify via ovs-vsctl
    println!("\n=== Verification via ovs-vsctl ===");
    let output = std::process::Command::new("ovs-vsctl")
        .args(["--db=unix:/tmp/ovs-test/db.sock", "show"])
        .output()?;
    println!("{}", String::from_utf8_lossy(&output.stdout));

    println!("\nDone!");
    Ok(())
}
