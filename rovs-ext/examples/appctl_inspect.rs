//! Example: Inspect OVS datapath and conntrack state via `AppCtl`
//!
//! Demonstrates using the unixctl client to inspect switch internals:
//! - `dpif/show`: Datapath overview with ports
//! - `dpif/dump-flows`: Datapath flow table with stats
//! - `dpctl/dump-conntrack`: Connection tracking entries
//! - `dpctl/ct-stats-show`: Conntrack statistics
//! - `dpctl/flush-conntrack`: Clear conntrack state
//!
//! This is the Rust equivalent of running `ovs-appctl` commands,
//! but connects directly to the vswitchd unixctl socket.
//!
//! Run with:
//! ```sh
//! # Start OVS container with full mode (vswitchd needed for appctl)
//! ./scripts/test-with-ovs.sh start full
//!
//! # Run the example
//! cargo run -p rovs-ext --example appctl_inspect
//!
//! # Or specify a socket path explicitly
//! VSWITCHD_SOCKET=/var/run/openvswitch/ovs-vswitchd.123.ctl \
//!     cargo run -p rovs-ext --example appctl_inspect
//! ```

use clap::Parser;
use rovs_ext::appctl::AppCtl;

#[derive(Parser)]
#[command(about = "Inspect OVS datapath and conntrack state")]
struct Args {
    /// Bridge name to inspect
    #[arg(short, long, default_value = "br-test")]
    bridge: String,

    /// Only show conntrack for this zone
    #[arg(short, long)]
    zone: Option<u16>,

    /// Include wildcard masks in flow dump
    #[arg(short = 'm', long)]
    verbose_flows: bool,

    /// Flush conntrack entries before inspecting
    #[arg(long)]
    flush: bool,

    /// vswitchd unixctl socket path (auto-discovered if not set)
    #[arg(long, env = "VSWITCHD_SOCKET")]
    socket: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rovs=debug".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    // Connect to vswitchd
    let mut ctl = if let Some(path) = &args.socket {
        println!("Connecting to {path}...");
        AppCtl::connect(path).await?
    } else {
        println!("Auto-discovering vswitchd socket...");
        AppCtl::connect_default().await?
    };

    println!("Connected to ovs-vswitchd\n");

    // =========================================================================
    // dpif/show — Datapath overview
    // =========================================================================

    println!("=== Datapath Overview (dpif/show) ===\n");
    let dpif_info = ctl.dpif_show().await?;
    println!("{dpif_info}");

    // =========================================================================
    // dpif/dump-flows — Datapath flow table
    // =========================================================================

    println!("=== Datapath Flows ({}) ===\n", args.bridge);

    let flows = if args.verbose_flows {
        ctl.dpif_dump_flows_verbose(&args.bridge).await?
    } else {
        ctl.dpif_dump_flows(&args.bridge).await?
    };

    if flows.is_empty() {
        println!("  (no datapath flows)");
    } else {
        println!("  {:<6} FLOW", "INDEX");
        println!("  {:<6} ----", "-----");
        for (i, flow) in flows.iter().enumerate() {
            println!("  {:<6} {flow}", i + 1);
        }
        println!();

        // Summary stats
        let total_packets: u64 = flows.iter().map(|f| f.packets).sum();
        let total_bytes: u64 = flows.iter().map(|f| f.bytes).sum();
        let active = flows.iter().filter(|f| f.used.is_some()).count();
        println!(
            "  Summary: {} flows ({} active), {} total packets, {} total bytes",
            flows.len(),
            active,
            total_packets,
            total_bytes
        );
    }

    // =========================================================================
    // Flush conntrack (optional)
    // =========================================================================

    if args.flush {
        println!("\n=== Flushing Conntrack ===\n");
        ctl.flush_conntrack(args.zone).await?;
        match args.zone {
            Some(z) => println!("  Flushed conntrack entries in zone {z}"),
            None => println!("  Flushed all conntrack entries"),
        }
    }

    // =========================================================================
    // dpctl/ct-stats-show — Conntrack statistics
    // =========================================================================

    println!("\n=== Conntrack Statistics ===\n");
    let ct_stats = ctl.ct_stats(args.zone).await?;
    if ct_stats.trim().is_empty() {
        println!("  (no conntrack entries)");
    } else {
        for line in ct_stats.lines() {
            println!("  {line}");
        }
    }

    // =========================================================================
    // dpctl/dump-conntrack — Connection tracking entries
    // =========================================================================

    println!("\n=== Conntrack Entries ===\n");

    let entries = ctl.dump_conntrack(args.zone).await?;

    if entries.is_empty() {
        println!("  (no conntrack entries)");
    } else {
        for entry in &entries {
            println!("  {entry}");
        }
        println!();

        // Group by protocol
        let mut by_proto: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for entry in &entries {
            *by_proto.entry(&entry.protocol).or_default() += 1;
        }
        print!("  Summary: {} entries (", entries.len());
        let protos: Vec<_> = by_proto.iter().map(|(k, v)| format!("{v} {k}")).collect();
        print!("{}", protos.join(", "));
        println!(")");
    }

    Ok(())
}
