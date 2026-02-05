//! Debug test for ct_state matching
//!
//! Key finding: When using ct() action with commit flag, OVS requires
//! eth_type to be specified in the match. This is because connection
//! tracking operates at the network layer (IP).
//!
//! Without eth_type: BadAction error code 10 (OFPBAC_MATCH_INCONSISTENT)
//! With eth_type: Success

use rovs_openflow::oxm::ct_state;
use rovs_openflow::{ActionList, Flow, Match, VConn, CT_COMMIT};
use rovs_transport::Address;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: Address = std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6653".to_string())
        .parse()?;
    let mut conn = VConn::connect(&addr).await?;
    println!("Connected!");

    // Clear tables
    conn.send_flow_sync(&Flow::delete().table(1)).await.ok();
    conn.send_flow_sync(&Flow::delete().table(2)).await.ok();

    // Test 1: ct_state match + ct(commit) WITHOUT eth_type - should FAIL
    println!("\nTest 1: ct_state match + ct(commit) WITHOUT eth_type...");
    let flow_no_ethtype = Flow::add()
        .table(1)
        .priority(80)
        .match_fields(
            Match::new()
                .in_port(1)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, 1, Some(2)));

    match conn.send_flow_sync(&flow_no_ethtype).await {
        Ok(_) => println!("  OK (unexpected!)"),
        Err(e) => println!("  Expected error: BadAction code 10 (MATCH_INCONSISTENT)\n  Got: {e:?}"),
    }

    // Test 2: ct_state match + ct(commit) WITH eth_type=0x0800 - should SUCCEED
    println!("\nTest 2: ct_state match + ct(commit) WITH eth_type=IPv4...");
    let flow_with_ethtype = Flow::add()
        .table(1)
        .priority(81)
        .match_fields(
            Match::new()
                .in_port(1)
                .eth_type(0x0800) // IPv4 - required for ct(commit)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, 1, Some(2)));

    match conn.send_flow_sync(&flow_with_ethtype).await {
        Ok(_) => println!("  OK!"),
        Err(e) => println!("  Error (unexpected): {e:?}"),
    }

    // Test 3: ct_state match + ct(commit) WITH eth_type=0x86dd - should SUCCEED
    println!("\nTest 3: ct_state match + ct(commit) WITH eth_type=IPv6...");
    let flow_ipv6 = Flow::add()
        .table(1)
        .priority(82)
        .match_fields(
            Match::new()
                .in_port(1)
                .eth_type(0x86dd) // IPv6 - required for ct(commit)
                .ct_state_masked(ct_state::TRK | ct_state::NEW, ct_state::TRK | ct_state::NEW),
        )
        .actions(ActionList::new().ct(CT_COMMIT, 1, Some(2)));

    match conn.send_flow_sync(&flow_ipv6).await {
        Ok(_) => println!("  OK!"),
        Err(e) => println!("  Error (unexpected): {e:?}"),
    }

    // Clean up
    println!("\nCleaning up...");
    conn.send_flow_sync(&Flow::delete().table(1)).await.ok();
    conn.send_flow_sync(&Flow::delete().table(2)).await.ok();

    println!("\n=== Conclusion ===");
    println!("When using ct() action with commit flag, OVS requires eth_type");
    println!("(IPv4=0x0800 or IPv6=0x86dd) to be specified in the match.");
    println!("This is because connection tracking operates at the network layer.");

    Ok(())
}
