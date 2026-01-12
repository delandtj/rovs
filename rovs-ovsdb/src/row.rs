//! OVSDB row representation.

use std::collections::HashMap;

use serde_json::Value;
use uuid::Uuid;

/// A row in an OVSDB table.
#[derive(Debug, Clone)]
pub struct Row {
    /// The row's UUID
    pub uuid: Uuid,
    /// Column values
    pub columns: HashMap<String, Value>,
    /// Whether this row has been modified locally
    pub modified: bool,
}

impl Row {
    /// Create a new row with the given UUID.
    pub fn new(uuid: Uuid) -> Self {
        Self {
            uuid,
            columns: HashMap::new(),
            modified: false,
        }
    }

    /// Get a column value.
    pub fn get(&self, column: &str) -> Option<&Value> {
        self.columns.get(column)
    }

    /// Get a column value as a string.
    pub fn get_string(&self, column: &str) -> Option<&str> {
        self.get(column).and_then(|v| v.as_str())
    }

    /// Get a column value as an integer.
    pub fn get_i64(&self, column: &str) -> Option<i64> {
        self.get(column).and_then(|v| v.as_i64())
    }

    /// Get a column value as a boolean.
    pub fn get_bool(&self, column: &str) -> Option<bool> {
        self.get(column).and_then(|v| v.as_bool())
    }

    /// Set a column value.
    pub fn set(&mut self, column: impl Into<String>, value: Value) {
        self.columns.insert(column.into(), value);
        self.modified = true;
    }

    /// Update columns from a JSON object.
    pub fn update(&mut self, values: &HashMap<String, Value>) {
        for (k, v) in values {
            self.columns.insert(k.clone(), v.clone());
        }
        self.modified = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_row_has_uuid() {
        let uuid = Uuid::new_v4();
        let row = Row::new(uuid);
        assert_eq!(row.uuid, uuid);
        assert!(!row.modified);
        assert!(row.columns.is_empty());
    }

    #[test]
    fn get_string() {
        let uuid = Uuid::new_v4();
        let mut row = Row::new(uuid);
        row.columns.insert("name".to_string(), json!("br0"));

        assert_eq!(row.get_string("name"), Some("br0"));
        assert_eq!(row.get_string("missing"), None);

        // Non-string value returns None
        row.columns.insert("tag".to_string(), json!(100));
        assert_eq!(row.get_string("tag"), None);
    }

    #[test]
    fn get_i64() {
        let uuid = Uuid::new_v4();
        let mut row = Row::new(uuid);
        row.columns.insert("tag".to_string(), json!(100));
        row.columns.insert("ofport".to_string(), json!(-1));

        assert_eq!(row.get_i64("tag"), Some(100));
        assert_eq!(row.get_i64("ofport"), Some(-1));
        assert_eq!(row.get_i64("missing"), None);
    }

    #[test]
    fn get_bool() {
        let uuid = Uuid::new_v4();
        let mut row = Row::new(uuid);
        row.columns.insert("enabled".to_string(), json!(true));
        row.columns.insert("disabled".to_string(), json!(false));

        assert_eq!(row.get_bool("enabled"), Some(true));
        assert_eq!(row.get_bool("disabled"), Some(false));
        assert_eq!(row.get_bool("missing"), None);
    }

    #[test]
    fn set_marks_modified() {
        let uuid = Uuid::new_v4();
        let mut row = Row::new(uuid);
        assert!(!row.modified);

        row.set("name", json!("br0"));
        assert!(row.modified);
        assert_eq!(row.get_string("name"), Some("br0"));
    }

    #[test]
    fn update_merges_columns() {
        let uuid = Uuid::new_v4();
        let mut row = Row::new(uuid);
        row.columns.insert("name".to_string(), json!("br0"));
        row.columns
            .insert("fail_mode".to_string(), json!("standalone"));
        row.modified = false;

        let mut updates = HashMap::new();
        updates.insert("fail_mode".to_string(), json!("secure"));
        updates.insert("new_field".to_string(), json!("value"));

        row.update(&updates);

        assert!(row.modified);
        assert_eq!(row.get_string("name"), Some("br0")); // Unchanged
        assert_eq!(row.get_string("fail_mode"), Some("secure")); // Updated
        assert_eq!(row.get_string("new_field"), Some("value")); // Added
    }

    #[test]
    fn get_returns_raw_value() {
        let uuid = Uuid::new_v4();
        let mut row = Row::new(uuid);
        row.columns
            .insert("complex".to_string(), json!({"nested": "value"}));

        let val = row.get("complex");
        assert!(val.is_some());
        assert_eq!(val.unwrap()["nested"], "value");
    }
}
