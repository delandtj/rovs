//! High-level OVS client.

use rovs_openflow::VConn;
use rovs_ovsdb::{Client, Transaction};
use rovs_transport::Address;

use crate::{Bridge, Error, Flow, Port, Result};

/// High-level OVS client.
///
/// Provides a unified interface for OVSDB and OpenFlow operations.
pub struct OvsClient {
    /// OVSDB client
    ovsdb: Client,
    /// OpenFlow address (for on-demand connections)
    openflow_addr: Address,
}

impl OvsClient {
    /// Connect to an OVS instance.
    ///
    /// # Arguments
    ///
    /// * `ovsdb` - OVSDB connection string (e.g., "unix:/var/run/openvswitch/db.sock")
    /// * `openflow` - OpenFlow connection string (e.g., "tcp:127.0.0.1:6653")
    pub async fn connect(ovsdb: &str, openflow: &str) -> Result<Self> {
        let openflow_addr: Address = openflow.parse()?;
        let ovsdb = Client::connect(ovsdb).await?;

        Ok(Self {
            ovsdb,
            openflow_addr,
        })
    }

    /// Get the OpenFlow address.
    pub fn openflow_address(&self) -> &Address {
        &self.openflow_addr
    }

    /// Get the OVSDB client.
    pub fn ovsdb(&self) -> &Client {
        &self.ovsdb
    }

    /// Get a mutable reference to the OVSDB client.
    pub fn ovsdb_mut(&mut self) -> &mut Client {
        &mut self.ovsdb
    }

    /// List all bridges.
    pub async fn list_bridges(&self) -> Result<Vec<Bridge>> {
        let bridges: Vec<Bridge> = self
            .ovsdb
            .idl()
            .rows("Bridge")
            .map(|row| Bridge {
                uuid: row.uuid,
                name: row.get_string("name").unwrap_or_default().to_owned(),
                datapath_id: row.get_string("datapath_id").map(|s| s.to_owned()),
                datapath_type: row
                    .get_string("datapath_type")
                    .unwrap_or_default()
                    .to_owned(),
                ports: Vec::new(),
                fail_mode: row.get_string("fail_mode").map(|s| s.to_owned()),
                stp_enable: row.get_bool("stp_enable").unwrap_or(false),
                controller: Vec::new(),
            })
            .collect();

        Ok(bridges)
    }

    /// Get a bridge by name.
    pub async fn get_bridge(&self, name: &str) -> Result<Bridge> {
        let bridges = self.list_bridges().await?;
        bridges
            .into_iter()
            .find(|b| b.name == name)
            .ok_or_else(|| Error::BridgeNotFound(name.to_owned()))
    }

    /// Create a new bridge.
    pub async fn create_bridge(&mut self, name: &str) -> Result<Bridge> {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.create_bridge(name);
        self.ovsdb.commit(&mut txn).await?;

        // Fetch the created bridge from IDL
        self.get_bridge(name).await
    }

    /// Delete a bridge.
    pub async fn delete_bridge(&mut self, name: &str) -> Result<()> {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.delete_bridge(name);
        self.ovsdb.commit(&mut txn).await?;
        Ok(())
    }

    /// Add an internal port to a bridge.
    pub async fn add_port(&mut self, bridge: &str, port: &str) -> Result<Port> {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.add_internal_port(bridge, port);
        self.ovsdb.commit(&mut txn).await?;

        // Return a basic Port struct
        Ok(Port::new(port))
    }

    /// Delete a port from a bridge.
    pub async fn delete_port(&mut self, bridge: &str, port: &str) -> Result<()> {
        let mut txn = Transaction::new("Open_vSwitch");
        txn.delete_port(bridge, port);
        self.ovsdb.commit(&mut txn).await?;
        Ok(())
    }

    /// Add a flow to a bridge.
    ///
    /// Note: This creates a new OpenFlow connection for each call. For bulk
    /// operations, use `rovs_openflow::VConn` directly.
    pub async fn add_flow(&mut self, _bridge: &str, flow: Flow) -> Result<()> {
        let mut conn = VConn::connect(&self.openflow_addr).await?;
        conn.send_flow_sync(&flow).await?;
        Ok(())
    }

    /// Delete flows from a bridge.
    ///
    /// Note: This creates a new OpenFlow connection for each call.
    pub async fn delete_flows(&mut self, _bridge: &str, flow: Flow) -> Result<()> {
        let mut conn = VConn::connect(&self.openflow_addr).await?;
        conn.send_flow_sync(&flow).await?;
        Ok(())
    }

    /// Dump all flows from a bridge.
    ///
    /// Note: This creates a new OpenFlow connection for each call.
    pub async fn dump_flows(
        &mut self,
        _bridge: &str,
    ) -> Result<Vec<rovs_openflow::FlowStatsEntry>> {
        let mut conn = VConn::connect(&self.openflow_addr).await?;
        let flows = conn.dump_flows().await?;
        Ok(flows)
    }
}
