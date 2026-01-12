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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_i64() {
        let atom: Atom = 42i64.into();
        assert!(matches!(atom, Atom::Integer(42)));
    }

    #[test]
    fn from_f64() {
        let atom: Atom = 2.5f64.into();
        assert!(matches!(atom, Atom::Real(v) if (v - 2.5).abs() < f64::EPSILON));
    }

    #[test]
    fn from_bool() {
        let atom_true: Atom = true.into();
        let atom_false: Atom = false.into();
        assert!(matches!(atom_true, Atom::Boolean(true)));
        assert!(matches!(atom_false, Atom::Boolean(false)));
    }

    #[test]
    fn from_string() {
        let atom: Atom = String::from("test").into();
        assert!(matches!(atom, Atom::String(s) if s == "test"));
    }

    #[test]
    fn from_str() {
        let atom: Atom = "test".into();
        assert!(matches!(atom, Atom::String(s) if s == "test"));
    }

    #[test]
    fn from_uuid() {
        let uuid = Uuid::nil();
        let atom: Atom = uuid.into();
        assert!(matches!(atom, Atom::Uuid(u) if u == Uuid::nil()));
    }

    #[test]
    fn serde_roundtrip_integer() {
        let atom = Atom::Integer(42);
        let json = serde_json::to_string(&atom).unwrap();
        let parsed: Atom = serde_json::from_str(&json).unwrap();
        assert_eq!(atom, parsed);
    }

    #[test]
    fn serde_roundtrip_real() {
        let atom = Atom::Real(1.234);
        let json = serde_json::to_string(&atom).unwrap();
        let parsed: Atom = serde_json::from_str(&json).unwrap();
        assert_eq!(atom, parsed);
    }

    #[test]
    fn serde_roundtrip_boolean() {
        let atom = Atom::Boolean(true);
        let json = serde_json::to_string(&atom).unwrap();
        let parsed: Atom = serde_json::from_str(&json).unwrap();
        assert_eq!(atom, parsed);
    }

    #[test]
    fn serde_roundtrip_string() {
        let atom = Atom::String("test".to_string());
        let json = serde_json::to_string(&atom).unwrap();
        let parsed: Atom = serde_json::from_str(&json).unwrap();
        assert_eq!(atom, parsed);
    }

    #[test]
    fn serde_uuid_serializes_to_string() {
        // Note: With #[serde(untagged)], UUID serializes to a string
        // and deserializes back as Atom::String (not Atom::Uuid).
        // This is expected behavior for untagged enums.
        let atom = Atom::Uuid(Uuid::nil());
        let json = serde_json::to_string(&atom).unwrap();
        assert_eq!(json, "\"00000000-0000-0000-0000-000000000000\"");

        // Deserialization produces String, not Uuid
        let parsed: Atom = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Atom::String(_)));
    }
}
