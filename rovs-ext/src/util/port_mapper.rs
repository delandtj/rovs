//! Port name to OpenFlow port mapping.

use std::collections::HashMap;

use crate::{Error, Result};

/// Maps port names to OpenFlow port numbers.
///
/// This is useful for topology builders and flow templates that need
/// to resolve port names to OpenFlow port numbers from OVSDB data.
#[derive(Debug, Clone, Default)]
pub struct PortMapper {
    /// Map from port name to OpenFlow port number.
    name_to_ofport: HashMap<String, u32>,
    /// Map from OpenFlow port number to port name.
    ofport_to_name: HashMap<u32, String>,
}

impl PortMapper {
    /// Create an empty port mapper.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a port mapper from an iterator of (name, ofport) pairs.
    pub fn from_pairs(iter: impl IntoIterator<Item = (String, u32)>) -> Self {
        let mut mapper = Self::new();
        for (name, ofport) in iter {
            mapper.insert(name, ofport);
        }
        mapper
    }

    /// Insert a port name to OpenFlow port mapping.
    pub fn insert(&mut self, name: impl Into<String>, ofport: u32) {
        let name = name.into();
        self.ofport_to_name.insert(ofport, name.clone());
        self.name_to_ofport.insert(name, ofport);
    }

    /// Remove a port by name.
    pub fn remove_by_name(&mut self, name: &str) -> Option<u32> {
        if let Some(ofport) = self.name_to_ofport.remove(name) {
            self.ofport_to_name.remove(&ofport);
            Some(ofport)
        } else {
            None
        }
    }

    /// Remove a port by OpenFlow port number.
    pub fn remove_by_ofport(&mut self, ofport: u32) -> Option<String> {
        if let Some(name) = self.ofport_to_name.remove(&ofport) {
            self.name_to_ofport.remove(&name);
            Some(name)
        } else {
            None
        }
    }

    /// Get the OpenFlow port number for a port name.
    #[must_use]
    pub fn get_ofport(&self, name: &str) -> Option<u32> {
        self.name_to_ofport.get(name).copied()
    }

    /// Get the port name for an OpenFlow port number.
    #[must_use]
    pub fn get_name(&self, ofport: u32) -> Option<&str> {
        self.ofport_to_name.get(&ofport).map(|s| s.as_str())
    }

    /// Get the OpenFlow port number for a port name, or return an error.
    pub fn require_ofport(&self, name: &str) -> Result<u32> {
        self.get_ofport(name)
            .ok_or_else(|| Error::PortNotFound(name.to_owned()))
    }

    /// Get the port name for an OpenFlow port number, or return an error.
    pub fn require_name(&self, ofport: u32) -> Result<&str> {
        self.get_name(ofport)
            .ok_or_else(|| Error::PortNotFound(format!("ofport {ofport}")))
    }

    /// Check if a port name is mapped.
    #[must_use]
    pub fn contains_name(&self, name: &str) -> bool {
        self.name_to_ofport.contains_key(name)
    }

    /// Check if an OpenFlow port number is mapped.
    #[must_use]
    pub fn contains_ofport(&self, ofport: u32) -> bool {
        self.ofport_to_name.contains_key(&ofport)
    }

    /// Get the number of mapped ports.
    #[must_use]
    pub fn len(&self) -> usize {
        self.name_to_ofport.len()
    }

    /// Check if the mapper is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.name_to_ofport.is_empty()
    }

    /// Clear all mappings.
    pub fn clear(&mut self) {
        self.name_to_ofport.clear();
        self.ofport_to_name.clear();
    }

    /// Iterate over all (name, ofport) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, u32)> {
        self.name_to_ofport
            .iter()
            .map(|(name, ofport)| (name.as_str(), *ofport))
    }

    /// Get all port names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.name_to_ofport.keys().map(|s| s.as_str())
    }

    /// Get all OpenFlow port numbers.
    pub fn ofports(&self) -> impl Iterator<Item = u32> + '_ {
        self.ofport_to_name.keys().copied()
    }
}

impl FromIterator<(String, u32)> for PortMapper {
    fn from_iter<I: IntoIterator<Item = (String, u32)>>(iter: I) -> Self {
        Self::from_pairs(iter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut mapper = PortMapper::new();
        mapper.insert("eth0", 1);
        mapper.insert("eth1", 2);

        assert_eq!(mapper.get_ofport("eth0"), Some(1));
        assert_eq!(mapper.get_ofport("eth1"), Some(2));
        assert_eq!(mapper.get_ofport("eth2"), None);

        assert_eq!(mapper.get_name(1), Some("eth0"));
        assert_eq!(mapper.get_name(2), Some("eth1"));
        assert_eq!(mapper.get_name(3), None);
    }

    #[test]
    fn require_methods() {
        let mut mapper = PortMapper::new();
        mapper.insert("eth0", 1);

        assert!(mapper.require_ofport("eth0").is_ok());
        assert!(mapper.require_ofport("eth1").is_err());

        assert!(mapper.require_name(1).is_ok());
        assert!(mapper.require_name(2).is_err());
    }

    #[test]
    fn remove_by_name() {
        let mut mapper = PortMapper::new();
        mapper.insert("eth0", 1);

        assert_eq!(mapper.remove_by_name("eth0"), Some(1));
        assert!(mapper.is_empty());
    }

    #[test]
    fn remove_by_ofport() {
        let mut mapper = PortMapper::new();
        mapper.insert("eth0", 1);

        assert_eq!(mapper.remove_by_ofport(1), Some("eth0".to_owned()));
        assert!(mapper.is_empty());
    }

    #[test]
    fn from_iterator() {
        let pairs = vec![
            ("eth0".to_owned(), 1),
            ("eth1".to_owned(), 2),
        ];
        let mapper: PortMapper = pairs.into_iter().collect();

        assert_eq!(mapper.len(), 2);
        assert_eq!(mapper.get_ofport("eth0"), Some(1));
        assert_eq!(mapper.get_ofport("eth1"), Some(2));
    }
}
