//! Bridge pair topology builder.
//!
//! Creates two interconnected bridges with optional VLAN configuration.

use rovs_ovsdb::{Client, Transaction};

use crate::Result;

/// Configuration for a bridge pair.
#[derive(Debug, Clone)]
pub struct BridgePairConfig {
    /// First bridge name.
    pub bridge1: String,
    /// Second bridge name.
    pub bridge2: String,
    /// Optional patch port name for bridge1 (default: patch-{bridge1}-to-{bridge2}).
    pub patch1_name: Option<String>,
    /// Optional patch port name for bridge2 (default: patch-{bridge2}-to-{bridge1}).
    pub patch2_name: Option<String>,
    /// Optional VLAN trunk configuration.
    pub vlans: Option<Vec<u16>>,
    /// Set fail_mode to secure on both bridges.
    pub secure_fail_mode: bool,
}

impl BridgePairConfig {
    /// Create a new bridge pair configuration.
    #[must_use]
    pub fn new(bridge1: impl Into<String>, bridge2: impl Into<String>) -> Self {
        Self {
            bridge1: bridge1.into(),
            bridge2: bridge2.into(),
            patch1_name: None,
            patch2_name: None,
            vlans: None,
            secure_fail_mode: false,
        }
    }
}

/// Bridge pair topology builder.
///
/// Creates two OVS bridges connected by patch ports. This is useful for:
///
/// - Separating traffic domains
/// - Creating internal/external bridge pairs
/// - VLAN trunk connections between bridges
///
/// # Example
///
/// ```ignore
/// use rovs_ext::topology::BridgePair;
///
/// // Create a bridge pair
/// let pair = BridgePair::new("br-int", "br-ext");
///
/// // Build the OVSDB transaction
/// let txn = pair.build_transaction();
///
/// // Or create directly on a client
/// pair.create(&mut client).await?;
/// ```
#[derive(Debug, Clone)]
pub struct BridgePair {
    config: BridgePairConfig,
}

impl BridgePair {
    /// Create a new bridge pair builder.
    #[must_use]
    pub fn new(bridge1: impl Into<String>, bridge2: impl Into<String>) -> Self {
        Self {
            config: BridgePairConfig::new(bridge1, bridge2),
        }
    }

    /// Set custom patch port names.
    #[must_use]
    pub fn patch_names(mut self, patch1: impl Into<String>, patch2: impl Into<String>) -> Self {
        self.config.patch1_name = Some(patch1.into());
        self.config.patch2_name = Some(patch2.into());
        self
    }

    /// Configure VLAN trunk on the patch ports.
    #[must_use]
    pub fn vlans(mut self, vlans: Vec<u16>) -> Self {
        self.config.vlans = Some(vlans);
        self
    }

    /// Set fail_mode to secure on both bridges.
    #[must_use]
    pub fn secure_fail_mode(mut self) -> Self {
        self.config.secure_fail_mode = true;
        self
    }

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &BridgePairConfig {
        &self.config
    }

    /// Build an OVSDB transaction to create the bridge pair.
    ///
    /// The transaction creates:
    /// 1. Bridge 1 with its internal port
    /// 2. Bridge 2 with its internal port
    /// 3. Patch ports connecting the two bridges
    #[must_use]
    pub fn build_transaction(&self) -> Transaction {
        let mut txn = Transaction::new("Open_vSwitch");

        // Create both bridges
        txn.create_bridge(&self.config.bridge1);
        txn.create_bridge(&self.config.bridge2);

        // Add patch ports connecting them
        if let Some(ref vlans) = self.config.vlans {
            txn.add_trunk_patch_ports(
                &self.config.bridge1,
                &self.config.bridge2,
                vlans,
                self.config.patch1_name.as_deref(),
                self.config.patch2_name.as_deref(),
            );
        } else {
            txn.add_patch_ports(
                &self.config.bridge1,
                &self.config.bridge2,
                self.config.patch1_name.as_deref(),
                self.config.patch2_name.as_deref(),
            );
        }

        // Set fail_mode if requested
        if self.config.secure_fail_mode {
            txn.update_by_name(
                "Bridge",
                &self.config.bridge1,
                serde_json::json!({"fail_mode": "secure"}),
            );
            txn.update_by_name(
                "Bridge",
                &self.config.bridge2,
                serde_json::json!({"fail_mode": "secure"}),
            );
        }

        txn
    }

    /// Create the bridge pair on an OVSDB client.
    pub async fn create(&self, client: &mut Client) -> Result<()> {
        let mut txn = self.build_transaction();
        client.commit(&mut txn).await?;
        Ok(())
    }

    /// Build an OVSDB transaction to delete the bridge pair.
    ///
    /// Note: This uses simple name-based deletion which may not work
    /// reliably if there are other references. For reliable deletion,
    /// use the IDL to look up UUIDs first.
    #[must_use]
    pub fn build_delete_transaction(&self) -> Transaction {
        let mut txn = Transaction::new("Open_vSwitch");

        // Delete patch ports first
        let patch1 = self.config.patch1_name.clone().unwrap_or_else(|| {
            format!("patch-{}-to-{}", self.config.bridge1, self.config.bridge2)
        });
        let patch2 = self.config.patch2_name.clone().unwrap_or_else(|| {
            format!("patch-{}-to-{}", self.config.bridge2, self.config.bridge1)
        });

        txn.delete_port(&self.config.bridge1, &patch1);
        txn.delete_port(&self.config.bridge2, &patch2);

        // Delete bridges
        txn.delete_bridge(&self.config.bridge1);
        txn.delete_bridge(&self.config.bridge2);

        txn
    }

    /// Delete the bridge pair from an OVSDB client.
    pub async fn delete(&self, client: &mut Client) -> Result<()> {
        let mut txn = self.build_delete_transaction();
        client.commit(&mut txn).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_transaction_creates_operations() {
        let pair = BridgePair::new("br-int", "br-ext");
        let txn = pair.build_transaction();

        // Should have operations for:
        // - 2 bridges (3 ops each: interface, port, bridge, mutate Open_vSwitch) = 8 ops
        // - 2 patch ports (3 ops each: interface, port, mutate bridge) = 6 ops
        assert!(!txn.is_empty());
    }

    #[test]
    fn vlans_config() {
        let pair = BridgePair::new("br-int", "br-ext").vlans(vec![100, 200]);
        assert_eq!(pair.config().vlans, Some(vec![100, 200]));
    }

    #[test]
    fn custom_patch_names() {
        let pair = BridgePair::new("br-int", "br-ext")
            .patch_names("p-int", "p-ext");
        assert_eq!(pair.config().patch1_name, Some("p-int".to_owned()));
        assert_eq!(pair.config().patch2_name, Some("p-ext".to_owned()));
    }
}
