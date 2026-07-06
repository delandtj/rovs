//! Example: Cloud-Hypervisor VM Integration
//!
//! Demonstrates integrating `cloud-hypervisor` VMs with Open vSwitch:
//!
//! 1. Create a VM bridge with `OpenFlow` support
//! 2. Attach tap interfaces created by `cloud-hypervisor`
//! 3. Configure VLAN isolation between VMs
//! 4. Set up NAT gateway for VM internet access
//!
//! # Cloud-Hypervisor Setup
//!
//! Cloud-hypervisor creates tap interfaces when launching VMs. The typical workflow:
//!
//! ```sh
//! # 1. Create the tap interface before starting the VM
//! sudo ip tuntap add dev vmtap0 mode tap
//! sudo ip link set vmtap0 up
//!
//! # 2. Start cloud-hypervisor with the tap device
//! cloud-hypervisor \
//!     --kernel /path/to/vmlinux \
//!     --disk path=/path/to/disk.img \
//!     --net tap=vmtap0,mac=02:00:00:00:00:01
//!
//! # 3. This example attaches vmtap0 to the OVS bridge
//! ```
//!
//! # Run with:
//! ```sh
//! # Start OVS container:
//! ./scripts/test-with-ovs.sh start
//!
//! # Run the example:
//! OVSDB_ADDR=tcp:127.0.0.1:6640 cargo run -p rovs-ext --example cloud_hypervisor_vm
//! ```

// Examples are intentionally verbose for educational purposes
#![allow(clippy::too_many_lines)]

use std::collections::HashMap;

use clap::{Parser, Subcommand};
use rovs_ovsdb::{Client, Transaction};

#[derive(Parser)]
#[command(name = "cloud_hypervisor_vm")]
#[command(about = "Cloud-Hypervisor VM integration with OVS")]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create the VM bridge infrastructure
    Setup {
        /// Bridge name for VMs
        #[arg(long, default_value = "br-vm")]
        bridge: String,

        /// Enable OpenFlow-only mode (no normal switching)
        #[arg(long)]
        openflow: bool,

        /// External interface for NAT gateway (e.g., eth0)
        #[arg(long)]
        external: Option<String>,
    },

    /// Attach a tap interface to the VM bridge
    Attach {
        /// Bridge name
        #[arg(long, default_value = "br-vm")]
        bridge: String,

        /// Tap interface name (e.g., vmtap0)
        #[arg(long)]
        tap: String,

        /// VLAN tag for the VM (optional)
        #[arg(long)]
        vlan: Option<u16>,

        /// VM name for `external_ids` (optional)
        #[arg(long)]
        vm_name: Option<String>,
    },

    /// Detach a tap interface from the VM bridge
    Detach {
        /// Tap interface name to remove
        #[arg(long)]
        tap: String,
    },

    /// List all VMs attached to the bridge
    List {
        /// Bridge name
        #[arg(long, default_value = "br-vm")]
        bridge: String,
    },

    /// Clean up the VM bridge
    Cleanup {
        /// Bridge name
        #[arg(long, default_value = "br-vm")]
        bridge: String,
    },

    /// Run the full demo (creates bridge, simulates VM attachments)
    Demo,
}

fn get_ovsdb_addr() -> String {
    std::env::var("OVSDB_ADDR").unwrap_or_else(|_| "tcp:127.0.0.1:6640".to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = get_ovsdb_addr();
    println!("Connecting to OVSDB at {addr}...");

    let mut client = Client::connect(&addr).await?;
    println!("Connected to OVSDB!\n");

    match args.command {
        Some(Commands::Setup {
            bridge,
            openflow,
            external,
        }) => {
            setup_vm_bridge(&mut client, &bridge, openflow, external.as_deref()).await?;
        }
        Some(Commands::Attach {
            bridge,
            tap,
            vlan,
            vm_name,
        }) => {
            attach_tap(&mut client, &bridge, &tap, vlan, vm_name.as_deref()).await?;
        }
        Some(Commands::Detach { tap }) => {
            detach_tap(&mut client, &tap).await?;
        }
        Some(Commands::List { bridge }) => {
            list_vms(&mut client, &bridge)?;
        }
        Some(Commands::Cleanup { bridge }) => {
            cleanup_bridge(&mut client, &bridge).await?;
        }
        Some(Commands::Demo) | None => {
            run_demo(&mut client).await?;
        }
    }

    Ok(())
}

/// Create the VM bridge infrastructure
async fn setup_vm_bridge(
    client: &mut Client,
    bridge_name: &str,
    openflow_mode: bool,
    external_iface: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Setting up VM Bridge ===");
    println!("Bridge: {bridge_name}");
    println!("OpenFlow mode: {openflow_mode}");
    if let Some(ext) = external_iface {
        println!("External interface: {ext}");
    }

    let mut txn = Transaction::new("Open_vSwitch");

    // Create the bridge
    let (_bridge_ref, _port_ref, _iface_ref) = txn.create_bridge(bridge_name);

    client.commit(&mut txn).await?;

    // Set fail_mode if OpenFlow (need to lookup the bridge UUID)
    if openflow_mode {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.update_by_name(
            "Bridge",
            bridge_name,
            serde_json::json!({
                "fail_mode": "secure"
            }),
        );
        client.commit(&mut txn).await?;
        println!("  Set fail_mode=secure for OpenFlow-only operation");
    }
    println!("  Created bridge: {bridge_name}");

    // Add external interface if specified
    if let Some(ext_iface) = external_iface {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.add_system_port(bridge_name, ext_iface);
        client.commit(&mut txn).await?;
        println!("  Added external interface: {ext_iface}");
    }

    println!("\nBridge ready for VM tap interfaces.");
    println!("Use: cargo run -p rovs-ext --example cloud_hypervisor_vm -- attach --tap vmtap0");

    Ok(())
}

/// Attach a tap interface to the VM bridge
async fn attach_tap(
    client: &mut Client,
    bridge_name: &str,
    tap_name: &str,
    vlan: Option<u16>,
    vm_name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Attaching Tap Interface ===");
    println!("Bridge: {bridge_name}");
    println!("Tap: {tap_name}");
    if let Some(v) = vlan {
        println!("VLAN: {v}");
    }
    if let Some(name) = vm_name {
        println!("VM Name: {name}");
    }

    let mut txn = Transaction::new("Open_vSwitch");

    // Build external_ids for the port
    let mut external_ids: HashMap<String, String> = HashMap::new();
    if let Some(name) = vm_name {
        external_ids.insert("vm-name".to_string(), name.to_string());
    }
    external_ids.insert("attached-by".to_string(), "rovs".to_string());

    // Create port and interface
    let iface_row = serde_json::json!({
        "name": tap_name,
        "type": "system",
        "external_ids": ["map", external_ids.iter()
            .map(|(k, v)| [k.clone(), v.clone()])
            .collect::<Vec<_>>()
        ]
    });

    let iface_ref = txn.insert("Interface", iface_row);

    let mut port_row = serde_json::json!({
        "name": tap_name,
        "interfaces": iface_ref.to_json(),
        "external_ids": ["map", external_ids.iter()
            .map(|(k, v)| [k.clone(), v.clone()])
            .collect::<Vec<_>>()
        ]
    });

    // Add VLAN tag if specified
    if let Some(vlan_id) = vlan {
        port_row["tag"] = serde_json::json!(vlan_id);
    }

    let port_ref = txn.insert("Port", port_row);

    // Add port to bridge
    txn.mutate_by_name(
        "Bridge",
        bridge_name,
        vec![serde_json::json!([
            "ports",
            "insert",
            ["set", [port_ref.to_json()]]
        ])],
    );

    client.commit(&mut txn).await?;

    println!("\nTap interface attached successfully!");
    println!("The tap device should now be visible in the bridge:");
    println!("  ovs-vsctl show");

    Ok(())
}

/// Detach a tap interface from any bridge
async fn detach_tap(client: &mut Client, tap_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Detaching Tap Interface ===");
    println!("Tap: {tap_name}");

    let mut txn = Transaction::new("Open_vSwitch");
    txn.delete_port("", tap_name);
    client.commit(&mut txn).await?;

    println!("Tap interface detached.");
    Ok(())
}

/// List all VMs attached to the bridge
#[allow(clippy::unnecessary_wraps)]
fn list_vms(client: &mut Client, bridge_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== VMs on Bridge: {bridge_name} ===\n");

    // Find the bridge and its ports
    let bridge_row = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(bridge_name));

    let Some(bridge) = bridge_row else {
        println!("Bridge '{bridge_name}' not found.");
        return Ok(());
    };

    // Get port UUIDs from the bridge
    let ports_value = bridge.get("ports");

    // List ports with their details
    println!(
        "{:<15} {:<10} {:<20} {:<15}",
        "PORT", "VLAN", "VM-NAME", "TYPE"
    );
    println!("{}", "-".repeat(60));

    // Collect port UUIDs from the bridge's ports field
    let mut port_uuids: Vec<String> = Vec::new();
    if let Some(ports) = ports_value {
        // Handle OVSDB set format: ["set", [["uuid", "xxx"], ...]] or ["uuid", "xxx"]
        if let Some(arr) = ports.as_array() {
            if arr.len() == 2 && arr[0].as_str() == Some("uuid") {
                // Single UUID
                if let Some(uuid) = arr[1].as_str() {
                    port_uuids.push(uuid.to_string());
                }
            } else if arr.len() == 2 && arr[0].as_str() == Some("set") {
                // Set of UUIDs
                if let Some(uuids) = arr[1].as_array() {
                    for uuid_entry in uuids {
                        if let Some(uuid_arr) = uuid_entry.as_array() {
                            if uuid_arr.len() == 2 && uuid_arr[0].as_str() == Some("uuid") {
                                if let Some(uuid) = uuid_arr[1].as_str() {
                                    port_uuids.push(uuid.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for port in client.idl().rows("Port") {
        let port_uuid_str = port.uuid.to_string();
        if !port_uuids.contains(&port_uuid_str) {
            continue;
        }

        let port_name = port.get_string("name").unwrap_or("?");

        // Skip the bridge's internal port
        if port_name == bridge_name {
            continue;
        }

        let vlan = port
            .get_i64("tag")
            .map_or_else(|| "-".to_string(), |v| v.to_string());

        // Parse external_ids map to find vm-name
        let vm_name = port
            .get("external_ids")
            .and_then(|v| {
                // Format: ["map", [["key1", "val1"], ["key2", "val2"]]]
                if let Some(arr) = v.as_array() {
                    if arr.len() == 2 && arr[0].as_str() == Some("map") {
                        if let Some(map_entries) = arr[1].as_array() {
                            for entry in map_entries {
                                if let Some(kv) = entry.as_array() {
                                    if kv.len() == 2 && kv[0].as_str() == Some("vm-name") {
                                        return kv[1].as_str();
                                    }
                                }
                            }
                        }
                    }
                }
                None
            })
            .unwrap_or("-");

        // Get interface type
        let iface_type = client
            .idl()
            .rows("Interface")
            .find(|i| i.get_string("name") == Some(port_name))
            .and_then(|i| i.get_string("type"))
            .unwrap_or("system");

        println!("{port_name:<15} {vlan:<10} {vm_name:<20} {iface_type:<15}");
    }

    Ok(())
}

/// Clean up the VM bridge
async fn cleanup_bridge(
    client: &mut Client,
    bridge_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cleaning up Bridge: {bridge_name} ===");

    let mut txn = Transaction::new("Open_vSwitch");
    txn.delete_bridge(bridge_name);
    client.commit(&mut txn).await?;

    println!("Bridge and all ports deleted.");
    Ok(())
}

/// Run the full demo
async fn run_demo(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         Cloud-Hypervisor VM Integration Demo                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let bridge_name = "br-ch-demo";

    // ==========================================================================
    // Step 1: Create VM Bridge
    // ==========================================================================

    println!("--- Step 1: Create VM Bridge ---\n");

    let mut txn = Transaction::new("Open_vSwitch");
    let (_bridge_ref, _port_ref, _iface_ref) = txn.create_bridge(bridge_name);
    client.commit(&mut txn).await?;

    // Configure for OpenFlow (separate transaction to set options)
    let mut txn = Transaction::new("Open_vSwitch");
    txn.update_by_name(
        "Bridge",
        bridge_name,
        serde_json::json!({
            "fail_mode": "secure",
            "protocols": ["set", ["OpenFlow13", "OpenFlow14", "OpenFlow15"]]
        }),
    );
    client.commit(&mut txn).await?;
    println!("Created bridge: {bridge_name}");
    println!("  fail_mode: secure");
    println!("  protocols: OpenFlow13-15");

    // ==========================================================================
    // Step 2: Simulate VM Tap Attachments
    // ==========================================================================

    println!("\n--- Step 2: Attach VM Tap Interfaces ---\n");

    // VM 1: Web server in VLAN 100
    println!("Attaching vm-web (VLAN 100)...");
    attach_vm_tap(client, bridge_name, "vmtap-web", Some(100), "vm-web").await?;

    // VM 2: Database in VLAN 200
    println!("Attaching vm-db (VLAN 200)...");
    attach_vm_tap(client, bridge_name, "vmtap-db", Some(200), "vm-db").await?;

    // VM 3: App server in VLAN 100 (same as web)
    println!("Attaching vm-app (VLAN 100)...");
    attach_vm_tap(client, bridge_name, "vmtap-app", Some(100), "vm-app").await?;

    // VM 4: Management VM (no VLAN, direct access)
    println!("Attaching vm-mgmt (no VLAN)...");
    attach_vm_tap(client, bridge_name, "vmtap-mgmt", None, "vm-mgmt").await?;

    // ==========================================================================
    // Step 3: Add Uplink Port
    // ==========================================================================

    println!("\n--- Step 3: Configure Uplink ---\n");

    // Add a trunk port for external connectivity
    let mut txn = Transaction::new("Open_vSwitch");

    let iface_row = serde_json::json!({
        "name": "uplink0",
        "type": "internal"  // Would be "system" for real external interface
    });
    let iface_ref = txn.insert("Interface", iface_row);

    let port_row = serde_json::json!({
        "name": "uplink0",
        "interfaces": iface_ref.to_json(),
        "vlan_mode": "native-untagged",
        "trunks": ["set", [100, 200]],
        "external_ids": ["map", [
            ["purpose", "uplink"],
            ["attached-by", "rovs"]
        ]]
    });
    let port_ref = txn.insert("Port", port_row);

    txn.mutate_by_name(
        "Bridge",
        bridge_name,
        vec![serde_json::json!([
            "ports",
            "insert",
            ["set", [port_ref.to_json()]]
        ])],
    );

    client.commit(&mut txn).await?;
    println!("Added uplink0 (trunk: VLAN 100, 200)");

    // ==========================================================================
    // Step 4: Display Configuration
    // ==========================================================================

    println!("\n--- Step 4: Current Configuration ---\n");
    list_vms(client, bridge_name)?;

    // ==========================================================================
    // Step 5: Show Cloud-Hypervisor Commands
    // ==========================================================================

    println!("\n--- Cloud-Hypervisor Integration ---\n");
    println!("To launch VMs with these tap interfaces:\n");

    println!("# Create tap interfaces first:");
    println!("sudo ip tuntap add dev vmtap-web mode tap");
    println!("sudo ip link set vmtap-web up");
    println!();

    println!("# Start cloud-hypervisor:");
    println!("cloud-hypervisor \\");
    println!("    --kernel /path/to/vmlinux \\");
    println!("    --disk path=/path/to/web-disk.img \\");
    println!("    --net tap=vmtap-web,mac=02:00:00:00:01:01 \\");
    println!("    --cpus boot=2 --memory size=2G");
    println!();

    println!("# The tap is already attached to OVS bridge '{bridge_name}'");
    println!("# VLAN 100 isolation is handled by OVS");

    // ==========================================================================
    // Step 6: OpenFlow Configuration (Optional)
    // ==========================================================================

    println!("\n--- Step 5: OpenFlow Configuration ---\n");
    println!("For advanced networking, configure OpenFlow rules:");
    println!();
    println!("# Connect a controller:");
    println!("ovs-vsctl set-controller {bridge_name} tcp:127.0.0.1:6653");
    println!();
    println!("# Or install flows directly:");
    println!("OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example stateful_firewall");

    // ==========================================================================
    // Step 7: Cleanup
    // ==========================================================================

    println!("\n--- Step 6: Cleanup ---\n");
    println!("Cleaning up demo resources...");

    let mut txn = Transaction::new("Open_vSwitch");
    txn.delete_bridge(bridge_name);
    client.commit(&mut txn).await?;

    println!("Deleted bridge: {bridge_name}");

    // ==========================================================================
    // Summary
    // ==========================================================================

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                     Demo Complete                              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Key Operations:                                                ║");
    println!("║   1. setup    - Create VM bridge with optional OpenFlow        ║");
    println!("║   2. attach   - Add tap interface with optional VLAN           ║");
    println!("║   3. detach   - Remove tap interface                           ║");
    println!("║   4. list     - Show all VMs on bridge                         ║");
    println!("║   5. cleanup  - Delete bridge and all ports                    ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Example Usage:                                                 ║");
    println!("║   cargo run -p rovs-ext --example cloud_hypervisor_vm -- \\    ║");
    println!("║       setup --bridge br-vm --openflow                          ║");
    println!("║   cargo run -p rovs-ext --example cloud_hypervisor_vm -- \\    ║");
    println!("║       attach --tap vmtap0 --vlan 100 --vm-name my-vm           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Helper to attach a VM tap interface
async fn attach_vm_tap(
    client: &mut Client,
    bridge_name: &str,
    tap_name: &str,
    vlan: Option<u16>,
    vm_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut txn = Transaction::new("Open_vSwitch");

    let external_ids: Vec<[String; 2]> = vec![
        ["vm-name".to_string(), vm_name.to_string()],
        ["attached-by".to_string(), "rovs".to_string()],
    ];

    let iface_row = serde_json::json!({
        "name": tap_name,
        "type": "internal",  // Use "system" for real tap interfaces
        "external_ids": ["map", external_ids.clone()]
    });
    let iface_ref = txn.insert("Interface", iface_row);

    let mut port_row = serde_json::json!({
        "name": tap_name,
        "interfaces": iface_ref.to_json(),
        "external_ids": ["map", external_ids]
    });

    if let Some(vlan_id) = vlan {
        port_row["tag"] = serde_json::json!(vlan_id);
    }

    let port_ref = txn.insert("Port", port_row);

    txn.mutate_by_name(
        "Bridge",
        bridge_name,
        vec![serde_json::json!([
            "ports",
            "insert",
            ["set", [port_ref.to_json()]]
        ])],
    );

    client.commit(&mut txn).await?;
    println!("  Attached: {tap_name} (VM: {vm_name})");

    Ok(())
}
