//! Install NAT flows without cleanup - for inspection with ovs-ofctl.
//!
//! Demonstrates dual-stack (IPv4 + IPv6) NAT configuration.

use rovs_ext::flows::{DnatConfig, DnatService, SnatConfig, SnatGateway};
use rovs_openflow::VConn;
use rovs_transport::Address;
use std::net::{Ipv4Addr, Ipv6Addr};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: Address = std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6654".to_string())
        .parse()?;

    let mut conn = VConn::connect(&addr).await?;
    println!("Connected to {addr}");

    // Install dual-stack SNAT gateway in tables 0-2
    let snat = SnatGateway::new(
        SnatConfig::dual_stack(
            Ipv4Addr::new(203, 0, 113, 1),
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
            1, // internal port
            2, // external port
        )
        .zone(1)
        .port_range(10000, 65000),
    );
    snat.install(&mut conn, 0, 100).await?;
    println!("Installed dual-stack SNAT flows in tables 0-2");
    println!("  IPv4: 203.0.113.1");
    println!("  IPv6: 2001:db8::1");

    // Install dual-stack DNAT service in tables 10-12
    let dnat = DnatService::new(
        DnatConfig::new(2, 1)
            .zone(2)
            // IPv4 rules
            .forward_tcp(80, Ipv4Addr::new(192, 168, 1, 10), 8080)
            .forward_tcp(443, Ipv4Addr::new(192, 168, 1, 10), 8443)
            // IPv6 rules
            .forward_tcp_v6(80, Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 10), 8080)
            .forward_tcp_v6(443, Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 10), 8443),
    );
    dnat.install(&mut conn, 10, 100).await?;
    println!("\nInstalled dual-stack DNAT flows in tables 10-12");
    println!("  IPv4: TCP 80,443 -> 192.168.1.10");
    println!("  IPv6: TCP 80,443 -> 2001:db8::10");

    println!("\nFlows installed. Inspect with:");
    println!("  sudo ovs-ofctl dump-flows br-nat");
    println!("\nTo clean up:");
    println!("  sudo ovs-ofctl del-flows br-nat");

    Ok(())
}
