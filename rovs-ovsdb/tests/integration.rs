//! Integration tests for rovs-ovsdb.
//!
//! These tests require a running OVSDB server. Set the `OVSDB_ADDR` environment
//! variable to the server address (e.g., `unix:/tmp/ovs-test/db.sock`).
//!
//! To set up a test OVSDB server:
//! ```bash
//! mkdir -p /tmp/ovs-test
//! ovsdb-tool create /tmp/ovs-test/conf.db /usr/share/openvswitch/vswitch.ovsschema
//! ovsdb-server /tmp/ovs-test/conf.db \
//!     --remote=punix:/tmp/ovs-test/db.sock \
//!     --unixctl=/tmp/ovs-test/ovsdb.ctl \
//!     --pidfile=/tmp/ovs-test/ovsdb.pid \
//!     --detach
//! ovs-vsctl --db=unix:/tmp/ovs-test/db.sock init
//! ```
//!
//! Run these tests with: `cargo test -- --ignored`

use rovs_ovsdb::{Client, Transaction};

fn get_ovsdb_addr() -> Option<String> {
    std::env::var("OVSDB_ADDR").ok()
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn connect_and_fetch_schema() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let client = Client::connect(&addr).await.expect("Failed to connect");

    assert!(client.is_connected());

    let schema = client.schema().expect("Schema should be loaded");
    assert_eq!(schema.name, "Open_vSwitch");
    assert!(schema.tables.contains_key("Bridge"));
    assert!(schema.tables.contains_key("Port"));
    assert!(schema.tables.contains_key("Interface"));
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn create_and_delete_bridge() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    // Generate unique bridge name to avoid conflicts
    let bridge_name = format!(
        "test-br-{}",
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
    );

    // Create bridge
    let mut txn = Transaction::new("Open_vSwitch");
    let (bridge_ref, port_ref, iface_ref) = txn.create_bridge(&bridge_name);

    let success = client.commit(&mut txn).await.expect("Commit failed");
    assert!(success, "Transaction should succeed");

    // Verify UUIDs were returned
    assert!(txn.get_uuid(&extract_name(&bridge_ref)).is_some());
    assert!(txn.get_uuid(&extract_name(&port_ref)).is_some());
    assert!(txn.get_uuid(&extract_name(&iface_ref)).is_some());

    // Wait for update
    client.wait().await.expect("Wait failed");

    // Verify bridge exists in IDL
    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name));
    assert!(bridge.is_some(), "Bridge should exist in IDL");

    // Get UUIDs for deletion
    let bridge_uuid = bridge.unwrap().uuid;
    let port_uuids: Vec<_> = client
        .idl()
        .rows("Port")
        .filter(|r| r.get_string("name") == Some(&bridge_name))
        .map(|r| r.uuid)
        .collect();
    let iface_uuids: Vec<_> = client
        .idl()
        .rows("Interface")
        .filter(|r| r.get_string("name") == Some(&bridge_name))
        .map(|r| r.uuid)
        .collect();

    // Delete bridge
    let mut del_txn = Transaction::new("Open_vSwitch");
    del_txn.delete_bridge_uuid(bridge_uuid, &port_uuids, &iface_uuids);

    let del_success = client
        .commit(&mut del_txn)
        .await
        .expect("Delete commit failed");
    assert!(del_success, "Delete transaction should succeed");

    // Wait for update
    client.wait().await.expect("Wait failed");

    // Verify bridge is gone
    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name));
    assert!(bridge.is_none(), "Bridge should be deleted");
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn add_port_to_bridge() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!(
        "test-br-{}",
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
    );
    let port_name = format!(
        "test-port-{}",
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
    );

    // Create bridge
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Create bridge failed");
    client.wait().await.unwrap();

    // Add internal port
    let mut txn2 = Transaction::new("Open_vSwitch");
    txn2.add_internal_port(&bridge_name, &port_name);
    let success = client.commit(&mut txn2).await.expect("Add port failed");
    assert!(success);
    client.wait().await.unwrap();

    // Verify port exists
    let port = client
        .idl()
        .rows("Port")
        .find(|r| r.get_string("name") == Some(&port_name));
    assert!(port.is_some(), "Port should exist");

    // Cleanup
    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn add_vlan_port() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!(
        "test-br-{}",
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
    );

    // Create bridge
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    txn.add_vlan_port(&bridge_name, "vlan100", 100);
    client.commit(&mut txn).await.expect("Create bridge failed");
    client.wait().await.unwrap();

    // Verify VLAN port has correct tag
    let port = client
        .idl()
        .rows("Port")
        .find(|r| r.get_string("name") == Some("vlan100"));
    assert!(port.is_some(), "VLAN port should exist");
    assert_eq!(port.unwrap().get_i64("tag"), Some(100));

    // Cleanup
    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn create_patch_ports() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge1 = format!(
        "test-br1-{}",
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
    );
    let bridge2 = format!(
        "test-br2-{}",
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
    );

    // Create two bridges with patch ports
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge1);
    txn.create_bridge(&bridge2);
    txn.add_patch_ports(&bridge1, &bridge2, None, None);

    let success = client.commit(&mut txn).await.expect("Commit failed");
    assert!(success);
    client.wait().await.unwrap();

    // Verify patch interfaces exist
    let patch_ifaces: Vec<_> = client
        .idl()
        .rows("Interface")
        .filter(|r| r.get_string("type") == Some("patch"))
        .collect();
    assert!(
        patch_ifaces.len() >= 2,
        "Should have at least 2 patch interfaces"
    );

    // Cleanup
    cleanup_bridge(&mut client, &bridge1).await;
    cleanup_bridge(&mut client, &bridge2).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn transaction_error_on_duplicate_name() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!(
        "test-br-{}",
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
    );

    // Create bridge
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Create bridge failed");
    client.wait().await.unwrap();

    // Try to create duplicate bridge - should fail
    let mut txn2 = Transaction::new("Open_vSwitch");
    txn2.create_bridge(&bridge_name);
    let result = client.commit(&mut txn2).await;

    // Transaction should complete but report failure
    assert!(result.is_ok());
    assert!(!result.unwrap(), "Duplicate bridge should fail");

    // Cleanup
    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn list_databases() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let dbs = client.list_dbs().await.expect("list_dbs failed");
    assert!(dbs.contains(&"Open_vSwitch".to_string()));
}

// Helper function to extract the name from a RowRef
fn extract_name(row_ref: &rovs_ovsdb::RowRef) -> String {
    match row_ref {
        rovs_ovsdb::RowRef::Named(name) => name.clone(),
        rovs_ovsdb::RowRef::Uuid(uuid) => uuid.to_string(),
    }
}

// Helper function to clean up a test bridge
async fn cleanup_bridge(client: &mut Client, bridge_name: &str) {
    // First, process any pending updates to ensure IDL is current
    let _ = client.run().await;

    // Find bridge UUID
    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(bridge_name));

    if let Some(bridge) = bridge {
        let bridge_uuid = bridge.uuid;

        // Find all ports for this bridge
        let port_uuids: Vec<_> = client
            .idl()
            .rows("Port")
            .filter(|r| {
                // Check if port name matches bridge or any associated port
                r.get_string("name").is_some_and(|n| {
                    n.starts_with("test-")
                        || n == bridge_name
                        || n.starts_with("patch-")
                        || n.starts_with("vlan")
                })
            })
            .map(|r| r.uuid)
            .collect();

        let iface_uuids: Vec<_> = client
            .idl()
            .rows("Interface")
            .filter(|r| {
                r.get_string("name").is_some_and(|n| {
                    n.starts_with("test-")
                        || n == bridge_name
                        || n.starts_with("patch-")
                        || n.starts_with("vlan")
                })
            })
            .map(|r| r.uuid)
            .collect();

        let mut del_txn = Transaction::new("Open_vSwitch");
        del_txn.delete_bridge_uuid(bridge_uuid, &port_uuids, &iface_uuids);
        // Just commit, don't wait - cleanup doesn't need to verify completion
        let _ = client.commit(&mut del_txn).await;
    }
}
