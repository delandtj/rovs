//! Dual bridge with VLAN trunk example.
//!
//! This example demonstrates a realistic OVS topology:
//!
//! ```text
//!                        VLAN 100 only
//!   [veth0] ─── br-ext ═══════════════ br-int ─── [int-port]
//!                │                        │           (access vlan 100)
//!                │                        │
//!           [ext-mgmt]               [int-mgmt]
//!           (internal)               (internal)
//! ```
//!
//! - br-ext: External bridge with a veth port (simulating physical NIC)
//! - br-int: Internal bridge connected via VLAN-restricted patch ports
//! - Only VLAN 100 traffic can pass between the bridges
//!
//! Prerequisites:
//!   # Create veth pair (requires root or `CAP_NET_ADMIN`)
//!   sudo ip link add veth0 type veth peer name veth1
//!   sudo ip link set veth0 up
//!   sudo ip link set veth1 up
//!
//! Usage:
//!   # Against local OVS (requires running ovs-vswitchd)
//!   sudo `OVSDB_ADDR=unix:/var/run/openvswitch/db.sock` cargo run --example `dual_bridge_vlan`
//!
//!   # Against container (ovsdb-only mode, veth won't actually work but OVSDB accepts it)
//!   `OVSDB_ADDR=tcp:127.0.0.1:6640` cargo run --example `dual_bridge_vlan`

use rovs_ovsdb::{Client, Transaction};
use std::process::Command;
use tracing_subscriber::{EnvFilter, fmt};

const VETH_NAME: &str = "veth0";
const BR_EXT: &str = "br-ext";
const BR_INT: &str = "br-int";

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("rovs=info".parse()?))
        .init();

    let addr = std::env::var("OVSDB_ADDR")
        .unwrap_or_else(|_| "unix:/var/run/openvswitch/db.sock".to_owned());

    println!("=== Dual Bridge VLAN Example ===\n");
    println!("Connecting to OVSDB at: {addr}");

    let mut client = Client::connect(&addr).await?;
    println!(
        "Connected to OVSDB v{}",
        client.schema().map_or("?", |s| s.version.as_str())
    );

    // Check if veth exists (informational only)
    let veth_exists = std::fs::metadata(format!("/sys/class/net/{VETH_NAME}")).is_ok();
    if !veth_exists {
        println!("\nNote: {VETH_NAME} doesn't exist. Create it with:");
        println!("  sudo ip link add veth0 type veth peer name veth1");
        println!("  sudo ip link set veth0 up && sudo ip link set veth1 up");
        println!("\nContinuing anyway (OVSDB will accept the config)...\n");
    }

    // Clean up any existing test bridges
    cleanup_bridges(&mut client).await?;

    // =========================================================================
    // Step 1: Create external bridge (br-ext)
    // =========================================================================
    println!("\n=== Creating {BR_EXT} (external bridge) ===");

    let mut txn = Transaction::new("Open_vSwitch");
    txn.comment("Create external bridge with veth and management port");

    // Create bridge with default internal port
    txn.create_bridge(BR_EXT);

    // Attach veth0 (system port - represents physical/virtual NIC)
    txn.add_system_port(BR_EXT, VETH_NAME);
    println!("  - Adding system port: {VETH_NAME} (veth/physical NIC)");

    // Add management internal port
    txn.add_internal_port(BR_EXT, "ext-mgmt");
    println!("  - Adding internal port: ext-mgmt");

    commit_and_wait(&mut client, &mut txn, "br-ext creation").await?;

    // =========================================================================
    // Step 2: Create internal bridge (br-int)
    // =========================================================================
    println!("\n=== Creating {BR_INT} (internal bridge) ===");

    let mut txn = Transaction::new("Open_vSwitch");
    txn.comment("Create internal bridge with VLAN access port");

    // Create bridge
    txn.create_bridge(BR_INT);

    // Add management internal port
    txn.add_internal_port(BR_INT, "int-mgmt");
    println!("  - Adding internal port: int-mgmt");

    // Add VLAN 100 access port (traffic on this port is untagged but belongs to VLAN 100)
    txn.add_vlan_port(BR_INT, "int-vlan100", 100);
    println!("  - Adding VLAN access port: int-vlan100 (tag=100)");

    commit_and_wait(&mut client, &mut txn, "br-int creation").await?;

    // =========================================================================
    // Step 3: Connect bridges with VLAN-restricted patch ports
    // =========================================================================
    println!("\n=== Connecting bridges with VLAN 100 trunk ===");

    let mut txn = Transaction::new("Open_vSwitch");
    txn.comment("Create VLAN-restricted patch ports between bridges");

    // Create patch ports that only allow VLAN 100
    txn.add_trunk_patch_ports(BR_EXT, BR_INT, &[100], None, None);
    println!("  - Patch port: patch-{BR_EXT}-to-{BR_INT}");
    println!("  - Patch port: patch-{BR_INT}-to-{BR_EXT}");
    println!("  - Allowed VLANs: [100]");

    commit_and_wait(&mut client, &mut txn, "patch port creation").await?;

    // =========================================================================
    // Display final topology
    // =========================================================================
    println!("\n=== Final Topology ===\n");

    // Show bridges with their ports
    for bridge in client.idl().rows("Bridge") {
        let bridge_name = bridge.get_string("name").unwrap_or("<unnamed>");
        println!("Bridge: {bridge_name}");

        // Get port UUIDs that belong to this bridge
        // OVSDB sets are encoded as: single value, or ["set", [values...]]
        let bridge_port_uuids: Vec<String> = match bridge.get("ports") {
            Some(serde_json::Value::Array(arr)) if arr.len() == 2 && arr[0] == "set" => {
                // ["set", [[uuid, "..."], [uuid, "..."], ...]]
                arr[1]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| {
                                item.as_array()
                                    .and_then(|a| a.get(1))
                                    .and_then(|v| v.as_str())
                                    .map(String::from)
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
            Some(serde_json::Value::Array(arr)) if arr.len() == 2 && arr[0] == "uuid" => {
                // Single port: ["uuid", "..."]
                arr.get(1)
                    .and_then(|v| v.as_str())
                    .map(|s| vec![s.to_string()])
                    .unwrap_or_default()
            }
            _ => vec![],
        };

        for port in client.idl().rows("Port") {
            let port_name = port.get_string("name").unwrap_or("<unnamed>");
            let port_uuid_str = port.uuid.to_string();

            // Check if this port belongs to this bridge
            if !bridge_port_uuids.contains(&port_uuid_str) {
                continue;
            }

            // Skip the bridge's default port (same name as bridge)
            if port_name == bridge_name {
                continue;
            }

            let tag = port.get_i64("tag");
            let vlan_mode = port.get_string("vlan_mode");
            let trunks = port.get("trunks");

            // Find interface type
            let iface_type = client
                .idl()
                .rows("Interface")
                .find(|i| i.get_string("name") == Some(port_name))
                .and_then(|i| i.get_string("type"))
                .unwrap_or("");

            let mut details = vec![];
            if !iface_type.is_empty() && iface_type != "internal" {
                details.push(format!("type={iface_type}"));
            }
            if let Some(t) = tag {
                details.push(format!("tag={t}"));
            }
            if let Some(mode) = vlan_mode {
                details.push(format!("vlan_mode={mode}"));
            }
            if let Some(t) = trunks {
                // Format trunks nicely
                if let Some(arr) = t.as_array() {
                    if arr.len() == 2 && arr[0].as_str() == Some("set") {
                        if let Some(vlans) = arr[1].as_array() {
                            if !vlans.is_empty() {
                                let vlan_list: Vec<String> = vlans
                                    .iter()
                                    .filter_map(|v| v.as_i64().map(|n| n.to_string()))
                                    .collect();
                                details.push(format!("trunks=[{}]", vlan_list.join(",")));
                            }
                        }
                    }
                } else if let Some(n) = t.as_i64() {
                    details.push(format!("trunks=[{n}]"));
                }
            }

            println!(
                "  └─ {}: {}",
                port_name,
                if details.is_empty() {
                    "internal".to_string()
                } else {
                    details.join(", ")
                }
            );
        }
    }

    // Show ovs-vsctl output if available
    println!("\n=== ovs-vsctl show ===\n");
    let db_arg = format!("--db={addr}");

    match Command::new("ovs-vsctl").args([&db_arg, "show"]).output() {
        Ok(output) => {
            if output.status.success() {
                print!("{}", String::from_utf8_lossy(&output.stdout));
            } else {
                println!("(ovs-vsctl not available or failed)");
            }
        }
        Err(_) => println!("(ovs-vsctl not found)"),
    }

    println!("\n=== Example Complete ===");
    println!("\nTo test VLAN isolation (requires ovs-vswitchd running):");
    println!("  1. Assign IPs to the internal ports:");
    println!("     sudo ip addr add 10.100.0.1/24 dev int-vlan100");
    println!("     sudo ip link set int-vlan100 up");
    println!("  2. VLAN 100 traffic will pass through the patch ports");
    println!("  3. Other VLAN traffic will be dropped at the patch");

    println!("\nTo clean up:");
    println!("  ovs-vsctl {db_arg} del-br {BR_EXT}");
    println!("  ovs-vsctl {db_arg} del-br {BR_INT}");

    Ok(())
}

async fn commit_and_wait(
    client: &mut Client,
    txn: &mut Transaction,
    description: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match client.commit(txn).await {
        Ok(true) => {
            println!("  ✓ {description} succeeded");
            client.wait().await?;
            Ok(())
        }
        Ok(false) => Err(format!("{description} failed").into()),
        Err(e) => Err(format!("{description} error: {e}").into()),
    }
}

async fn cleanup_bridges(_client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    // Use ovs-vsctl for cleanup - it handles referential integrity correctly
    let addr = std::env::var("OVSDB_ADDR")
        .unwrap_or_else(|_| "unix:/var/run/openvswitch/db.sock".to_owned());
    let db_arg = format!("--db={addr}");

    for bridge in [BR_EXT, BR_INT] {
        let output = Command::new("ovs-vsctl")
            .args([&db_arg, "--if-exists", "del-br", bridge])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                println!("Cleaned up bridge: {bridge}");
            }
        }
    }

    // Give OVSDB time to process deletions
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(())
}
