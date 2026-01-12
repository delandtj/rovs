//! Example: Connect to OVSDB and monitor for changes
//!
//! Run with: cargo run --example ovsdb_monitor
//!
//! Requires OVS to be running. The default socket path is:
//! /var/run/openvswitch/db.sock

use rovs_ovsdb::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_env_filter("rovs=debug")
        .init();

    // Connection address - adjust as needed
    let addr = std::env::var("OVSDB_ADDR")
        .unwrap_or_else(|_| "unix:/var/run/openvswitch/db.sock".to_owned());

    println!("Connecting to OVSDB at: {addr}");

    // Connect with default config (monitors Open_vSwitch database)
    let mut client = Client::connect(&addr).await?;

    println!("Connected!");

    // Print schema info
    if let Some(schema) = client.schema() {
        println!("\nDatabase: {} v{}", schema.name, schema.version);
        println!("Tables:");
        for table_name in schema.tables.keys() {
            println!("  - {table_name}");
        }
    }

    // Print current bridges
    println!("\n--- Current Bridges ---");
    for row in client.idl().rows("Bridge") {
        let name = row.get_string("name").unwrap_or("<unknown>");
        let datapath_type = row.get_string("datapath_type").unwrap_or("");
        println!("Bridge: {name} (datapath_type: {datapath_type})");
    }

    // Print current ports
    println!("\n--- Current Ports ---");
    for row in client.idl().rows("Port") {
        let name = row.get_string("name").unwrap_or("<unknown>");
        println!("Port: {name}");
    }

    // Print current interfaces
    println!("\n--- Current Interfaces ---");
    for row in client.idl().rows("Interface") {
        let name = row.get_string("name").unwrap_or("<unknown>");
        let iface_type = row.get_string("type").unwrap_or("system");
        let ofport = row.get_i64("ofport").unwrap_or(-1);
        println!("Interface: {name} (type: {iface_type}, ofport: {ofport})");
    }

    println!("\n--- Waiting for updates (Ctrl+C to exit) ---");
    println!("Try running: ovs-vsctl add-br test-br");

    // Monitor loop
    loop {
        client.wait().await?;
        let seqno = client.idl().change_seqno();
        println!("\n[Update received, seqno: {seqno}]");

        // Re-print bridges after update
        println!("Bridges:");
        for row in client.idl().rows("Bridge") {
            let name = row.get_string("name").unwrap_or("<unknown>");
            println!("  - {name}");
        }
    }
}
