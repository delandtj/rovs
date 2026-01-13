//! OVSDB transaction handling.

use std::collections::HashMap;

use serde_json::{Value, json};
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
    /// Returns a [`RowRef::Named`] that can be used to reference this row
    /// in subsequent operations within the same transaction. After commit,
    /// use [`get_uuid`](Self::get_uuid) to retrieve the actual UUID.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let iface_ref = txn.insert("Interface", json!({"name": "eth0", "type": "internal"}));
    /// let port_ref = txn.insert("Port", json!({
    ///     "name": "eth0",
    ///     "interfaces": iface_ref.to_json()  // Reference the interface
    /// }));
    /// ```
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

    /// Insert a new row using a [`HashMap`] instead of a JSON value.
    ///
    /// This is a convenience wrapper around [`insert`](Self::insert) for cases
    /// where building a `HashMap` is more natural than using `serde_json::json!`.
    pub fn insert_raw(&mut self, table: &str, row: HashMap<String, Value>) -> RowRef {
        self.insert(table, json!(row))
    }

    /// Update columns in an existing row identified by UUID.
    ///
    /// Only the columns specified in `columns` are modified; other columns
    /// retain their current values.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Update a bridge's fail_mode
    /// txn.update("Bridge", bridge_uuid, json!({"fail_mode": "secure"}));
    /// ```
    pub fn update(&mut self, table: &str, uuid: Uuid, columns: Value) {
        self.operations.push(json!({
            "op": "update",
            "table": table,
            "where": [["_uuid", "==", ["uuid", uuid.to_string()]]],
            "row": columns
        }));
    }

    /// Mutate a row by adding/removing elements from sets or maps.
    ///
    /// Unlike [`update`](Self::update), mutations modify collection columns
    /// without replacing them entirely. Common mutations:
    /// - `["column", "insert", value]` - Add to set/map
    /// - `["column", "delete", value]` - Remove from set/map
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Add a port UUID to a bridge's ports set
    /// txn.mutate("Bridge", bridge_uuid, vec![
    ///     json!(["ports", "insert", ["uuid", port_uuid.to_string()]])
    /// ]);
    /// ```
    pub fn mutate(&mut self, table: &str, uuid: Uuid, mutations: Vec<Value>) {
        self.operations.push(json!({
            "op": "mutate",
            "table": table,
            "where": [["_uuid", "==", ["uuid", uuid.to_string()]]],
            "mutations": mutations
        }));
    }

    /// Mutate rows matching `name == value`.
    ///
    /// Convenience wrapper for [`mutate_where`](Self::mutate_where) using a
    /// name-based condition. Useful for tables with unique name columns.
    pub fn mutate_by_name(&mut self, table: &str, name: &str, mutations: Vec<Value>) {
        self.operations.push(json!({
            "op": "mutate",
            "table": table,
            "where": [["name", "==", name]],
            "mutations": mutations
        }));
    }

    /// Mutate rows matching a custom condition.
    ///
    /// The condition should be an OVSDB where clause array. An empty array
    /// `json!([])` matches all rows (useful for singleton tables like `Open_vSwitch`).
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Add bridge to Open_vSwitch.bridges (singleton table)
    /// txn.mutate_where("Open_vSwitch", json!([]), vec![
    ///     json!(["bridges", "insert", bridge_ref.to_json()])
    /// ]);
    /// ```
    pub fn mutate_where(&mut self, table: &str, condition: Value, mutations: Vec<Value>) {
        self.operations.push(json!({
            "op": "mutate",
            "table": table,
            "where": condition,
            "mutations": mutations
        }));
    }

    /// Delete a row by its UUID.
    ///
    /// # Note
    ///
    /// OVSDB enforces referential integrity. If other rows have strong references
    /// to this row, the transaction will fail. Remove references first using
    /// [`mutate`](Self::mutate).
    pub fn delete(&mut self, table: &str, uuid: Uuid) {
        self.operations.push(json!({
            "op": "delete",
            "table": table,
            "where": [["_uuid", "==", ["uuid", uuid.to_string()]]]
        }));
    }

    /// Delete rows where `name == value`.
    ///
    /// Convenience wrapper for [`delete_where`](Self::delete_where).
    /// May delete multiple rows if names are not unique.
    pub fn delete_by_name(&mut self, table: &str, name: &str) {
        self.operations.push(json!({
            "op": "delete",
            "table": table,
            "where": [["name", "==", name]]
        }));
    }

    /// Delete all rows matching a custom condition.
    ///
    /// The condition should be an OVSDB where clause array.
    pub fn delete_where(&mut self, table: &str, condition: Value) {
        self.operations.push(json!({
            "op": "delete",
            "table": table,
            "where": condition
        }));
    }

    /// Add a wait operation that blocks until a condition is met.
    ///
    /// The transaction will not proceed until the specified rows match
    /// the expected values. Useful for synchronization or ordering guarantees.
    ///
    /// # Arguments
    ///
    /// * `table` - Table to wait on
    /// * `columns` - Columns to check
    /// * `condition` - Where clause to select rows
    /// * `expected` - Expected row values
    pub fn wait(&mut self, table: &str, columns: Vec<String>, condition: Value, expected: Value) {
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

    /// Add a comment to the transaction for debugging/logging.
    ///
    /// Comments are recorded in OVSDB logs and can help trace operations.
    pub fn comment(&mut self, comment: impl Into<String>) {
        self.operations.push(json!({
            "op": "comment",
            "comment": comment.into()
        }));
    }

    /// Build the JSON-RPC parameters for the `transact` method.
    ///
    /// Returns an array with the database name followed by all operations.
    /// This is called internally by [`Client::commit`](crate::Client::commit).
    pub fn build(&self) -> Value {
        let mut ops: Vec<Value> = vec![self.db_name.clone().into()];
        ops.extend(self.operations.clone());
        Value::Array(ops)
    }

    /// Get the list of operations for debugging or inspection.
    pub fn operations(&self) -> &[Value] {
        &self.operations
    }

    /// Check if the transaction has no operations.
    ///
    /// Empty transactions can be committed but have no effect.
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Get the mapping from named-uuids to actual UUIDs after commit.
    ///
    /// This map is populated by [`process_result`](Self::process_result) after
    /// a successful commit. Keys are like "row0", "row1", etc.
    pub fn uuid_map(&self) -> &HashMap<String, Uuid> {
        &self.uuid_map
    }

    /// Look up the actual UUID for a named-uuid after commit.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let bridge_ref = txn.create_bridge("br0");  // Returns RowRef::Named("row2")
    /// client.commit(&mut txn).await?;
    /// let bridge_uuid = txn.get_uuid("row2").expect("bridge was created");
    /// ```
    pub fn get_uuid(&self, name: &str) -> Option<Uuid> {
        self.uuid_map.get(name).copied()
    }

    /// Process the transaction result from the server.
    ///
    /// Extracts UUIDs from insert results and populates [`uuid_map`](Self::uuid_map).
    /// Updates the transaction status based on success/failure.
    ///
    /// Returns `true` if the transaction succeeded, `false` otherwise.
    ///
    /// This is called internally by [`Client::commit`](crate::Client::commit).
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

    /// Manually mark the transaction as successful.
    ///
    /// Normally set automatically by [`process_result`](Self::process_result).
    pub fn set_success(&mut self) {
        self.status = TransactionStatus::Success;
    }

    /// Manually mark the transaction as failed with an error.
    pub fn set_error(&mut self) {
        self.status = TransactionStatus::Error;
    }

    /// Mark the transaction as needing retry (e.g., due to a conflict).
    pub fn set_try_again(&mut self) {
        self.status = TransactionStatus::TryAgain;
    }

    /// Mark the transaction as explicitly aborted.
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
    /// Every OVS bridge must have at least one port (its "local" port).
    /// This method creates the bridge, a port, and an internal interface,
    /// all with the same name, and registers the bridge with `Open_vSwitch`.
    ///
    /// Returns `(bridge_ref, port_ref, iface_ref)` for use in subsequent operations.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let (bridge_ref, _, _) = txn.create_bridge("br0");
    /// client.commit(&mut txn).await?;
    /// ```
    pub fn create_bridge(&mut self, name: &str) -> (RowRef, RowRef, RowRef) {
        // Create interface
        let iface_ref = self.insert(
            "Interface",
            json!({
                "name": name,
                "type": "internal"
            }),
        );

        // Create port referencing the interface
        let port_ref = self.insert(
            "Port",
            json!({
                "name": name,
                "interfaces": iface_ref.to_json()
            }),
        );

        // Create bridge referencing the port
        let bridge_ref = self.insert(
            "Bridge",
            json!({
                "name": name,
                "ports": port_ref.to_json()
            }),
        );

        // Add bridge to Open_vSwitch table (empty where = match the single row)
        self.mutate_where(
            "Open_vSwitch",
            json!([]),
            vec![json!(["bridges", "insert", bridge_ref.to_json()])],
        );

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
        self.mutate_where(
            "Open_vSwitch",
            json!([]),
            vec![json!([
                "bridges",
                "delete",
                ["set", [["uuid", bridge_uuid.to_string()]]]
            ])],
        );

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

    /// Add a port with a custom interface type to an existing bridge.
    ///
    /// Creates an interface with the specified type, wraps it in a port,
    /// and adds the port to the bridge's `ports` set.
    ///
    /// Returns `(port_ref, iface_ref)` for use in subsequent operations.
    ///
    /// # Arguments
    ///
    /// * `bridge_name` - Name of the existing bridge
    /// * `port_name` - Name for the new port and interface
    /// * `iface_type` - Interface type (e.g., "internal", "system", "patch")
    pub fn add_port(
        &mut self,
        bridge_name: &str,
        port_name: &str,
        iface_type: &str,
    ) -> (RowRef, RowRef) {
        // Create interface
        let iface_ref = self.insert(
            "Interface",
            json!({
                "name": port_name,
                "type": iface_type
            }),
        );

        // Create port referencing the interface
        let port_ref = self.insert(
            "Port",
            json!({
                "name": port_name,
                "interfaces": iface_ref.to_json()
            }),
        );

        // Add port to bridge
        self.mutate_by_name(
            "Bridge",
            bridge_name,
            vec![json!(["ports", "insert", port_ref.to_json()])],
        );

        (port_ref, iface_ref)
    }

    /// Add an internal port to a bridge.
    ///
    /// Convenience wrapper for [`add_port`](Self::add_port) with type "internal".
    /// Internal ports are virtual interfaces managed by OVS (not physical NICs).
    pub fn add_internal_port(&mut self, bridge_name: &str, port_name: &str) -> (RowRef, RowRef) {
        self.add_port(bridge_name, port_name, "internal")
    }

    /// Add a system port to a bridge.
    ///
    /// System ports represent physical or virtual network interfaces (NICs, veth
    /// pairs, etc.) that exist in the kernel. The interface must already exist.
    ///
    /// # Arguments
    ///
    /// * `bridge_name` - Name of the existing bridge
    /// * `port_name` - Name of the existing kernel interface to attach
    pub fn add_system_port(&mut self, bridge_name: &str, port_name: &str) -> (RowRef, RowRef) {
        self.add_port(bridge_name, port_name, "system")
    }

    /// Add a VLAN access port (internal type with a VLAN tag).
    ///
    /// Creates an internal port with the specified VLAN ID. Traffic on this
    /// port will be tagged/untagged according to the VLAN configuration.
    ///
    /// # Arguments
    ///
    /// * `bridge_name` - Name of the existing bridge
    /// * `port_name` - Name for the new port
    /// * `vlan_id` - VLAN ID (1-4094)
    pub fn add_vlan_port(
        &mut self,
        bridge_name: &str,
        port_name: &str,
        vlan_id: u16,
    ) -> (RowRef, RowRef) {
        // Create interface
        let iface_ref = self.insert(
            "Interface",
            json!({
                "name": port_name,
                "type": "internal"
            }),
        );

        // Create port with VLAN tag
        let port_ref = self.insert(
            "Port",
            json!({
                "name": port_name,
                "interfaces": iface_ref.to_json(),
                "tag": vlan_id
            }),
        );

        // Add port to bridge
        self.mutate_by_name(
            "Bridge",
            bridge_name,
            vec![json!(["ports", "insert", port_ref.to_json()])],
        );

        (port_ref, iface_ref)
    }

    /// Create a patch port pair connecting two bridges.
    ///
    /// Patch ports are virtual cables connecting two OVS bridges. Each end
    /// references the other via the `peer` option.
    ///
    /// Default port names are `patch-{bridge1}-to-{bridge2}` and vice versa.
    ///
    /// Returns `(port1_ref, iface1_ref, port2_ref, iface2_ref)`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// txn.create_bridge("br-int");
    /// txn.create_bridge("br-ext");
    /// txn.add_patch_ports("br-int", "br-ext", None, None);
    /// ```
    pub fn add_patch_ports(
        &mut self,
        bridge1: &str,
        bridge2: &str,
        port1_name: Option<&str>,
        port2_name: Option<&str>,
    ) -> (RowRef, RowRef, RowRef, RowRef) {
        // Use provided names or generate defaults
        let p1_name = port1_name
            .map(|s| s.to_owned())
            .unwrap_or_else(|| format!("patch-{}-to-{}", bridge1, bridge2));
        let p2_name = port2_name
            .map(|s| s.to_owned())
            .unwrap_or_else(|| format!("patch-{}-to-{}", bridge2, bridge1));

        // Create interface 1 (patch type with peer option)
        let iface1_ref = self.insert(
            "Interface",
            json!({
                "name": p1_name,
                "type": "patch",
                "options": ["map", [["peer", p2_name]]]
            }),
        );

        // Create port 1
        let port1_ref = self.insert(
            "Port",
            json!({
                "name": p1_name,
                "interfaces": iface1_ref.to_json()
            }),
        );

        // Add port1 to bridge1
        self.mutate_by_name(
            "Bridge",
            bridge1,
            vec![json!(["ports", "insert", port1_ref.to_json()])],
        );

        // Create interface 2 (patch type with peer option)
        let iface2_ref = self.insert(
            "Interface",
            json!({
                "name": p2_name,
                "type": "patch",
                "options": ["map", [["peer", p1_name]]]
            }),
        );

        // Create port 2
        let port2_ref = self.insert(
            "Port",
            json!({
                "name": p2_name,
                "interfaces": iface2_ref.to_json()
            }),
        );

        // Add port2 to bridge2
        self.mutate_by_name(
            "Bridge",
            bridge2,
            vec![json!(["ports", "insert", port2_ref.to_json()])],
        );

        (port1_ref, iface1_ref, port2_ref, iface2_ref)
    }

    /// Create a patch port pair with VLAN trunk configuration.
    ///
    /// Like [`add_patch_ports`](Self::add_patch_ports), but configures the ports
    /// as VLAN trunks that only allow specified VLANs to pass through.
    ///
    /// # Arguments
    ///
    /// * `bridge1` - First bridge name
    /// * `bridge2` - Second bridge name
    /// * `vlans` - List of VLAN IDs to allow (1-4094)
    /// * `port1_name` - Optional custom name for first patch port
    /// * `port2_name` - Optional custom name for second patch port
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Allow only VLANs 100 and 200 between bridges
    /// txn.add_trunk_patch_ports("br-int", "br-ext", &[100, 200], None, None);
    /// ```
    pub fn add_trunk_patch_ports(
        &mut self,
        bridge1: &str,
        bridge2: &str,
        vlans: &[u16],
        port1_name: Option<&str>,
        port2_name: Option<&str>,
    ) -> (RowRef, RowRef, RowRef, RowRef) {
        // Use provided names or generate defaults
        let p1_name = port1_name
            .map(|s| s.to_owned())
            .unwrap_or_else(|| format!("patch-{}-to-{}", bridge1, bridge2));
        let p2_name = port2_name
            .map(|s| s.to_owned())
            .unwrap_or_else(|| format!("patch-{}-to-{}", bridge2, bridge1));

        // Format VLAN list for OVSDB set encoding
        let vlan_set = if vlans.len() == 1 {
            // Single value doesn't need set encoding
            json!(vlans[0])
        } else {
            // Multiple values need ["set", [...]]
            json!(["set", vlans])
        };

        // Create interface 1 (patch type with peer option)
        let iface1_ref = self.insert(
            "Interface",
            json!({
                "name": p1_name,
                "type": "patch",
                "options": ["map", [["peer", p2_name]]]
            }),
        );

        // Create port 1 with trunk configuration
        let port1_ref = self.insert(
            "Port",
            json!({
                "name": p1_name,
                "interfaces": iface1_ref.to_json(),
                "vlan_mode": "trunk",
                "trunks": vlan_set
            }),
        );

        // Add port1 to bridge1
        self.mutate_by_name(
            "Bridge",
            bridge1,
            vec![json!(["ports", "insert", port1_ref.to_json()])],
        );

        // Create interface 2 (patch type with peer option)
        let iface2_ref = self.insert(
            "Interface",
            json!({
                "name": p2_name,
                "type": "patch",
                "options": ["map", [["peer", p1_name]]]
            }),
        );

        // Create port 2 with trunk configuration
        let port2_ref = self.insert(
            "Port",
            json!({
                "name": p2_name,
                "interfaces": iface2_ref.to_json(),
                "vlan_mode": "trunk",
                "trunks": vlan_set
            }),
        );

        // Add port2 to bridge2
        self.mutate_by_name(
            "Bridge",
            bridge2,
            vec![json!(["ports", "insert", port2_ref.to_json()])],
        );

        (port1_ref, iface1_ref, port2_ref, iface2_ref)
    }

    /// Delete a port by UUID (recommended method).
    ///
    /// Removes the port from the bridge's `ports` set, then deletes the
    /// interface and port rows. This is the safe way to delete ports as it
    /// properly handles referential integrity.
    ///
    /// # Arguments
    ///
    /// * `bridge_uuid` - UUID of the bridge containing the port
    /// * `port_uuid` - UUID of the port to delete
    /// * `iface_uuid` - UUID of the port's interface
    pub fn delete_port_uuid(&mut self, bridge_uuid: Uuid, port_uuid: Uuid, iface_uuid: Uuid) {
        // Remove port from bridge's ports set
        self.mutate(
            "Bridge",
            bridge_uuid,
            vec![json!([
                "ports",
                "delete",
                ["set", [["uuid", port_uuid.to_string()]]]
            ])],
        );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_generates_correct_json() {
        let mut txn = Transaction::new("Open_vSwitch");
        let row_ref = txn.insert("Bridge", json!({"name": "br0"}));

        assert!(matches!(row_ref, RowRef::Named(ref name) if name == "row0"));
        assert_eq!(txn.operations().len(), 1);

        let op = &txn.operations()[0];
        assert_eq!(op["op"], "insert");
        assert_eq!(op["table"], "Bridge");
        assert_eq!(op["uuid-name"], "row0");
        assert_eq!(op["row"]["name"], "br0");
    }

    #[test]
    fn insert_increments_uuid_counter() {
        let mut txn = Transaction::new("Open_vSwitch");

        let ref1 = txn.insert("Interface", json!({"name": "eth0"}));
        let ref2 = txn.insert("Port", json!({"name": "eth0"}));
        let ref3 = txn.insert("Bridge", json!({"name": "br0"}));

        assert!(matches!(ref1, RowRef::Named(ref n) if n == "row0"));
        assert!(matches!(ref2, RowRef::Named(ref n) if n == "row1"));
        assert!(matches!(ref3, RowRef::Named(ref n) if n == "row2"));
    }

    #[test]
    fn update_generates_correct_where_clause() {
        let mut txn = Transaction::new("Open_vSwitch");
        let uuid = Uuid::new_v4();

        txn.update("Bridge", uuid, json!({"fail_mode": "secure"}));

        let op = &txn.operations()[0];
        assert_eq!(op["op"], "update");
        assert_eq!(op["table"], "Bridge");
        assert_eq!(op["where"][0][0], "_uuid");
        assert_eq!(op["where"][0][1], "==");
        assert_eq!(op["row"]["fail_mode"], "secure");
    }

    #[test]
    fn delete_generates_correct_json() {
        let mut txn = Transaction::new("Open_vSwitch");
        let uuid = Uuid::new_v4();

        txn.delete("Bridge", uuid);

        let op = &txn.operations()[0];
        assert_eq!(op["op"], "delete");
        assert_eq!(op["table"], "Bridge");
        assert_eq!(op["where"][0][0], "_uuid");
    }

    #[test]
    fn delete_by_name_uses_name_condition() {
        let mut txn = Transaction::new("Open_vSwitch");

        txn.delete_by_name("Bridge", "br0");

        let op = &txn.operations()[0];
        assert_eq!(op["op"], "delete");
        assert_eq!(op["where"][0][0], "name");
        assert_eq!(op["where"][0][1], "==");
        assert_eq!(op["where"][0][2], "br0");
    }

    #[test]
    fn mutate_generates_correct_mutations() {
        let mut txn = Transaction::new("Open_vSwitch");
        let uuid = Uuid::new_v4();

        txn.mutate(
            "Bridge",
            uuid,
            vec![json!(["ports", "insert", ["named-uuid", "row0"]])],
        );

        let op = &txn.operations()[0];
        assert_eq!(op["op"], "mutate");
        assert_eq!(op["table"], "Bridge");
        assert_eq!(op["mutations"][0][0], "ports");
        assert_eq!(op["mutations"][0][1], "insert");
    }

    #[test]
    fn mutate_where_with_empty_condition() {
        let mut txn = Transaction::new("Open_vSwitch");

        txn.mutate_where(
            "Open_vSwitch",
            json!([]),
            vec![json!(["bridges", "insert", ["named-uuid", "row0"]])],
        );

        let op = &txn.operations()[0];
        assert_eq!(op["where"], json!([]));
    }

    #[test]
    fn build_prepends_database_name() {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.insert("Bridge", json!({"name": "br0"}));

        let params = txn.build();
        let arr = params.as_array().unwrap();

        assert_eq!(arr[0], "Open_vSwitch");
        assert_eq!(arr[1]["op"], "insert");
    }

    #[test]
    fn row_ref_to_json() {
        let named = RowRef::Named("row0".to_string());
        assert_eq!(named.to_json(), json!(["named-uuid", "row0"]));

        let uuid = Uuid::nil();
        let uuid_ref = RowRef::Uuid(uuid);
        assert_eq!(uuid_ref.to_json(), json!(["uuid", uuid.to_string()]));
    }

    #[test]
    fn process_result_extracts_uuids() {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.insert("Interface", json!({"name": "eth0"}));
        txn.insert("Port", json!({"name": "eth0"}));

        let result = json!([
            {"uuid": ["uuid", "11111111-1111-1111-1111-111111111111"]},
            {"uuid": ["uuid", "22222222-2222-2222-2222-222222222222"]}
        ]);

        let success = txn.process_result(&result);

        assert!(success);
        assert_eq!(txn.status(), TransactionStatus::Success);
        assert!(txn.get_uuid("row0").is_some());
        assert!(txn.get_uuid("row1").is_some());
    }

    #[test]
    fn process_result_detects_errors() {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.insert("Bridge", json!({"name": "br0"}));

        let result = json!([
            {"error": "constraint violation", "details": "duplicate key"}
        ]);

        let success = txn.process_result(&result);

        assert!(!success);
        assert_eq!(txn.status(), TransactionStatus::Error);
    }

    #[test]
    fn create_bridge_generates_four_operations() {
        let mut txn = Transaction::new("Open_vSwitch");
        let (bridge_ref, port_ref, iface_ref) = txn.create_bridge("br0");

        // Should create: interface, port, bridge, mutate Open_vSwitch
        assert_eq!(txn.operations().len(), 4);

        assert!(matches!(iface_ref, RowRef::Named(ref n) if n == "row0"));
        assert!(matches!(port_ref, RowRef::Named(ref n) if n == "row1"));
        assert!(matches!(bridge_ref, RowRef::Named(ref n) if n == "row2"));

        // Verify interface
        assert_eq!(txn.operations()[0]["table"], "Interface");
        assert_eq!(txn.operations()[0]["row"]["name"], "br0");
        assert_eq!(txn.operations()[0]["row"]["type"], "internal");

        // Verify port references interface
        assert_eq!(txn.operations()[1]["table"], "Port");
        assert_eq!(
            txn.operations()[1]["row"]["interfaces"],
            json!(["named-uuid", "row0"])
        );

        // Verify bridge references port
        assert_eq!(txn.operations()[2]["table"], "Bridge");
        assert_eq!(
            txn.operations()[2]["row"]["ports"],
            json!(["named-uuid", "row1"])
        );

        // Verify mutate adds bridge to Open_vSwitch
        assert_eq!(txn.operations()[3]["op"], "mutate");
        assert_eq!(txn.operations()[3]["table"], "Open_vSwitch");
    }

    #[test]
    fn add_vlan_port_sets_tag() {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.add_vlan_port("br0", "vlan100", 100);

        // Interface, Port, Mutate
        assert_eq!(txn.operations().len(), 3);

        // Port should have tag
        let port_op = &txn.operations()[1];
        assert_eq!(port_op["row"]["tag"], 100);
    }

    #[test]
    fn add_patch_ports_creates_peer_options() {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.add_patch_ports("br-int", "br-ext", None, None);

        // 2 interfaces, 2 ports, 2 mutates
        assert_eq!(txn.operations().len(), 6);

        // Check first interface has peer pointing to second
        let iface1 = &txn.operations()[0];
        assert_eq!(iface1["row"]["type"], "patch");
        assert_eq!(
            iface1["row"]["options"],
            json!(["map", [["peer", "patch-br-ext-to-br-int"]]])
        );

        // Check second interface has peer pointing to first
        let iface2 = &txn.operations()[3];
        assert_eq!(
            iface2["row"]["options"],
            json!(["map", [["peer", "patch-br-int-to-br-ext"]]])
        );
    }

    #[test]
    fn is_empty_reflects_operations() {
        let mut txn = Transaction::new("Open_vSwitch");
        assert!(txn.is_empty());

        txn.insert("Bridge", json!({"name": "br0"}));
        assert!(!txn.is_empty());
    }

    #[test]
    fn comment_adds_comment_operation() {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.comment("Test transaction");

        assert_eq!(txn.operations()[0]["op"], "comment");
        assert_eq!(txn.operations()[0]["comment"], "Test transaction");
    }

    #[test]
    fn add_system_port_uses_system_type() {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.add_system_port("br0", "eth0");

        // Interface, Port, Mutate
        assert_eq!(txn.operations().len(), 3);

        // Interface should have type "system"
        let iface_op = &txn.operations()[0];
        assert_eq!(iface_op["row"]["type"], "system");
        assert_eq!(iface_op["row"]["name"], "eth0");
    }

    #[test]
    fn add_trunk_patch_ports_single_vlan() {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.add_trunk_patch_ports("br-int", "br-ext", &[100], None, None);

        // 2 interfaces, 2 ports, 2 mutates
        assert_eq!(txn.operations().len(), 6);

        // Check port has vlan_mode and trunks
        let port1 = &txn.operations()[1];
        assert_eq!(port1["row"]["vlan_mode"], "trunk");
        assert_eq!(port1["row"]["trunks"], 100); // Single value, no set encoding

        let port2 = &txn.operations()[4];
        assert_eq!(port2["row"]["vlan_mode"], "trunk");
        assert_eq!(port2["row"]["trunks"], 100);
    }

    #[test]
    fn add_trunk_patch_ports_multiple_vlans() {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.add_trunk_patch_ports("br-int", "br-ext", &[100, 200, 300], None, None);

        // Check port has set-encoded trunks
        let port1 = &txn.operations()[1];
        assert_eq!(port1["row"]["vlan_mode"], "trunk");
        assert_eq!(port1["row"]["trunks"], json!(["set", [100, 200, 300]]));
    }
}
