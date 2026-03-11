//! OVS unixctl client (`ovs-appctl` equivalent).
//!
//! Connects directly to the `ovs-vswitchd` management socket to execute
//! administrative commands. This is the same protocol that `ovs-appctl` uses,
//! but without shelling out.
//!
//! # Example
//!
//! ```ignore
//! use rovs_ext::appctl::AppCtl;
//!
//! let mut ctl = AppCtl::connect("/var/run/openvswitch/ovs-vswitchd.123.ctl").await?;
//!
//! // Dump datapath flows
//! let flows = ctl.dpif_dump_flows("br0").await?;
//! for flow in &flows {
//!     println!("{flow}");
//! }
//!
//! // Dump conntrack entries
//! let entries = ctl.dump_conntrack(Some(1)).await?;
//! for entry in &entries {
//!     println!("{entry}");
//! }
//! ```

mod conntrack;
mod dpif;

pub use conntrack::ConntrackEntry;
pub use dpif::DpifFlow;

use std::path::{Path, PathBuf};

use rovs_jsonrpc::Connection;
use rovs_transport::{Address, Stream};
use serde_json::Value;

use crate::{Error, Result};

/// Client for the OVS unixctl protocol.
///
/// Connects to the `ovs-vswitchd` management socket and issues commands
/// using JSON-RPC 1.0 — the same protocol as `ovs-appctl`.
pub struct AppCtl {
    conn: Connection,
}

impl AppCtl {
    /// Connect to a specific unixctl socket path.
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self> {
        let addr = Address::Unix(path.as_ref().to_path_buf());
        let stream = Stream::connect(&addr).await?;
        Ok(Self {
            conn: Connection::new(stream),
        })
    }

    /// Discover and connect to the default `ovs-vswitchd` socket.
    ///
    /// Searches for `/var/run/openvswitch/ovs-vswitchd.*.ctl` and connects
    /// to the first match.
    pub async fn connect_default() -> Result<Self> {
        let path = discover_vswitchd_socket()?;
        Self::connect(&path).await
    }

    // --- dpif commands (must have) ---

    /// Dump datapath flows for a bridge.
    ///
    /// Equivalent to `ovs-appctl dpif/dump-flows <bridge>`.
    pub async fn dpif_dump_flows(&mut self, bridge: &str) -> Result<Vec<DpifFlow>> {
        let output = self.transact("dpif/dump-flows", &[bridge]).await?;
        Ok(dpif::parse_dpif_flows(&output))
    }

    /// Dump datapath flows with wildcard mask information.
    ///
    /// Equivalent to `ovs-appctl dpif/dump-flows -m <bridge>`.
    pub async fn dpif_dump_flows_verbose(&mut self, bridge: &str) -> Result<Vec<DpifFlow>> {
        let output = self.transact("dpif/dump-flows", &["-m", bridge]).await?;
        Ok(dpif::parse_dpif_flows(&output))
    }

    /// Show datapaths with port info and statistics.
    ///
    /// Equivalent to `ovs-appctl dpif/show`.
    /// Returns the raw output since the format is already human-readable.
    pub async fn dpif_show(&mut self) -> Result<String> {
        self.transact("dpif/show", &[]).await
    }

    // --- conntrack commands (should have) ---

    /// Dump connection tracking entries, optionally filtered by zone.
    ///
    /// Equivalent to `ovs-appctl dpctl/dump-conntrack [zone=<zone>]`.
    pub async fn dump_conntrack(&mut self, zone: Option<u16>) -> Result<Vec<ConntrackEntry>> {
        let output = match zone {
            Some(z) => {
                let zone_arg = format!("zone={z}");
                self.transact("dpctl/dump-conntrack", &[&zone_arg]).await?
            }
            None => self.transact("dpctl/dump-conntrack", &[]).await?,
        };
        Ok(conntrack::parse_conntrack_entries(&output))
    }

    /// Show conntrack statistics grouped by protocol.
    ///
    /// Equivalent to `ovs-appctl dpctl/ct-stats-show [zone=<zone>]`.
    /// Returns the raw output since it's already a readable summary.
    pub async fn ct_stats(&mut self, zone: Option<u16>) -> Result<String> {
        match zone {
            Some(z) => {
                let zone_arg = format!("zone={z}");
                self.transact("dpctl/ct-stats-show", &[&zone_arg]).await
            }
            None => self.transact("dpctl/ct-stats-show", &[]).await,
        }
    }

    /// Flush connection tracking entries, optionally filtered by zone.
    ///
    /// Equivalent to `ovs-appctl dpctl/flush-conntrack [zone=<zone>]`.
    pub async fn flush_conntrack(&mut self, zone: Option<u16>) -> Result<()> {
        match zone {
            Some(z) => {
                let zone_arg = format!("zone={z}");
                self.transact("dpctl/flush-conntrack", &[&zone_arg]).await?;
            }
            None => {
                self.transact("dpctl/flush-conntrack", &[]).await?;
            }
        }
        Ok(())
    }

    /// Send a command and return the result string.
    async fn transact(&mut self, method: &str, args: &[&str]) -> Result<String> {
        let params: Vec<Value> = args.iter().map(|s| Value::String((*s).to_owned())).collect();
        let result = self
            .conn
            .transact(method, Value::Array(params))
            .await
            .map_err(|e| Error::AppCtl(e.to_string()))?;

        match result {
            Value::String(s) => Ok(s),
            Value::Null => Ok(String::new()),
            other => Ok(other.to_string()),
        }
    }
}

/// Search for the default ovs-vswitchd unixctl socket.
fn discover_vswitchd_socket() -> Result<PathBuf> {
    let run_dir = Path::new("/var/run/openvswitch");
    if !run_dir.is_dir() {
        return Err(Error::AppCtl(format!(
            "{} does not exist or is not a directory",
            run_dir.display()
        )));
    }

    let pattern = "ovs-vswitchd.*.ctl";
    for entry in std::fs::read_dir(run_dir).map_err(|e| Error::AppCtl(e.to_string()))? {
        let entry = entry.map_err(|e| Error::AppCtl(e.to_string()))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("ovs-vswitchd.") && name_str.ends_with(".ctl") {
            return Ok(entry.path());
        }
    }

    Err(Error::AppCtl(format!(
        "no {pattern} found in {}",
        run_dir.display()
    )))
}
