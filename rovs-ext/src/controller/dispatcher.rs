//! Packet handler dispatcher.

use rovs_openflow::VConn;

use crate::Result;

use super::event::PacketInEvent;
use super::handler::{HandlerAction, HandlerContext, PacketHandler};

/// Dispatcher for packet handlers.
///
/// Maintains a list of handlers and dispatches Packet-In events to them
/// in registration order.
pub struct Dispatcher {
    handlers: Vec<Box<dyn PacketHandler>>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    /// Create a new empty dispatcher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Register a packet handler.
    ///
    /// Handlers are called in registration order. The first handler that
    /// returns `HandlerAction::Handled` or `HandlerAction::SendPacketOut`
    /// stops further processing.
    pub fn register<H: PacketHandler + 'static>(&mut self, handler: H) {
        self.handlers.push(Box::new(handler));
    }

    /// Get the number of registered handlers.
    #[must_use]
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// Dispatch a Packet-In event to registered handlers.
    ///
    /// Calls handlers in order until one handles the packet. If a handler
    /// returns `SendPacketOut`, the packet is sent before returning.
    ///
    /// Returns `HandlerAction::NotHandled` if no handler processed the packet.
    pub async fn dispatch(&self, event: &PacketInEvent, conn: &mut VConn) -> Result<HandlerAction> {
        let mut ctx = HandlerContext::new(conn);

        for handler in &self.handlers {
            if !handler.can_handle(event) {
                continue;
            }

            match handler.handle(event, &mut ctx).await? {
                HandlerAction::NotHandled => {}
                HandlerAction::SendPacketOut(packet_out) => {
                    ctx.send_packet_out(&packet_out).await?;
                    return Ok(HandlerAction::Handled);
                }
                action => return Ok(action),
            }
        }

        Ok(HandlerAction::NotHandled)
    }
}
