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
