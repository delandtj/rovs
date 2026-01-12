//! OVSDB schema parsing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Result};

/// A complete OVSDB database schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbSchema {
    /// Schema name
    pub name: String,
    /// Schema version
    pub version: String,
    /// Table definitions
    pub tables: HashMap<String, TableSchema>,
}

impl DbSchema {
    /// Parse a schema from JSON.
    pub fn from_json(json: &Value) -> Result<Self> {
        serde_json::from_value(json.clone()).map_err(|e| Error::Schema(e.to_string()))
    }

    /// Get a table schema by name.
    pub fn table(&self, name: &str) -> Option<&TableSchema> {
        self.tables.get(name)
    }
}

/// Schema for a single table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    /// Column definitions
    pub columns: HashMap<String, ColumnSchema>,
    /// Maximum number of rows (None = unlimited)
    #[serde(rename = "maxRows")]
    pub max_rows: Option<u64>,
    /// Whether this table is part of the root set
    #[serde(rename = "isRoot", default)]
    pub is_root: bool,
    /// Indexes defined on this table
    #[serde(default)]
    pub indexes: Vec<Vec<String>>,
}

impl TableSchema {
    /// Get a column schema by name.
    pub fn column(&self, name: &str) -> Option<&ColumnSchema> {
        self.columns.get(name)
    }
}

/// Schema for a single column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    /// Column type definition
    #[serde(rename = "type")]
    pub col_type: ColumnType,
    /// Whether this column is mutable
    #[serde(default = "default_true")]
    pub mutable: bool,
    /// Whether this column is ephemeral
    #[serde(default)]
    pub ephemeral: bool,
}

fn default_true() -> bool {
    true
}

/// Column type definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ColumnType {
    /// Simple atomic type (string shorthand)
    Atomic(String),
    /// Complex type definition
    Complex(ComplexType),
}

/// Complex column type with key/value and constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexType {
    /// Key type (for maps) or value type (for sets/scalars)
    pub key: TypeSpec,
    /// Value type (for maps only)
    pub value: Option<TypeSpec>,
    /// Minimum number of elements
    pub min: Option<u64>,
    /// Maximum number of elements
    pub max: Option<MaxValue>,
}

/// Type specification for key or value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TypeSpec {
    /// Simple type name
    Simple(String),
    /// Type with constraints
    Constrained(ConstrainedType),
}

/// Type with additional constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstrainedType {
    /// Base type
    #[serde(rename = "type")]
    pub base_type: String,
    /// Reference table (for uuid types)
    #[serde(rename = "refTable")]
    pub ref_table: Option<String>,
    /// Reference type (strong or weak)
    #[serde(rename = "refType")]
    pub ref_type: Option<String>,
    /// Enum values
    #[serde(rename = "enum")]
    pub enum_values: Option<Value>,
    /// Minimum value
    pub min_integer: Option<i64>,
    /// Maximum value
    pub max_integer: Option<i64>,
    /// Minimum length
    pub min_length: Option<u64>,
    /// Maximum length
    pub max_length: Option<u64>,
}

/// Maximum value (can be "unlimited" or a number).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MaxValue {
    /// Unlimited
    Unlimited(String),
    /// Specific limit
    Limit(u64),
}
