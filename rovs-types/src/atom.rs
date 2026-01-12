//! OVSDB atomic values.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An atomic value in OVSDB.
///
/// Corresponds to the `<atom>` type in RFC 7047.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Atom {
    /// Integer value (i64)
    Integer(i64),
    /// Real/floating point value
    Real(f64),
    /// Boolean value
    Boolean(bool),
    /// String value
    String(String),
    /// UUID value
    Uuid(Uuid),
}

impl From<i64> for Atom {
    fn from(v: i64) -> Self {
        Self::Integer(v)
    }
}

impl From<f64> for Atom {
    fn from(v: f64) -> Self {
        Self::Real(v)
    }
}

impl From<bool> for Atom {
    fn from(v: bool) -> Self {
        Self::Boolean(v)
    }
}

impl From<String> for Atom {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for Atom {
    fn from(v: &str) -> Self {
        Self::String(v.to_owned())
    }
}

impl From<Uuid> for Atom {
    fn from(v: Uuid) -> Self {
        Self::Uuid(v)
    }
}
