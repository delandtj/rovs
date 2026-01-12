//! OVSDB transaction handling.

use std::collections::HashMap;

use serde_json::{json, Value};
use uuid::Uuid;

/// Transaction status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    /// Not yet committed
    Uncommitted,
    /// No actual changes to commit
    Unchanged,
    /// Commit in progress
    Incomplete,
    /// Explicitly aborted
    Aborted,
    /// Successfully committed
    Success,
    /// Conflict, should retry
    TryAgain,
    /// Need lock before committing
    NotLocked,
    /// Hard failure
    Error,
}

/// Reference to a row - either an existing UUID or a named-uuid for new rows.
#[derive(Debug, Clone)]
pub enum RowRef {
    /// Reference to existing row by UUID
    Uuid(Uuid),
    /// Reference to a row being inserted in this transaction
    Named(String),
}

impl RowRef {
    /// Convert to JSON value for OVSDB protocol.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Uuid(uuid) => json!(["uuid", uuid.to_string()]),
            Self::Named(name) => json!(["named-uuid", name]),
        }
    }
}

impl From<Uuid> for RowRef {
    fn from(uuid: Uuid) -> Self {
        Self::Uuid(uuid)
    }
}

impl From<&str> for RowRef {
    fn from(name: &str) -> Self {
        Self::Named(name.to_owned())
    }
}

impl From<String> for RowRef {
    fn from(name: String) -> Self {
        Self::Named(name)
    }
}

/// An OVSDB transaction.
pub struct Transaction {
    /// Database name
    db_name: String,
    /// Operations to perform
    operations: Vec<Value>,
    /// Status
    status: TransactionStatus,
    /// Mapping from named-uuid to actual UUID after commit
    uuid_map: HashMap<String, Uuid>,
    /// UUID counter for named-uuid
    uuid_counter: u32,
}

impl Transaction {
    /// Create a new transaction for the given database.
    pub fn new(db_name: impl Into<String>) -> Self {
        Self {
            db_name: db_name.into(),
            operations: Vec::new(),
            status: TransactionStatus::Uncommitted,
            uuid_map: HashMap::new(),
            uuid_counter: 0,
        }
    }

    /// Get the transaction status.
    pub fn status(&self) -> TransactionStatus {
        self.status
    }

    /// Get the database name.
    pub fn db_name(&self) -> &str {
        &self.db_name
    }

    /// Generate a temporary UUID name.
    fn next_temp_uuid(&mut self) -> String {
        let name = format!("row{}", self.uuid_counter);
        self.uuid_counter += 1;
        name
    }

    /// Insert a new row into a table.
    ///
    /// Returns a `RowRef::Named` that can be used to reference this row
    /// in subsequent operations within the same transaction.
    pub fn insert(&mut self, table: &str, row: Value) -> RowRef {
        let uuid_name = self.next_temp_uuid();

        self.operations.push(json!({
            "op": "insert",
            "table": table,
            "row": row,
            "uuid-name": uuid_name
        }));

        RowRef::Named(uuid_name)
    }

    /// Insert a new row and return a reference for use in mutate operations.
    pub fn insert_raw(&mut self, table: &str, row: HashMap<String, Value>) -> RowRef {
        self.insert(table, json!(row))
    }

    /// Update a row by UUID.
    pub fn update(&mut self, table: &str, uuid: Uuid, columns: Value) {
        self.operations.push(json!({
            "op": "update",
            "table": table,
            "where": [["_uuid", "==", ["uuid", uuid.to_string()]]],
            "row": columns
        }));
    }

    /// Mutate a row - add/remove from sets or maps.
    pub fn mutate(&mut self, table: &str, uuid: Uuid, mutations: Vec<Value>) {
        self.operations.push(json!({
            "op": "mutate",
            "table": table,
            "where": [["_uuid", "==", ["uuid", uuid.to_string()]]],
            "mutations": mutations
        }));
    }

    /// Mutate a row by name condition.
    pub fn mutate_by_name(&mut self, table: &str, name: &str, mutations: Vec<Value>) {
        self.operations.push(json!({
            "op": "mutate",
            "table": table,
            "where": [["name", "==", name]],
            "mutations": mutations
        }));
    }

    /// Mutate rows matching a custom where clause.
    pub fn mutate_where(&mut self, table: &str, condition: Value, mutations: Vec<Value>) {
        self.operations.push(json!({
            "op": "mutate",
            "table": table,
            "where": condition,
            "mutations": mutations
        }));
    }

    /// Delete a row by UUID.
    pub fn delete(&mut self, table: &str, uuid: Uuid) {
        self.operations.push(json!({
            "op": "delete",
            "table": table,
            "where": [["_uuid", "==", ["uuid", uuid.to_string()]]]
        }));
    }

    /// Delete rows by name.
    pub fn delete_by_name(&mut self, table: &str, name: &str) {
        self.operations.push(json!({
            "op": "delete",
            "table": table,
            "where": [["name", "==", name]]
        }));
    }

    /// Delete rows matching a condition.
    pub fn delete_where(&mut self, table: &str, condition: Value) {
        self.operations.push(json!({
            "op": "delete",
            "table": table,
            "where": condition
        }));
    }

    /// Add a wait operation.
    pub fn wait(
        &mut self,
        table: &str,
        columns: Vec<String>,
        condition: Value,
        expected: Value,
    ) {
        self.operations.push(json!({
            "op": "wait",
            "table": table,
            "columns": columns,
            "where": condition,
            "rows": expected,
            "until": "==",
            "timeout": 0
        }));
    }

    /// Add a comment to the transaction.
    pub fn comment(&mut self, comment: impl Into<String>) {
        self.operations.push(json!({
            "op": "comment",
            "comment": comment.into()
        }));
    }

    /// Build the transaction request parameters.
    pub fn build(&self) -> Value {
        let mut ops: Vec<Value> = vec![self.db_name.clone().into()];
        ops.extend(self.operations.clone());
        Value::Array(ops)
    }

    /// Get the operations (for debugging).
    pub fn operations(&self) -> &[Value] {
        &self.operations
    }

    /// Check if the transaction has any operations.
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Get the UUID map after commit.
    pub fn uuid_map(&self) -> &HashMap<String, Uuid> {
        &self.uuid_map
    }

    /// Look up the actual UUID for a named-uuid after commit.
    pub fn get_uuid(&self, name: &str) -> Option<Uuid> {
        self.uuid_map.get(name).copied()
    }

    /// Process the transaction result and update status/uuid_map.
    pub fn process_result(&mut self, result: &Value) -> bool {
        if let Some(arr) = result.as_array() {
            for (i, op_result) in arr.iter().enumerate() {
                // Check for errors
                if let Some(error) = op_result.get("error") {
                    if !error.is_null() {
                        tracing::error!("Transaction op {} failed: {:?}", i, op_result);
                        self.status = TransactionStatus::Error;
                        return false;
                    }
                }

                // Extract UUIDs from insert results
                if let Some(uuid_obj) = op_result.get("uuid") {
                    if let Some(uuid_arr) = uuid_obj.as_array() {
                        if uuid_arr.len() == 2 && uuid_arr[0] == "uuid" {
                            if let Some(uuid_str) = uuid_arr[1].as_str() {
                                if let Ok(uuid) = uuid_str.parse::<Uuid>() {
                                    // Map row{i} to the actual UUID
                                    let name = format!("row{}", i);
                                    self.uuid_map.insert(name, uuid);
                                }
                            }
                        }
                    }
                }
            }

            self.status = TransactionStatus::Success;
            true
        } else {
            self.status = TransactionStatus::Error;
            false
        }
    }

    /// Mark the transaction as successful.
    pub fn set_success(&mut self) {
        self.status = TransactionStatus::Success;
    }

    /// Mark the transaction as failed.
    pub fn set_error(&mut self) {
        self.status = TransactionStatus::Error;
    }

    /// Mark as needing retry.
    pub fn set_try_again(&mut self) {
        self.status = TransactionStatus::TryAgain;
    }

    /// Abort the transaction.
    pub fn abort(&mut self) {
        self.status = TransactionStatus::Aborted;
    }
}

// ============================================================================
// High-level topology operations
// ============================================================================

impl Transaction {
    /// Create a bridge with a default internal port.
    ///
    /// Returns references to the bridge, port, and interface rows.
    pub fn create_bridge(&mut self, name: &str) -> (RowRef, RowRef, RowRef) {
        // Create interface
        let iface_ref = self.insert("Interface", json!({
            "name": name,
            "type": "internal"
        }));

        // Create port referencing the interface
        let port_ref = self.insert("Port", json!({
            "name": name,
            "interfaces": iface_ref.to_json()
        }));

        // Create bridge referencing the port
        let bridge_ref = self.insert("Bridge", json!({
            "name": name,
            "ports": port_ref.to_json()
        }));

        // Add bridge to Open_vSwitch table (empty where = match the single row)
        self.mutate_where("Open_vSwitch", json!([]), vec![
            json!(["bridges", "insert", bridge_ref.to_json()])
        ]);

        (bridge_ref, port_ref, iface_ref)
    }

    /// Delete a bridge by UUID.
    ///
    /// This removes the bridge from Open_vSwitch.bridges and deletes all
    /// associated rows (bridge, ports, interfaces).
    ///
    /// Note: You must provide the bridge UUID. Use `Client::idl()` to look up
    /// the UUID from the bridge name before calling this.
    pub fn delete_bridge_uuid(
        &mut self,
        bridge_uuid: Uuid,
        port_uuids: &[Uuid],
        iface_uuids: &[Uuid],
    ) {
        // Remove bridge from Open_vSwitch.bridges
        self.mutate_where("Open_vSwitch", json!([]), vec![
            json!(["bridges", "delete", ["set", [["uuid", bridge_uuid.to_string()]]]])
        ]);

        // Delete interface rows first (referenced by ports)
        for uuid in iface_uuids {
            self.delete("Interface", *uuid);
        }

        // Delete port rows (referenced by bridge)
        for uuid in port_uuids {
            self.delete("Port", *uuid);
        }

        // Delete bridge row
        self.delete("Bridge", bridge_uuid);
    }

    /// Delete a bridge by name (simple version).
    ///
    /// Note: This deletes rows by name but may fail if the bridge is still
    /// referenced. For reliable deletion, use `delete_bridge_uuid` after
    /// looking up UUIDs from the IDL.
    pub fn delete_bridge(&mut self, name: &str) {
        // Delete associated interface (must delete before port due to refs)
        self.delete_by_name("Interface", name);

        // Delete associated port (must delete before bridge due to refs)
        self.delete_by_name("Port", name);

        // Delete bridge row - this may fail if still in Open_vSwitch.bridges
        self.delete_by_name("Bridge", name);
    }

    /// Add a port to an existing bridge.
    ///
    /// Returns references to the port and interface rows.
    pub fn add_port(
        &mut self,
        bridge_name: &str,
        port_name: &str,
        iface_type: &str,
    ) -> (RowRef, RowRef) {
        // Create interface
        let iface_ref = self.insert("Interface", json!({
            "name": port_name,
            "type": iface_type
        }));

        // Create port referencing the interface
        let port_ref = self.insert("Port", json!({
            "name": port_name,
            "interfaces": iface_ref.to_json()
        }));

        // Add port to bridge
        self.mutate_by_name("Bridge", bridge_name, vec![
            json!(["ports", "insert", port_ref.to_json()])
        ]);

        (port_ref, iface_ref)
    }

    /// Add an internal port to a bridge.
    pub fn add_internal_port(&mut self, bridge_name: &str, port_name: &str) -> (RowRef, RowRef) {
        self.add_port(bridge_name, port_name, "internal")
    }

    /// Add a VLAN port (internal type with tag).
    pub fn add_vlan_port(
        &mut self,
        bridge_name: &str,
        port_name: &str,
        vlan_id: u16,
    ) -> (RowRef, RowRef) {
        // Create interface
        let iface_ref = self.insert("Interface", json!({
            "name": port_name,
            "type": "internal"
        }));

        // Create port with VLAN tag
        let port_ref = self.insert("Port", json!({
            "name": port_name,
            "interfaces": iface_ref.to_json(),
            "tag": vlan_id
        }));

        // Add port to bridge
        self.mutate_by_name("Bridge", bridge_name, vec![
            json!(["ports", "insert", port_ref.to_json()])
        ]);

        (port_ref, iface_ref)
    }

    /// Create a patch port pair connecting two bridges.
    ///
    /// Returns (port1, iface1, port2, iface2).
    pub fn add_patch_ports(
        &mut self,
        bridge1: &str,
        bridge2: &str,
        port1_name: Option<&str>,
        port2_name: Option<&str>,
    ) -> (RowRef, RowRef, RowRef, RowRef) {
        // Use provided names or generate defaults
        let p1_name = port1_name.map(|s| s.to_owned())
            .unwrap_or_else(|| format!("patch-{}-to-{}", bridge1, bridge2));
        let p2_name = port2_name.map(|s| s.to_owned())
            .unwrap_or_else(|| format!("patch-{}-to-{}", bridge2, bridge1));

        // Create interface 1 (patch type with peer option)
        let iface1_ref = self.insert("Interface", json!({
            "name": p1_name,
            "type": "patch",
            "options": ["map", [["peer", p2_name]]]
        }));

        // Create port 1
        let port1_ref = self.insert("Port", json!({
            "name": p1_name,
            "interfaces": iface1_ref.to_json()
        }));

        // Add port1 to bridge1
        self.mutate_by_name("Bridge", bridge1, vec![
            json!(["ports", "insert", port1_ref.to_json()])
        ]);

        // Create interface 2 (patch type with peer option)
        let iface2_ref = self.insert("Interface", json!({
            "name": p2_name,
            "type": "patch",
            "options": ["map", [["peer", p1_name]]]
        }));

        // Create port 2
        let port2_ref = self.insert("Port", json!({
            "name": p2_name,
            "interfaces": iface2_ref.to_json()
        }));

        // Add port2 to bridge2
        self.mutate_by_name("Bridge", bridge2, vec![
            json!(["ports", "insert", port2_ref.to_json()])
        ]);

        (port1_ref, iface1_ref, port2_ref, iface2_ref)
    }

    /// Delete a port by UUID.
    ///
    /// Removes the port from the bridge and deletes the port and interface rows.
    pub fn delete_port_uuid(
        &mut self,
        bridge_uuid: Uuid,
        port_uuid: Uuid,
        iface_uuid: Uuid,
    ) {
        // Remove port from bridge's ports set
        self.mutate("Bridge", bridge_uuid, vec![
            json!(["ports", "delete", ["set", [["uuid", port_uuid.to_string()]]]])
        ]);

        // Delete interface
        self.delete("Interface", iface_uuid);

        // Delete port
        self.delete("Port", port_uuid);
    }

    /// Delete a port by name (simple version).
    ///
    /// Note: This deletes rows by name but may fail due to references.
    /// For reliable deletion, use `delete_port_uuid` after looking up UUIDs.
    pub fn delete_port(&mut self, _bridge_name: &str, port_name: &str) {
        // Delete interface first (referenced by port)
        self.delete_by_name("Interface", port_name);

        // Delete port - may fail if still in bridge's ports set
        self.delete_by_name("Port", port_name);
    }
}
