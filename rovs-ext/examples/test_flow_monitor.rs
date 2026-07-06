//! Flow monitor example — live flow change notifications.
//!
//! Demonstrates the Nicira Flow Monitor (NXST_FLOW_MONITOR) extension.
//! Opens two OpenFlow connections: one monitors flow changes, the other
//! adds/modifies/deletes flows. The monitor connection prints events as
//! they arrive.
//!
//! # Usage
//!
//! ```bash
//! # Start OVS with OpenFlow support
//! ./scripts/test-with-ovs.sh start full
//!
//! # Run the example
//! OPENFLOW_ADDR=tcp:127.0.0.1:6653 cargo run -p rovs-ext --example test_flow_monitor
//! ```

use rovs_openflow::{
    ActionList, Flow, FlowMonitorRequest, FlowUpdate, FlowUpdateEvent, Match, VConn,
};
use rovs_transport::Address;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: Address = std::env::var("OPENFLOW_ADDR")
        .unwrap_or_else(|_| "tcp:127.0.0.1:6653".to_string())
        .parse()?;

    // Connection 1: monitor
    let mut mon = VConn::connect(&addr).await?;
    println!("Monitor connection established (OF {:?})", mon.version());

    // Connection 2: flow operations
    let mut ops = VConn::connect(&addr).await?;
    println!("Operations connection established");

    // Register flow monitor for all changes
    let request = FlowMonitorRequest::all_changes(1);
    let initial = mon.monitor_flows(request).await?;
    println!("\n--- Initial snapshot: {} flows ---", initial.len());
    for update in &initial {
        print_update(update);
    }

    // Install a test flow
    println!("\n--- Installing test flow (table=0, priority=200, tcp_dst=8080) ---");
    let flow = Flow::add()
        .table(0)
        .priority(200)
        .cookie(0xCAFE)
        .match_fields(Match::new().eth_type(0x0800).ip_proto(6).tcp_dst(8080))
        .actions(ActionList::new().output(1));
    ops.send_flow_sync(&flow).await?;

    // Receive the update
    let updates = mon.recv_flow_updates().await?;
    println!("\n--- Received {} update(s) ---", updates.len());
    for update in &updates {
        print_update(update);
    }

    // Modify the flow (change priority via delete + re-add)
    println!("\n--- Modifying flow (priority 200 -> 300) ---");
    let del = Flow::delete()
        .table(0)
        .cookie(0xCAFE)
        .match_fields(Match::new().eth_type(0x0800).ip_proto(6).tcp_dst(8080));
    ops.send_flow_sync(&del).await?;

    let flow2 = Flow::add()
        .table(0)
        .priority(300)
        .cookie(0xCAFE)
        .match_fields(Match::new().eth_type(0x0800).ip_proto(6).tcp_dst(8080))
        .actions(ActionList::new().output(2));
    ops.send_flow_sync(&flow2).await?;

    // Receive delete + add updates
    for _ in 0..2 {
        let updates = mon.recv_flow_updates().await?;
        println!("\n--- Received {} update(s) ---", updates.len());
        for update in &updates {
            print_update(update);
        }
    }

    // Clean up
    println!("\n--- Deleting test flow ---");
    let del = Flow::delete().table(0).cookie(0xCAFE);
    ops.send_flow_sync(&del).await?;

    let updates = mon.recv_flow_updates().await?;
    println!("\n--- Received {} update(s) ---", updates.len());
    for update in &updates {
        print_update(update);
    }

    println!("\nDone!");
    Ok(())
}

fn print_update(update: &FlowUpdate) {
    match update {
        FlowUpdate::Full(f) => {
            let event = match f.event {
                FlowUpdateEvent::Added => "ADDED",
                FlowUpdateEvent::Deleted => "DELETED",
                FlowUpdateEvent::Modified => "MODIFIED",
            };
            println!(
                "  {event}: table={} priority={} cookie=0x{:x} match={} actions={} bytes",
                f.table_id,
                f.priority,
                f.cookie,
                f.match_fields,
                f.actions.len(),
            );
        }
        FlowUpdate::Abbrev { xid } => {
            println!("  ABBREV: xid={xid}");
        }
    }
}
