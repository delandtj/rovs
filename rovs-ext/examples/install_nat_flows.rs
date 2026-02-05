//! Install NAT flows without cleanup - for inspection with ovs-ofctl.

use std::net::Ipv4Addr;
use rovs_ext::flows::{DnatConfig, DnatService, SnatConfig, SnatGateway};
use rovs_openflow::VConn;
use rovs_transport::Address;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: Address = std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6654".to_string())
        .parse()?;

    let mut conn = VConn::connect(&addr).await?;
    println!("Connected to {addr}");

    // Install SNAT gateway in tables 0-2
    let snat = SnatGateway::new(
        SnatConfig::new(Ipv4Addr::new(203, 0, 113, 1), 1, 2)
            .zone(1)
            .port_range(10000, 65000)
    );
    snat.install(&mut conn, 0, 100).await?;
    println!("Installed SNAT flows in tables 0-2");

    // Install DNAT service in tables 10-12
    let dnat = DnatService::new(
        DnatConfig::new(2, 1)
            .zone(2)
            .forward_tcp(80, Ipv4Addr::new(192, 168, 1, 10), 8080)
            .forward_tcp(443, Ipv4Addr::new(192, 168, 1, 10), 8443)
    );
    dnat.install(&mut conn, 10, 100).await?;
    println!("Installed DNAT flows in tables 10-12");

    println!("\nFlows installed. Inspect with:");
    println!("  sudo ovs-ofctl dump-flows br-nat");
    println!("\nTo clean up:");
    println!("  sudo ovs-ofctl del-flows br-nat");

    Ok(())
}
