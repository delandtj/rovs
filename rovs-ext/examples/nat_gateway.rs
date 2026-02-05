//! Example: NAT Gateway with SNAT and DNAT
//!
//! Demonstrates using the high-level NAT flow templates:
//! - `SnatGateway` for masquerading outbound traffic
//! - `DnatService` for port forwarding to internal servers
//!
//! Network topology:
//! ```text
//!   Internal Network          NAT Gateway           Internet
//!   192.168.1.0/24    <-->   [OVS Bridge]   <-->   External
//!      port 1                                        port 2
//! ```
//!
//! Run with:
//! ```sh
//! ./scripts/test-with-ovs.sh start full
//! OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example nat_gateway
//! ```

use std::net::Ipv4Addr;

use rovs_ext::flows::{DnatConfig, SnatConfig, SnatGateway, DnatService};
use rovs_openflow::VConn;
use rovs_transport::Address;

fn get_openflow_addr() -> Address {
    std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6653".to_string())
        .parse()
        .expect("Invalid OPENFLOW_ADDR")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = get_openflow_addr();
    println!("Connecting to OpenFlow at {addr}...");

    let mut conn = VConn::connect(&addr).await?;
    println!("Connected! OpenFlow version: {:?}\n", conn.version());

    // =========================================================================
    // Example 1: SNAT Gateway (Masquerade)
    // =========================================================================
    println!("=== Example 1: SNAT Gateway ===");
    println!("Outbound traffic from port 1 will be SNATed to 203.0.113.1\n");

    let snat_config = SnatConfig::new(
        Ipv4Addr::new(203, 0, 113, 1), // External IP
        1,                              // Internal port
        2,                              // External port
    )
    .zone(1)
    .port_range(10000, 65000)
    .random();

    let snat_gateway = SnatGateway::new(snat_config);

    // Install SNAT flows in tables 0, 1, 2
    println!("Installing SNAT gateway flows (tables 0-2)...");
    snat_gateway.install(&mut conn, 0, 100).await?;
    println!("  Installed {} flows", snat_gateway.all_flows(0, 100).len());

    // Show the flow pipeline
    println!("\nSNAT Flow Pipeline:");
    println!("  Table 0: Connection tracking (ct action)");
    println!("  Table 1: Policy - SNAT new outbound, forward established");
    println!("  Table 2: Output after NAT commit");

    // Clean up example 1
    println!("\nCleaning up SNAT flows...");
    snat_gateway.delete(&mut conn, 0).await?;

    // =========================================================================
    // Example 2: DNAT Service (Port Forwarding)
    // =========================================================================
    println!("\n=== Example 2: DNAT Service ===");
    println!("Port forwarding rules:");
    println!("  - TCP 80  -> 192.168.1.10:8080 (HTTP)");
    println!("  - TCP 443 -> 192.168.1.10:8443 (HTTPS)");
    println!("  - TCP 22  -> 192.168.1.20:22   (SSH)\n");

    let dnat_config = DnatConfig::new(2, 1) // external port 2, internal port 1
        .zone(2)
        .forward_tcp(80, Ipv4Addr::new(192, 168, 1, 10), 8080)
        .forward_tcp(443, Ipv4Addr::new(192, 168, 1, 10), 8443)
        .forward_tcp(22, Ipv4Addr::new(192, 168, 1, 20), 22);

    let dnat_service = DnatService::new(dnat_config);

    // Install DNAT flows in tables 10, 11, 12
    println!("Installing DNAT service flows (tables 10-12)...");
    dnat_service.install(&mut conn, 10, 100).await?;
    println!("  Installed {} flows", dnat_service.all_flows(10, 100).len());

    // Show the flow pipeline
    println!("\nDNAT Flow Pipeline:");
    println!("  Table 10: Connection tracking");
    println!("  Table 11: Policy - DNAT matching ports, forward established");
    println!("  Table 12: Output after NAT commit");

    // Verify flows were installed
    println!("\nInstalled flows in table 11 (policy):");
    let flows = conn.dump_flows().await?;
    for f in flows.iter().filter(|f| f.table_id == 11) {
        let tcp_dst = f.match_fields.tcp_dst.map_or("*".to_string(), |p| p.to_string());
        println!("  priority={} tcp_dst={}", f.priority, tcp_dst);
    }

    // Clean up example 2
    println!("\nCleaning up DNAT flows...");
    conn.send_flow_sync(&rovs_openflow::Flow::delete().table(10)).await?;
    conn.send_flow_sync(&rovs_openflow::Flow::delete().table(11)).await?;
    conn.send_flow_sync(&rovs_openflow::Flow::delete().table(12)).await?;

    // =========================================================================
    // Summary
    // =========================================================================
    println!("\n=== NAT Gateway Example Complete ===");
    println!("\nKey concepts:");
    println!("  - SnatGateway: Masquerade outbound traffic with configurable IP/port ranges");
    println!("  - DnatService: Port forwarding with multiple rules");
    println!("  - Both use 3-table pipeline: ct -> policy -> output");
    println!("  - Connection tracking handles reply traffic automatically");

    Ok(())
}
