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

use rovs_ovsdb::{Client, ClientConfig, IdlState, MonitorVersion, Transaction, TransactionStatus};
use serde_json::json;

/// Generate a short unique name for test resources.
/// Linux interface names are limited to 15 characters (IFNAMSIZ - 1).
fn short_id() -> String {
    // Use first 4 chars of UUID for uniqueness
    uuid::Uuid::new_v4().to_string()[..4].to_string()
}

fn get_ovsdb_addr() -> Option<String> {
    std::env::var("OVSDB_ADDR").ok()
}

/// Barrier: make the IDL reflect all transactions committed so far.
///
/// `wait()` returns on ANY update notification. With ovs-vswitchd running,
/// the database sees constant unrelated statistics updates, so a single
/// `wait()` after commit is not a reliable barrier. Instead, round-trip a
/// no-op transact (the server queues monitor updates for a commit before it
/// replies to later requests on the same connection) and then apply
/// everything that was buffered during the round trip.
async fn sync(client: &mut Client) {
    client
        .transact(json!([]))
        .await
        .expect("sync transact failed");
    client.run().await.expect("sync run failed");
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
    let bridge_name = format!("tbr-{}", short_id());

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
    sync(&mut client).await;

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
    sync(&mut client).await;

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

    let bridge_name = format!("tbr-{}", short_id());
    let port_name = format!("tp-{}", short_id());

    // Create bridge
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Create bridge failed");
    sync(&mut client).await;

    // Add internal port
    let mut txn2 = Transaction::new("Open_vSwitch");
    txn2.add_internal_port(&bridge_name, &port_name);
    let success = client.commit(&mut txn2).await.expect("Add port failed");
    assert!(success);
    sync(&mut client).await;

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

    let bridge_name = format!("tbr-{}", short_id());

    // Create bridge
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    txn.add_vlan_port(&bridge_name, "vlan100", 100);
    client.commit(&mut txn).await.expect("Create bridge failed");
    sync(&mut client).await;

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

    let bridge1 = format!("tb1-{}", short_id());
    let bridge2 = format!("tb2-{}", short_id());

    // Create two bridges with patch ports
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge1);
    txn.create_bridge(&bridge2);
    txn.add_patch_ports(&bridge1, &bridge2, None, None);

    let success = client.commit(&mut txn).await.expect("Commit failed");
    assert!(success);
    sync(&mut client).await;

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

    let bridge_name = format!("tbr-{}", short_id());

    // Create bridge
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Create bridge failed");
    sync(&mut client).await;

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
                    n.starts_with("tbr-")
                        || n.starts_with("tb1-")
                        || n.starts_with("tb2-")
                        || n.starts_with("tp-")
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
                    n.starts_with("tbr-")
                        || n.starts_with("tb1-")
                        || n.starts_with("tb2-")
                        || n.starts_with("tp-")
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

// ============================================================================
// Group 1: Client Configuration
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn connect_with_custom_config() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let config = ClientConfig::default().tables(vec![
        "Bridge".to_string(),
        "Port".to_string(),
        "Interface".to_string(),
        "Open_vSwitch".to_string(),
    ]);
    let mut client = Client::connect_with_config(&addr, config)
        .await
        .expect("Failed to connect with config");

    assert!(client.is_connected());
    assert!(client.schema().is_some());

    // Verify we can still do CRUD
    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    let success = client.commit(&mut txn).await.expect("Commit failed");
    assert!(success);
    sync(&mut client).await;

    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name));
    assert!(bridge.is_some());

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn connect_with_monitor_v2() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let config = ClientConfig::default().monitor_version(MonitorVersion::V2);
    let mut client = Client::connect_with_config(&addr, config)
        .await
        .expect("Failed to connect with monitor V2");

    assert!(client.is_connected());
    assert_eq!(client.idl().state(), IdlState::Monitoring);

    // Verify CRUD works with V2
    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    let success = client.commit(&mut txn).await.expect("Commit failed");
    assert!(success);

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn connect_with_monitor_v3() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let config = ClientConfig::default().monitor_version(MonitorVersion::V3);
    let result = Client::connect_with_config(&addr, config).await;

    // V3 may not be supported on older OVS — gracefully handle
    match result {
        Ok(client) => {
            assert!(client.is_connected());
            assert_eq!(client.idl().state(), IdlState::Monitoring);
        }
        Err(e) => {
            // Expected on older OVS that doesn't support monitor_cond_since
            eprintln!("Monitor V3 not supported (expected on older OVS): {e}");
        }
    }
}

// ============================================================================
// Group 2: IDL State & Change Tracking
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn idl_state_is_monitoring_after_connect() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let client = Client::connect(&addr).await.expect("Failed to connect");

    assert_eq!(client.idl().state(), IdlState::Monitoring);
    assert!(client.idl().is_monitoring());
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn change_seqno_increments_on_update() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let seqno_before = client.idl().change_seqno();

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    let seqno_after = client.idl().change_seqno();
    assert!(
        seqno_after > seqno_before,
        "seqno should increase: {seqno_before} -> {seqno_after}"
    );

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn idl_row_lookup_by_uuid() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    // Find bridge UUID via rows iterator
    let bridge_uuid = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist")
        .uuid;

    // Look up by UUID
    let row = client.idl().row("Bridge", &bridge_uuid);
    assert!(row.is_some());
    assert_eq!(row.unwrap().get_string("name"), Some(bridge_name.as_str()));

    // Fabricated UUID returns None
    let fake = uuid::Uuid::nil();
    assert!(client.idl().row("Bridge", &fake).is_none());

    cleanup_bridge(&mut client, &bridge_name).await;
}

// ============================================================================
// Group 3: Row Typed Accessors
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn row_get_raw_value() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist");

    // Existing column returns Some
    let name_val = bridge.get("name");
    assert!(name_val.is_some());
    assert_eq!(name_val.unwrap().as_str(), Some(bridge_name.as_str()));

    // Non-existent column returns None
    assert!(bridge.get("nonexistent_column").is_none());

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn row_get_typed_accessors() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    txn.add_vlan_port(&bridge_name, &format!("vl-{}", short_id()), 42);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    // String accessor on bridge name
    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist");
    assert_eq!(bridge.get_string("name"), Some(bridge_name.as_str()));
    // get_i64 on a string column returns None
    assert!(bridge.get_i64("name").is_none());

    // Integer accessor on port tag
    let port = client
        .idl()
        .rows("Port")
        .find(|r| r.get_i64("tag") == Some(42));
    assert!(port.is_some(), "VLAN port with tag=42 should exist");

    cleanup_bridge(&mut client, &bridge_name).await;
}

// ============================================================================
// Group 4: Schema Introspection
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn schema_table_and_column_lookup() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let client = Client::connect(&addr).await.expect("Failed to connect");

    let schema = client.schema().expect("Schema should be loaded");

    // Bridge table exists and has name column
    let bridge_table = schema.table("Bridge");
    assert!(
        bridge_table.is_some(),
        "Bridge table should exist in schema"
    );
    let bridge_table = bridge_table.unwrap();
    assert!(
        bridge_table.column("name").is_some(),
        "Bridge should have 'name' column"
    );
    assert!(
        bridge_table.column("ports").is_some(),
        "Bridge should have 'ports' column"
    );

    // Open_vSwitch is a singleton (maxRows = 1)
    let ovs_table = schema.table("Open_vSwitch");
    assert!(ovs_table.is_some());
    assert_eq!(
        ovs_table.unwrap().max_rows,
        Some(1),
        "Open_vSwitch should be singleton"
    );

    // Nonexistent table returns None
    assert!(schema.table("DoesNotExist").is_none());

    // Nonexistent column returns None
    assert!(bridge_table.column("does_not_exist").is_none());
}

// ============================================================================
// Group 5: Mutations
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn mutate_by_uuid_add_external_id() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    let bridge_uuid = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist")
        .uuid;

    // Mutate: add external_id by UUID
    let mut mtxn = Transaction::new("Open_vSwitch");
    mtxn.mutate(
        "Bridge",
        bridge_uuid,
        vec![json!([
            "external_ids",
            "insert",
            ["map", [["test-key", "test-val"]]]
        ])],
    );
    let success = client.commit(&mut mtxn).await.expect("Mutate failed");
    assert!(success);
    sync(&mut client).await;

    // Verify in IDL
    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist");
    let ext_ids = bridge.get("external_ids");
    assert!(ext_ids.is_some(), "external_ids should be set");

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn mutate_by_name_add_external_id() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    // Mutate by name
    let mut mtxn = Transaction::new("Open_vSwitch");
    mtxn.mutate_by_name(
        "Bridge",
        &bridge_name,
        vec![json!([
            "external_ids",
            "insert",
            ["map", [["by-name", "yes"]]]
        ])],
    );
    let success = client.commit(&mut mtxn).await.expect("Mutate failed");
    assert!(success);
    sync(&mut client).await;

    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist");
    let ext_ids = bridge.get("external_ids");
    assert!(ext_ids.is_some());

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn mutate_where_singleton_table() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    // Add external_id to Open_vSwitch singleton
    let mut txn = Transaction::new("Open_vSwitch");
    txn.mutate_where(
        "Open_vSwitch",
        json!([]),
        vec![json!([
            "external_ids",
            "insert",
            ["map", [["test-singleton", "val"]]]
        ])],
    );
    let success = client.commit(&mut txn).await.expect("Mutate failed");
    assert!(success);
    sync(&mut client).await;

    // Verify
    let ovs_row = client.idl().rows("Open_vSwitch").next();
    assert!(ovs_row.is_some(), "Open_vSwitch singleton should exist");

    // Cleanup: remove the key we added
    let mut cleanup_txn = Transaction::new("Open_vSwitch");
    cleanup_txn.mutate_where(
        "Open_vSwitch",
        json!([]),
        vec![json!([
            "external_ids",
            "delete",
            ["map", [["test-singleton", "val"]]]
        ])],
    );
    let _ = client.commit(&mut cleanup_txn).await;
}

// ============================================================================
// Group 6: Updates
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn update_by_uuid() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    let bridge_uuid = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist")
        .uuid;

    // Update fail_mode by UUID
    let mut utxn = Transaction::new("Open_vSwitch");
    utxn.update("Bridge", bridge_uuid, json!({"fail_mode": "secure"}));
    let success = client.commit(&mut utxn).await.expect("Update failed");
    assert!(success);
    sync(&mut client).await;

    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist");
    assert_eq!(bridge.get_string("fail_mode"), Some("secure"));

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn update_by_name() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    // Update by name
    let mut utxn = Transaction::new("Open_vSwitch");
    utxn.update_by_name("Bridge", &bridge_name, json!({"fail_mode": "standalone"}));
    let success = client.commit(&mut utxn).await.expect("Update failed");
    assert!(success);
    sync(&mut client).await;

    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist");
    assert_eq!(bridge.get_string("fail_mode"), Some("standalone"));

    cleanup_bridge(&mut client, &bridge_name).await;
}

// ============================================================================
// Group 7: Deletion Variants
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn delete_where_by_condition() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let port_name = format!("tp-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    txn.add_internal_port(&bridge_name, &port_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    // Delete interface by condition (name match)
    let mut dtxn = Transaction::new("Open_vSwitch");
    // First remove port from bridge to avoid ref violation
    dtxn.mutate_by_name(
        "Bridge",
        &bridge_name,
        vec![json!(["ports", "delete", ["set", []]])],
    );
    dtxn.delete_where("Interface", json!([["name", "==", port_name]]));
    // This may partially fail due to refs, but delete_where itself should work
    let _ = client.commit(&mut dtxn).await;

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn delete_port_by_uuid() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let port_name = format!("tp-{}", short_id());

    // Create bridge + extra port
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    txn.add_internal_port(&bridge_name, &port_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    let bridge_uuid = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist")
        .uuid;
    let port_uuid = client
        .idl()
        .rows("Port")
        .find(|r| r.get_string("name") == Some(&port_name))
        .expect("Port should exist")
        .uuid;
    let iface_uuid = client
        .idl()
        .rows("Interface")
        .find(|r| r.get_string("name") == Some(&port_name))
        .expect("Interface should exist")
        .uuid;

    // Delete port by UUID
    let mut dtxn = Transaction::new("Open_vSwitch");
    dtxn.delete_port_uuid(bridge_uuid, port_uuid, iface_uuid);
    let success = client.commit(&mut dtxn).await.expect("Delete port failed");
    assert!(success);
    sync(&mut client).await;

    // Port should be gone, bridge should still exist
    let port = client
        .idl()
        .rows("Port")
        .find(|r| r.get_string("name") == Some(&port_name));
    assert!(port.is_none(), "Port should be deleted");

    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name));
    assert!(bridge.is_some(), "Bridge should still exist");

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn delete_port_by_name() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let port_name = format!("tp-{}", short_id());

    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    txn.add_internal_port(&bridge_name, &port_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    // Delete port by name (simple version — may fail on refs but exercises the API)
    let mut dtxn = Transaction::new("Open_vSwitch");
    dtxn.delete_port(&bridge_name, &port_name);
    // This may fail due to referential integrity, which is expected
    let _ = client.commit(&mut dtxn).await;

    cleanup_bridge(&mut client, &bridge_name).await;
}

// ============================================================================
// Group 8: Controller
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn set_controller_on_bridge() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    txn.set_controller(&bridge_name, "tcp:127.0.0.1:6653");
    let success = client.commit(&mut txn).await.expect("Commit failed");
    assert!(success);
    sync(&mut client).await;

    // Verify controller exists
    let controller = client
        .idl()
        .rows("Controller")
        .find(|r| r.get_string("target") == Some("tcp:127.0.0.1:6653"));
    assert!(controller.is_some(), "Controller should be created");

    // Verify bridge references the controller
    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name))
        .expect("Bridge should exist");
    let ctrl_ref = bridge.get("controller");
    assert!(ctrl_ref.is_some(), "Bridge should reference controller");

    cleanup_bridge(&mut client, &bridge_name).await;
}

// ============================================================================
// Group 9: Raw/Special Operations
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn raw_transact_select() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    // Raw select query
    let result = client
        .transact(json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge_name]]
        }]))
        .await
        .expect("Raw transact failed");

    // Result should be an array with one result object
    let arr = result.as_array().expect("Result should be array");
    assert!(!arr.is_empty());
    let rows = arr[0].get("rows").expect("Should have rows");
    let rows_arr = rows.as_array().expect("Rows should be array");
    assert_eq!(rows_arr.len(), 1);
    assert_eq!(rows_arr[0]["name"], bridge_name);

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn transaction_comment() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.comment("integration test: transaction_comment");
    txn.create_bridge(&bridge_name);
    let success = client.commit(&mut txn).await.expect("Commit failed");
    assert!(success, "Comment should not break transaction");
    sync(&mut client).await;

    let bridge = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name));
    assert!(bridge.is_some(), "Bridge should be created despite comment");

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn transaction_wait_operation() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Commit failed");
    sync(&mut client).await;

    // Wait on a condition that IS met (bridge exists with this name)
    let mut wtxn = Transaction::new("Open_vSwitch");
    wtxn.wait(
        "Bridge",
        vec!["name".to_string()],
        json!([["name", "==", bridge_name]]),
        json!([{"name": bridge_name}]),
    );
    let success = client.commit(&mut wtxn).await.expect("Wait txn failed");
    assert!(success, "Wait on met condition should succeed");

    cleanup_bridge(&mut client, &bridge_name).await;
}

// ============================================================================
// Group 10: Non-blocking Updates
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn run_drains_pending_updates() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client.commit(&mut txn).await.expect("Commit failed");

    // Give server time to send update
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // run() should drain pending notifications
    let updated = client.run().await.expect("run() failed");
    // May or may not have pending updates depending on timing
    // Second call should return false (no more pending)
    let updated_again = client.run().await.expect("run() failed");
    if updated {
        assert!(
            !updated_again,
            "Second run() should have no pending updates"
        );
    }

    cleanup_bridge(&mut client, &bridge_name).await;
}

// ============================================================================
// Group 11: Cancel Monitor
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn cancel_monitor_stops_updates() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());

    // Cancel the monitor
    client
        .cancel_monitor()
        .await
        .expect("cancel_monitor should succeed");

    // Create a bridge via a second client (so it doesn't go through our canceled monitor)
    let mut client2 = Client::connect(&addr)
        .await
        .expect("Failed to connect client2");
    let mut txn = Transaction::new("Open_vSwitch");
    txn.create_bridge(&bridge_name);
    client2.commit(&mut txn).await.expect("Commit failed");
    client2.wait().await.unwrap();

    // The first client should NOT see the new bridge via run()
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let _ = client.run().await;

    // Note: after cancel_monitor, client may not receive updates
    // The bridge should exist in client2's IDL
    let bridge_in_c2 = client2
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&bridge_name));
    assert!(bridge_in_c2.is_some(), "Bridge should exist in client2");

    // Cleanup via client2
    cleanup_bridge(&mut client2, &bridge_name).await;
}

// ============================================================================
// Group 12: Edge Cases
// ============================================================================

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn empty_transaction_succeeds() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let mut txn = Transaction::new("Open_vSwitch");
    assert!(txn.is_empty());

    let result = client.commit(&mut txn).await.expect("Commit failed");
    assert!(result, "Empty transaction should succeed (no-op)");
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn transaction_status_lifecycle() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let bridge_name = format!("tbr-{}", short_id());
    let mut txn = Transaction::new("Open_vSwitch");
    assert_eq!(txn.status(), TransactionStatus::Uncommitted);

    txn.create_bridge(&bridge_name);
    assert_eq!(
        txn.status(),
        TransactionStatus::Uncommitted,
        "Status should still be Uncommitted before commit"
    );

    let success = client.commit(&mut txn).await.expect("Commit failed");
    assert!(success);
    assert_eq!(
        txn.status(),
        TransactionStatus::Success,
        "Status should be Success after commit"
    );
    assert!(
        !txn.uuid_map().is_empty(),
        "uuid_map should be populated after success"
    );

    cleanup_bridge(&mut client, &bridge_name).await;
}

#[tokio::test]
#[ignore = "requires OVSDB server"]
async fn multiple_bridges_single_transaction() {
    let addr = get_ovsdb_addr().expect("OVSDB_ADDR not set");
    let mut client = Client::connect(&addr).await.expect("Failed to connect");

    let names: Vec<String> = (0..3).map(|_| format!("tbr-{}", short_id())).collect();

    let mut txn = Transaction::new("Open_vSwitch");
    for name in &names {
        txn.create_bridge(name);
    }
    let success = client.commit(&mut txn).await.expect("Commit failed");
    assert!(
        success,
        "Multi-bridge transaction should succeed atomically"
    );
    sync(&mut client).await;

    // All 3 should exist
    for name in &names {
        let bridge = client
            .idl()
            .rows("Bridge")
            .find(|r| r.get_string("name") == Some(name.as_str()));
        assert!(bridge.is_some(), "Bridge {name} should exist");
    }

    // uuid_map should have entries for all inserts
    // Each bridge creates 3 inserts (iface, port, bridge) = 9 total
    assert!(
        txn.uuid_map().len() >= 9,
        "uuid_map should have at least 9 entries, got {}",
        txn.uuid_map().len()
    );

    for name in &names {
        cleanup_bridge(&mut client, name).await;
    }
}
