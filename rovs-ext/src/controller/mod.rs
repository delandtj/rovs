//! OpenFlow controller framework.
//!
//! Provides an event-driven controller with pluggable packet handlers.
//!
//! # Architecture
//!
//! - [`Controller`] - Main controller that manages the OpenFlow connection
//! - [`PacketHandler`] - Trait for handling Packet-In events
//! - [`Dispatcher`] - Routes events to registered handlers
//! - [`protocol`] - Pre-built handlers for ARP, NDP, etc.
//!
//! # Example
//!
//! ```ignore
//! use rovs_ext::controller::{Controller, ControllerConfig};
//! use rovs_ext::controller::protocol::{ArpProxyHandler, NdpProxyHandler};
//!
//! // Create controller
//! let config = ControllerConfig::default();
//! let mut controller = Controller::new(&addr, config).await?;
//!
//! // Register handlers
//! let mut arp_handler = ArpProxyHandler::new();
//! arp_handler.add_entry([10, 0, 0, 99], [0x02, 0x00, 0x00, 0x00, 0x00, 0x99]);
//! controller.register(arp_handler);
//!
//! // Run event loop
//! controller.run().await?;
//! ```

mod dispatcher;
mod event;
mod handler;
pub mod protocol;

pub use dispatcher::Dispatcher;
pub use event::{ControllerEvent, PacketInEvent, ParsedEthernet};
pub use handler::{HandlerAction, HandlerContext, PacketHandler};

use rovs_openflow::VConn;
use rovs_transport::Address;

use crate::Result;

/// Controller configuration.
#[derive(Debug, Clone, Default)]
pub struct ControllerConfig {
    /// Log unhandled packets.
    pub log_unhandled: bool,
}

impl ControllerConfig {
    /// Create a new controller configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable logging of unhandled packets.
    #[must_use]
    pub fn log_unhandled(mut self, log: bool) -> Self {
        self.log_unhandled = log;
        self
    }
}

/// OpenFlow controller.
///
/// Manages the OpenFlow connection and dispatches Packet-In events
/// to registered handlers.
pub struct Controller {
    conn: VConn,
    dispatcher: Dispatcher,
    config: ControllerConfig,
}

impl Controller {
    /// Create a new controller and connect to the switch.
    pub async fn new(addr: &Address, config: ControllerConfig) -> Result<Self> {
        let conn = VConn::connect(addr).await?;

        Ok(Self {
            conn,
            dispatcher: Dispatcher::new(),
            config,
        })
    }

    /// Get a reference to the VConn.
    pub fn conn(&self) -> &VConn {
        &self.conn
    }

    /// Get a mutable reference to the VConn.
    pub fn conn_mut(&mut self) -> &mut VConn {
        &mut self.conn
    }

    /// Register a packet handler.
    pub fn register<H: PacketHandler + 'static>(&mut self, handler: H) {
        self.dispatcher.register(handler);
    }

    /// Run the controller event loop.
    ///
    /// This blocks forever, processing Packet-In events and dispatching
    /// them to registered handlers.
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!(
            "Controller running, {} handlers registered",
            self.dispatcher.handler_count()
        );

        loop {
            // Wait for Packet-In
            let packet_in = self.conn.recv_packet_in().await?;
            let event = PacketInEvent::from_packet_in(packet_in);

            // Dispatch to handlers
            let action = self.dispatcher.dispatch(&event, &mut self.conn).await?;

            // Log unhandled packets if configured
            if matches!(action, HandlerAction::NotHandled) && self.config.log_unhandled {
                tracing::debug!(
                    "Unhandled packet: in_port={}, ethertype={:04x?}, len={}",
                    event.in_port,
                    event.ethertype(),
                    event.data().len()
                );
            }
        }
    }

    /// Run the controller for a single packet.
    ///
    /// Useful for testing or one-shot packet processing.
    pub async fn run_once(&mut self) -> Result<HandlerAction> {
        let packet_in = self.conn.recv_packet_in().await?;
        let event = PacketInEvent::from_packet_in(packet_in);
        self.dispatcher.dispatch(&event, &mut self.conn).await
    }
}
