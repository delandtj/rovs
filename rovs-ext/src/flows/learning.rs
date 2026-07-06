//! MAC learning switch flow templates.
//!
//! Provides flow builders for implementing a basic MAC learning switch
//! using the NxLearn action.

use rovs_openflow::{ActionList, Flow, NxLearn, VConn, nxm};

use crate::Result;

/// Configuration for learning switch flows.
#[derive(Debug, Clone)]
pub struct LearningConfig {
    /// Table for learning flows (receives packets, learns, goes to forward table).
    pub learn_table: u8,
    /// Table for forwarding flows (learned entries and flood rule).
    pub forward_table: u8,
    /// Idle timeout for learned entries (seconds, 0 = no timeout).
    pub idle_timeout: u16,
    /// Priority for learned entries.
    pub learned_priority: u16,
    /// Priority for the flood rule (should be lower than learned entries).
    pub flood_priority: u16,
    /// Priority for the learning flow.
    pub learn_priority: u16,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            learn_table: 0,
            forward_table: 1,
            idle_timeout: 300, // 5 minutes
            learned_priority: 100,
            flood_priority: 1,
            learn_priority: 100,
        }
    }
}

impl LearningConfig {
    /// Create a new learning switch configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the learning table.
    #[must_use]
    pub fn learn_table(mut self, table: u8) -> Self {
        self.learn_table = table;
        self
    }

    /// Set the forwarding table.
    #[must_use]
    pub fn forward_table(mut self, table: u8) -> Self {
        self.forward_table = table;
        self
    }

    /// Set the idle timeout for learned entries.
    #[must_use]
    pub fn idle_timeout(mut self, timeout: u16) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Set all tables to use the same table (single-table mode).
    ///
    /// In single-table mode, learned flows and the flood rule are
    /// in the same table as the learning flow. This requires careful
    /// priority management.
    #[must_use]
    pub fn single_table(mut self, table: u8) -> Self {
        self.learn_table = table;
        self.forward_table = table;
        self
    }
}

/// MAC learning switch flow builder.
///
/// Creates flows for implementing a basic MAC learning switch:
///
/// 1. **Learning flow** (table 0 by default):
///    - Matches all packets
///    - Learns src_mac -> in_port mapping
///    - Goes to forwarding table
///
/// 2. **Flood flow** (table 1 by default):
///    - Low priority catch-all
///    - Floods unknown destinations
///
/// 3. **Learned flows** (table 1, created by NxLearn):
///    - Match on dst_mac
///    - Output to learned port
///
/// # Example
///
/// ```ignore
/// use rovs_ext::flows::LearningSwitchFlows;
///
/// let flows = LearningSwitchFlows::new(LearningConfig::default());
/// flows.install(&mut conn).await?;
/// ```
#[derive(Debug, Clone)]
pub struct LearningSwitchFlows {
    config: LearningConfig,
}

impl LearningSwitchFlows {
    /// Create a new learning switch flow builder.
    #[must_use]
    pub fn new(config: LearningConfig) -> Self {
        Self { config }
    }

    /// Create the MAC learning flow.
    ///
    /// This flow:
    /// 1. Matches all packets on the learning table
    /// 2. Creates a learned entry in the forward table mapping
    ///    the packet's source MAC to its input port
    /// 3. Goes to the forwarding table for output decision
    #[must_use]
    pub fn learning_flow(&self) -> Flow {
        let learn = NxLearn::new()
            .table(self.config.forward_table)
            .idle_timeout(self.config.idle_timeout)
            .priority(self.config.learned_priority)
            // Match on dst_mac = current src_mac
            .match_field(nxm::ETH_DST, nxm::ETH_SRC, 48)
            // Output to current in_port
            .output_field(nxm::IN_PORT, 16);

        Flow::add()
            .table(self.config.learn_table)
            .priority(self.config.learn_priority)
            .actions(
                ActionList::new()
                    .learn(learn)
                    .goto_table(self.config.forward_table),
            )
    }

    /// Create the flood flow for unknown destinations.
    ///
    /// This flow has low priority and matches any packet that wasn't
    /// matched by a learned entry, flooding it to all ports.
    #[must_use]
    pub fn flood_flow(&self) -> Flow {
        Flow::add()
            .table(self.config.forward_table)
            .priority(self.config.flood_priority)
            .actions(ActionList::new().flood())
    }

    /// Get all learning switch flows.
    #[must_use]
    pub fn all_flows(&self) -> Vec<Flow> {
        vec![self.learning_flow(), self.flood_flow()]
    }

    /// Install learning switch flows to the switch.
    pub async fn install(&self, conn: &mut VConn) -> Result<()> {
        for flow in self.all_flows() {
            conn.send_flow_sync(&flow).await?;
        }
        Ok(())
    }

    /// Delete learning switch flows from the switch.
    ///
    /// This deletes the learning and flood flows, as well as any
    /// learned entries in the forward table.
    pub async fn delete(&self, conn: &mut VConn) -> Result<()> {
        // Delete learning flow
        let delete_learn = Flow::delete().table(self.config.learn_table);
        conn.send_flow_sync(&delete_learn).await?;

        // Delete all flows in forward table (including learned entries)
        let delete_forward = Flow::delete().table(self.config.forward_table);
        conn.send_flow_sync(&delete_forward).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = LearningConfig::default();
        assert_eq!(config.learn_table, 0);
        assert_eq!(config.forward_table, 1);
        assert_eq!(config.idle_timeout, 300);
    }

    #[test]
    fn single_table_mode() {
        let config = LearningConfig::new().single_table(5);
        assert_eq!(config.learn_table, 5);
        assert_eq!(config.forward_table, 5);
    }

    #[test]
    fn all_flows_returns_two_flows() {
        let flows = LearningSwitchFlows::new(LearningConfig::default());
        assert_eq!(flows.all_flows().len(), 2);
    }
}
