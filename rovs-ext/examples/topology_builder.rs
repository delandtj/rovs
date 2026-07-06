//! Example: Topology Builders with rovs-ext
//!
//! Demonstrates using the high-level topology builders:
//! - `BridgePair`: Create two interconnected bridges
//! - `VlanTrunk`: Create a bridge with VLAN-configured ports
//!
//! Run with:
//! ```sh
//! # Start OVS container first:
//! ./scripts/test-with-ovs.sh start
//!
//! # Then run the example:
//! OVSDB_ADDR=tcp:127.0.0.1:6640 cargo run -p rovs-ext --example topology_builder
//! ```

use rovs_ext::topology::{AccessPortConfig, BridgePair, TrunkPortConfig, VlanTrunk};
use rovs_ovsdb::Client;

fn get_ovsdb_addr() -> String {
    std::env::var("OVSDB_ADDR").unwrap_or_else(|_| "tcp:127.0.0.1:6640".to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = get_ovsdb_addr();
    println!("Connecting to OVSDB at {addr}...");

    let mut client = Client::connect(&addr).await?;
    println!("Connected to OVSDB!");

    // ==========================================================================
    // Example 1: Bridge Pair
    // ==========================================================================
    //
    // A bridge pair creates two bridges connected by patch ports.
    // This is useful for:
    //   - Separating traffic domains (internal/external)
    //   - Applying different flow policies per bridge
    //   - Creating a DMZ topology

    println!("\n=== Bridge Pair ===");

    let bridge_pair = BridgePair::new("br-rovs-int", "br-rovs-ext").secure_fail_mode(); // OpenFlow-only forwarding

    println!("Creating bridge pair: br-rovs-int <-> br-rovs-ext");
    bridge_pair.create(&mut client).await?;
    println!("  Created br-rovs-int with internal port");
    println!("  Created br-rovs-ext with internal port");
    println!("  Connected with patch ports");

    // Verify creation
    println!("\nVerifying bridges...");
    for row in client.idl().rows("Bridge") {
        let name = row.get_string("name").unwrap_or("?");
        if name.starts_with("br-rovs") {
            let fail_mode = row.get_string("fail_mode").unwrap_or("standalone");
            println!("  Bridge: {name} (fail_mode: {fail_mode})");
        }
    }

    // Clean up bridge pair
    println!("\nDeleting bridge pair...");
    bridge_pair.delete(&mut client).await?;
    println!("  Deleted br-rovs-int and br-rovs-ext");

    // ==========================================================================
    // Example 2: Bridge Pair with VLAN Trunk
    // ==========================================================================
    //
    // Bridge pairs can also have VLAN trunking enabled on the patch ports.

    println!("\n=== Bridge Pair with VLAN Trunk ===");

    let vlan_bridge_pair = BridgePair::new("br-rovs-int", "br-rovs-ext")
        .vlans(vec![100, 200, 300]) // Only pass these VLANs between bridges
        .patch_names("p-int", "p-ext"); // Custom patch port names

    println!("Creating VLAN trunk bridge pair...");
    vlan_bridge_pair.create(&mut client).await?;
    println!("  Created bridge pair with VLAN trunk (100, 200, 300)");
    println!("  Patch ports: p-int <-> p-ext");

    // Clean up
    vlan_bridge_pair.delete(&mut client).await?;
    println!("  Cleaned up");

    // ==========================================================================
    // Example 3: VLAN Trunk Bridge
    // ==========================================================================
    //
    // A VLAN trunk bridge has access ports in different VLANs and optionally
    // trunk ports that carry tagged traffic.
    //
    // Topology:
    //   vm1 (VLAN 100) ----+
    //   vm2 (VLAN 100) ----+---- br-rovs-vlan ---- uplink (trunk: 100, 200)
    //   vm3 (VLAN 200) ----+

    println!("\n=== VLAN Trunk Bridge ===");

    let vlan_trunk = VlanTrunk::new("br-rovs-vlan")
        // VMs in VLAN 100
        .access_port(AccessPortConfig::new("vm1", 100))
        .access_port(AccessPortConfig::new("vm2", 100))
        // VM in VLAN 200
        .access_port(AccessPortConfig::new("vm3", 200))
        // Uplink trunk carrying both VLANs
        .trunk_port(TrunkPortConfig::new("uplink").allowed_vlans(vec![100, 200]))
        .secure_fail_mode();

    println!("Creating VLAN trunk bridge...");
    vlan_trunk.create(&mut client).await?;
    println!("  Created br-rovs-vlan");
    println!("  Access ports: vm1(vlan 100), vm2(vlan 100), vm3(vlan 200)");
    println!("  Trunk port: uplink (vlans 100, 200)");

    // Verify the ports
    println!("\nVerifying ports...");
    for row in client.idl().rows("Port") {
        let name = row.get_string("name").unwrap_or("?");
        if ["vm1", "vm2", "vm3", "uplink", "br-rovs-vlan"].contains(&name) {
            let tag = row.get_i64("tag");
            let vlan_mode = row.get_string("vlan_mode");
            match (tag, vlan_mode) {
                (Some(t), _) => println!("  Port {name} - access (VLAN {t})"),
                (None, Some(mode)) => println!("  Port {name} - {mode} mode"),
                _ => println!("  Port {name} - default"),
            }
        }
    }

    // Clean up
    println!("\nDeleting VLAN trunk bridge...");
    vlan_trunk.delete(&mut client).await?;
    println!("  Deleted br-rovs-vlan and all ports");

    // ==========================================================================
    // Example 4: Using Existing Bridge
    // ==========================================================================

    println!("\n=== Adding VLANs to Existing Bridge ===");

    // First create a bridge manually
    let mut txn = rovs_ovsdb::Transaction::new("Open_vSwitch");
    txn.create_bridge("br-rovs-existing");
    client.commit(&mut txn).await?;
    println!("Created br-rovs-existing");

    // Now add VLAN ports to it
    let vlan_ports = VlanTrunk::new("br-rovs-existing")
        .existing_bridge() // Don't try to create the bridge
        .access_port(AccessPortConfig::new("web1", 10))
        .access_port(AccessPortConfig::new("db1", 20));

    vlan_ports.create(&mut client).await?;
    println!("  Added web1 (VLAN 10) and db1 (VLAN 20)");

    // Clean up (delete ports then bridge)
    vlan_ports.delete(&mut client).await?;
    let mut txn = rovs_ovsdb::Transaction::new("Open_vSwitch");
    txn.delete_bridge("br-rovs-existing");
    client.commit(&mut txn).await?;
    println!("  Cleaned up");

    // ==========================================================================
    // Summary
    // ==========================================================================

    println!("\n--- Topology Builder Examples Complete ---");
    println!("\nKey patterns demonstrated:");
    println!("  1. BridgePair::new(a, b).create() - Connected bridges");
    println!("  2. BridgePair::new(a, b).vlans([...]) - VLAN trunk between bridges");
    println!("  3. VlanTrunk::new(br).access_port(...) - VLAN access ports");
    println!("  4. VlanTrunk::new(br).trunk_port(...) - VLAN trunk ports");
    println!("  5. VlanTrunk::new(br).existing_bridge() - Add to existing bridge");

    Ok(())
}
