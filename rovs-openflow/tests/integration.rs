//! Integration tests for rovs-openflow.
//!
//! These tests require a running OVS with ovs-vswitchd and OpenFlow enabled.
//! Set the `OPENFLOW_ADDR` environment variable to the OpenFlow address
//! (e.g., `tcp:127.0.0.1:6653`).
//!
//! Run these tests with:
//! ```bash
//! ./scripts/test-with-ovs.sh openflow
//! ```
//!
//! Or manually:
//! ```bash
//! podman run --rm -d --privileged -p 6640:6640 -p 6653:6653 --name rovs-ovsdb rovs-ovsdb
//! OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo test -p rovs-openflow -- --ignored
//! ```

use rovs_openflow::{ActionList, Flow, Match, VConn};
use rovs_transport::Address;

fn get_openflow_addr() -> Option<Address> {
    std::env::var("OPENFLOW_ADDR")
        .ok()
        .and_then(|s| s.parse().ok())
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn connect_and_handshake() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Verify we negotiated a version
    let version = conn.version();
    println!("Negotiated OpenFlow version: {:?}", version);
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn echo_request() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Send echo request
    conn.echo().await.expect("Echo failed");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn barrier_request() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Send barrier request
    conn.barrier().await.expect("Barrier failed");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn add_simple_flow() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Create a simple flow: in_port=1 actions=output:2
    let flow = Flow::add()
        .priority(100)
        .match_fields(Match::new().in_port(1))
        .actions(ActionList::new().output(2));

    // Send flow and wait for confirmation
    conn.send_flow_sync(&flow)
        .await
        .expect("Failed to add flow");

    // Clean up - delete the flow
    let delete_flow = Flow::delete()
        .priority(100)
        .match_fields(Match::new().in_port(1));

    conn.send_flow_sync(&delete_flow)
        .await
        .expect("Failed to delete flow");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn add_flow_with_ip_match() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Create flow: ip,nw_dst=10.0.0.0/24 actions=output:1
    let flow = Flow::add()
        .priority(200)
        .match_fields(
            Match::new()
                .eth_type(0x0800) // IPv4
                .ipv4_dst("10.0.0.0".parse().unwrap(), 24),
        )
        .actions(ActionList::new().output(1));

    conn.send_flow_sync(&flow)
        .await
        .expect("Failed to add flow");

    // Clean up
    let delete_flow = Flow::delete()
        .priority(200)
        .match_fields(
            Match::new()
                .eth_type(0x0800)
                .ipv4_dst("10.0.0.0".parse().unwrap(), 24),
        );

    conn.send_flow_sync(&delete_flow)
        .await
        .expect("Failed to delete flow");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn add_flow_with_tcp_match() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Create flow: tcp,tp_dst=80 actions=output:1
    let flow = Flow::add()
        .priority(300)
        .match_fields(
            Match::new()
                .eth_type(0x0800) // IPv4
                .ip_proto(6)      // TCP
                .tcp_dst(80),
        )
        .actions(ActionList::new().output(1));

    conn.send_flow_sync(&flow)
        .await
        .expect("Failed to add flow");

    // Clean up
    let delete_flow = Flow::delete()
        .priority(300)
        .match_fields(
            Match::new()
                .eth_type(0x0800)
                .ip_proto(6)
                .tcp_dst(80),
        );

    conn.send_flow_sync(&delete_flow)
        .await
        .expect("Failed to delete flow");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn delete_flow_by_match() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Add a flow
    let flow = Flow::add()
        .priority(150)
        .match_fields(Match::new().in_port(3))
        .actions(ActionList::new().output(4));

    conn.send_flow_sync(&flow)
        .await
        .expect("Failed to add flow");

    // Delete the flow using Delete (not DeleteStrict)
    let delete_flow = Flow::delete()
        .match_fields(Match::new().in_port(3));

    conn.send_flow_sync(&delete_flow)
        .await
        .expect("Failed to delete flow");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn add_flow_with_vlan() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Create flow with VLAN match and actions
    let flow = Flow::add()
        .priority(250)
        .match_fields(
            Match::new()
                .in_port(1)
                .vlan_vid(100), // VLAN 100
        )
        .actions(
            ActionList::new()
                .pop_vlan()
                .output(2),
        );

    conn.send_flow_sync(&flow)
        .await
        .expect("Failed to add flow");

    // Clean up
    let delete_flow = Flow::delete()
        .priority(250)
        .match_fields(
            Match::new()
                .in_port(1)
                .vlan_vid(100),
        );

    conn.send_flow_sync(&delete_flow)
        .await
        .expect("Failed to delete flow");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn add_flow_with_set_field() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Create flow that sets destination MAC
    let flow = Flow::add()
        .priority(350)
        .match_fields(Match::new().in_port(1))
        .actions(
            ActionList::new()
                .set_eth_dst([0x00, 0x11, 0x22, 0x33, 0x44, 0x55])
                .output(2),
        );

    conn.send_flow_sync(&flow)
        .await
        .expect("Failed to add flow");

    // Clean up
    let delete_flow = Flow::delete()
        .priority(350)
        .match_fields(Match::new().in_port(1));

    conn.send_flow_sync(&delete_flow)
        .await
        .expect("Failed to delete flow");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn add_flow_with_dec_ttl() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Create flow that decrements TTL
    let flow = Flow::add()
        .priority(400)
        .match_fields(
            Match::new()
                .in_port(1)
                .eth_type(0x0800), // IPv4
        )
        .actions(
            ActionList::new()
                .dec_ttl()
                .output(2),
        );

    conn.send_flow_sync(&flow)
        .await
        .expect("Failed to add flow");

    // Clean up
    let delete_flow = Flow::delete()
        .priority(400)
        .match_fields(
            Match::new()
                .in_port(1)
                .eth_type(0x0800),
        );

    conn.send_flow_sync(&delete_flow)
        .await
        .expect("Failed to delete flow");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn add_flow_with_timeout() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Create flow with idle and hard timeout
    let flow = Flow::add()
        .priority(175)
        .idle_timeout(60)  // 60 seconds
        .hard_timeout(300) // 5 minutes
        .match_fields(Match::new().in_port(1))
        .actions(ActionList::new().output(2));

    conn.send_flow_sync(&flow)
        .await
        .expect("Failed to add flow");

    // Clean up
    let delete_flow = Flow::delete()
        .priority(175)
        .match_fields(Match::new().in_port(1));

    conn.send_flow_sync(&delete_flow)
        .await
        .expect("Failed to delete flow");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn add_flow_to_specific_table() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Create flow in table 1
    let flow = Flow::add()
        .table(1)
        .priority(100)
        .match_fields(Match::new().in_port(1))
        .actions(ActionList::new().output(2));

    conn.send_flow_sync(&flow)
        .await
        .expect("Failed to add flow to table 1");

    // Clean up
    let delete_flow = Flow::delete()
        .table(1)
        .priority(100)
        .match_fields(Match::new().in_port(1));

    conn.send_flow_sync(&delete_flow)
        .await
        .expect("Failed to delete flow");
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn multiple_flows_sequential() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Add multiple flows
    for port in 1..=5u32 {
        let flow = Flow::add()
            .priority(100 + port as u16)
            .match_fields(Match::new().in_port(port))
            .actions(ActionList::new().output(port + 10));

        conn.send_flow_sync(&flow)
            .await
            .unwrap_or_else(|_| panic!("Failed to add flow for port {}", port));
    }

    // Delete all flows
    for port in 1..=5u32 {
        let delete_flow = Flow::delete()
            .priority(100 + port as u16)
            .match_fields(Match::new().in_port(port));

        conn.send_flow_sync(&delete_flow)
            .await
            .unwrap_or_else(|_| panic!("Failed to delete flow for port {}", port));
    }
}

#[tokio::test]
#[ignore = "requires ovs-vswitchd"]
async fn delete_all_flows_in_table() {
    let addr = get_openflow_addr().expect("OPENFLOW_ADDR not set");
    let mut conn = VConn::connect(&addr).await.expect("Failed to connect");

    // Add some flows to table 2
    for i in 1..=3u32 {
        let flow = Flow::add()
            .table(2)
            .priority(100 + i as u16)
            .match_fields(Match::new().in_port(i))
            .actions(ActionList::new().output(i + 10));

        conn.send_flow_sync(&flow)
            .await
            .expect("Failed to add flow");
    }

    // Delete all flows in table 2
    let delete_all = Flow::delete()
        .table(2);

    conn.send_flow_sync(&delete_all)
        .await
        .expect("Failed to delete all flows in table");
}
