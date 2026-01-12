//! OVSDB datum (collection) types.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::Atom;

/// A datum (collection) value in OVSDB.
///
/// Corresponds to the `<datum>` type in RFC 7047.
/// Can be either a set of atoms or a map of atom pairs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Datum {
    /// A single atomic value
    Atom(Atom),
    /// A set of atomic values
    Set(BTreeSet<OrdAtom>),
    /// A map of atomic key-value pairs
    Map(BTreeMap<OrdAtom, Atom>),
}

/// Wrapper around Atom that implements Ord for use in collections.
///
/// Note: This uses a string-based comparison for ordering, which may not be
/// semantically correct for all atom types. This is a pragmatic choice to
/// enable using atoms in ordered collections.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrdAtom(pub Atom);

impl PartialEq for OrdAtom {
    fn eq(&self, other: &Self) -> bool {
        // Compare by string representation to handle f64
        format!("{:?}", self.0) == format!("{:?}", other.0)
    }
}

impl Eq for OrdAtom {}

impl PartialOrd for OrdAtom {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrdAtom {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Compare by string representation
        format!("{:?}", self.0).cmp(&format!("{:?}", other.0))
    }
}

impl From<Atom> for Datum {
    fn from(a: Atom) -> Self {
        Self::Atom(a)
    }
}
