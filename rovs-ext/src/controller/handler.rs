//! Packet handler trait and context.

use std::future::Future;
use std::pin::Pin;

use rovs_openflow::{ActionList, PacketOut, VConn};

use crate::Result;

use super::event::PacketInEvent;

/// Boxed future type for async handler methods.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Action to take after handling a packet.
#[derive(Debug, Clone)]
pub enum HandlerAction {
    /// Packet was handled, no further processing needed.
    Handled,
    /// Packet was not handled, try next handler.
    NotHandled,
    /// Send packet out with the specified actions.
    SendPacketOut(PacketOut),
    /// Drop the packet.
    Drop,
}

/// Context for packet handlers.
///
/// Provides access to the VConn for sending packets and other
/// controller operations.
pub struct HandlerContext<'a> {
    /// The VConn for sending packets.
    conn: &'a mut VConn,
}

impl<'a> HandlerContext<'a> {
    /// Create a new handler context.
    pub fn new(conn: &'a mut VConn) -> Self {
        Self { conn }
    }

    /// Get a mutable reference to the VConn.
    pub fn conn(&mut self) -> &mut VConn {
        self.conn
    }

    /// Send a packet out to the switch.
    pub async fn send_packet_out(&mut self, packet_out: &PacketOut) -> Result<()> {
        self.conn.send_packet_out(packet_out).await?;
        Ok(())
    }

    /// Send a packet out with the given actions.
    pub async fn send_packet(
        &mut self,
        in_port: u32,
        data: Vec<u8>,
        actions: ActionList,
    ) -> Result<()> {
        let packet_out = PacketOut::new()
            .in_port(in_port)
            .actions(actions)
            .data(data);
        self.send_packet_out(&packet_out).await
    }

    /// Send a packet back to the input port.
    pub async fn send_to_in_port(&mut self, in_port: u32, data: Vec<u8>) -> Result<()> {
        self.send_packet(in_port, data, ActionList::new().in_port())
            .await
    }

    /// Send a packet to a specific port.
    pub async fn send_to_port(&mut self, in_port: u32, out_port: u32, data: Vec<u8>) -> Result<()> {
        self.send_packet(in_port, data, ActionList::new().output(out_port))
            .await
    }
}

/// Trait for packet handlers.
///
/// Handlers are called for each Packet-In event in the order they are
/// registered. The first handler that returns `HandlerAction::Handled`
/// or `HandlerAction::SendPacketOut` stops further processing.
///
/// # Example
///
/// ```ignore
/// struct MyHandler;
///
/// impl PacketHandler for MyHandler {
///     fn can_handle(&self, event: &PacketInEvent) -> bool {
///         event.is_arp()
///     }
///
///     fn handle<'a>(
///         &'a self,
///         event: &'a PacketInEvent,
///         ctx: &'a mut HandlerContext<'_>,
///     ) -> BoxFuture<'a, Result<HandlerAction>> {
///         Box::pin(async move {
///             // Handle ARP packet
///             Ok(HandlerAction::Handled)
///         })
///     }
/// }
/// ```
pub trait PacketHandler: Send + Sync {
    /// Check if this handler can handle the given event.
    ///
    /// This is called before `handle` to quickly filter events.
    /// Return `true` if this handler should attempt to handle the event.
    fn can_handle(&self, event: &PacketInEvent) -> bool;

    /// Handle a Packet-In event.
    ///
    /// Returns an action indicating what to do next.
    ///
    /// Use `Box::pin(async move { ... })` to implement this method.
    fn handle<'a>(
        &'a self,
        event: &'a PacketInEvent,
        ctx: &'a mut HandlerContext<'_>,
    ) -> BoxFuture<'a, Result<HandlerAction>>;
}
