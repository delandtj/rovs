//! Test NAT action encoding
//!
//! Tests the `ct()` action with NAT (SNAT/DNAT) nested action.
//!
//! Run with:
//! ```sh
//! OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example test_nat
//! ```

#![allow(clippy::similar_names)]

use std::net::Ipv4Addr;

use rovs_openflow::{ActionList, CT_COMMIT, Flow, Match, NatConfig, VConn};
use rovs_transport::Address;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: Address = std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6653".to_string())
        .parse()?;
    let mut conn = VConn::connect(&addr).await?;
    println!("Connected!");

    // Clear test tables
    conn.send_flow_sync(&Flow::delete().table(10)).await.ok();
    conn.send_flow_sync(&Flow::delete().table(11)).await.ok();

    // Test 1: SNAT with single IP
    println!("\nTest 1: SNAT to single IP (10.0.0.1)...");
    let flow1 = Flow::add()
        .table(10)
        .priority(100)
        .match_fields(Match::new().eth_type(0x0800).in_port(1))
        .actions(ActionList::new().ct_snat(1, Some(11), Ipv4Addr::new(10, 0, 0, 1)));

    match conn.send_flow_sync(&flow1).await {
        Ok(()) => println!("  OK!"),
        Err(e) => println!("  Error: {e:?}"),
    }

    // Test 2: DNAT with single IP
    println!("\nTest 2: DNAT to single IP (192.168.1.100)...");
    let flow2 = Flow::add()
        .table(10)
        .priority(99)
        .match_fields(Match::new().eth_type(0x0800).in_port(2))
        .actions(ActionList::new().ct_dnat(1, Some(11), Ipv4Addr::new(192, 168, 1, 100)));

    match conn.send_flow_sync(&flow2).await {
        Ok(()) => println!("  OK!"),
        Err(e) => println!("  Error: {e:?}"),
    }

    // Test 3: SNAT with IP range
    println!("\nTest 3: SNAT with IP range (10.0.0.1-10.0.0.10)...");
    let nat_config = NatConfig::snat_range(Ipv4Addr::new(10, 0, 0, 1), Ipv4Addr::new(10, 0, 0, 10));
    let flow3 = Flow::add()
        .table(10)
        .priority(98)
        .match_fields(Match::new().eth_type(0x0800).in_port(3))
        .actions(ActionList::new().ct_nat(CT_COMMIT, 1, Some(11), nat_config));

    match conn.send_flow_sync(&flow3).await {
        Ok(()) => println!("  OK!"),
        Err(e) => println!("  Error: {e:?}"),
    }

    // Test 4: SNAT with port range
    println!("\nTest 4: SNAT with port range (10.0.0.1:5000-6000)...");
    let nat_config = NatConfig::snat(Ipv4Addr::new(10, 0, 0, 1))
        .port_range(5000, 6000)
        .random();
    let flow4 = Flow::add()
        .table(10)
        .priority(97)
        .match_fields(Match::new().eth_type(0x0800).in_port(4))
        .actions(ActionList::new().ct_nat(CT_COMMIT, 1, Some(11), nat_config));

    match conn.send_flow_sync(&flow4).await {
        Ok(()) => println!("  OK!"),
        Err(e) => println!("  Error: {e:?}"),
    }

    // Test 5: DNAT with specific port
    println!("\nTest 5: DNAT to 192.168.1.100:8080...");
    let nat_config = NatConfig::dnat(Ipv4Addr::new(192, 168, 1, 100)).port(8080);
    let flow5 = Flow::add()
        .table(10)
        .priority(96)
        .match_fields(Match::new().eth_type(0x0800).in_port(5))
        .actions(ActionList::new().ct_nat(CT_COMMIT, 1, Some(11), nat_config));

    match conn.send_flow_sync(&flow5).await {
        Ok(()) => println!("  OK!"),
        Err(e) => println!("  Error: {e:?}"),
    }

    // Add output flow in table 11
    let output_flow = Flow::add()
        .table(11)
        .priority(100)
        .actions(ActionList::new().normal());
    conn.send_flow_sync(&output_flow).await?;

    // Dump flows to verify
    println!("\nVerifying installed flows (table 10):");
    let flows = conn.dump_flows().await?;
    for f in flows.iter().filter(|f| f.table_id == 10) {
        println!(
            "  priority={} in_port={:?}",
            f.priority, f.match_fields.in_port
        );
    }

    // Clean up
    println!("\nCleaning up...");
    conn.send_flow_sync(&Flow::delete().table(10)).await.ok();
    conn.send_flow_sync(&Flow::delete().table(11)).await.ok();

    println!("\n=== NAT Test Complete ===");

    Ok(())
}
