//! Shared OVSDB client handle with lazy connection and automatic reconnection.
//!
//! Provides [`OvsdbHandle`], a thread-safe wrapper around [`rovs_ovsdb::Client`]
//! that connects lazily on first use and reconnects if the connection is lost.
//!
//! # Example
//!
//! ```ignore
//! use rovs_ext::OvsdbHandle;
//!
//! let handle = OvsdbHandle::local();
//!
//! // First call connects, subsequent calls reuse the connection.
//! let mut client = handle.client().await?;
//! let bridge_exists = client.idl().rows("Bridge").any(|r| r.get_string("name") == Some("br0"));
//! ```

use rovs_ovsdb::Client;
use tokio::sync::Mutex;

/// A shared, lazily-connected OVSDB client handle.
///
/// Connects to the OVSDB server on first use and reuses the connection
/// for all subsequent operations. Thread-safe via [`tokio::sync::Mutex`].
///
/// If the connection drops (detected via [`Client::is_connected`]), the next
/// call to [`client()`](Self::client) will transparently reconnect.
pub struct OvsdbHandle {
    client: Mutex<Option<Client>>,
    addr: String,
}

impl OvsdbHandle {
    /// Create a new handle targeting the given OVSDB address.
    ///
    /// No connection is made until [`client()`](Self::client) is called.
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            client: Mutex::new(None),
            addr: addr.into(),
        }
    }

    /// Create a handle for the local OVS daemon socket.
    ///
    /// Equivalent to `OvsdbHandle::new("unix:/var/run/openvswitch/db.sock")`.
    pub fn local() -> Self {
        Self::new("unix:/var/run/openvswitch/db.sock")
    }

    /// Get the OVSDB server address.
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Acquire the shared client, connecting lazily on first use.
    ///
    /// Returns a guard that derefs to [`Client`]. Reconnects automatically
    /// if the previous connection was lost.
    pub async fn client(&self) -> crate::Result<OvsdbGuard<'_>> {
        let mut guard = self.client.lock().await;

        let needs_connect = match guard.as_ref() {
            None => true,
            Some(c) => !c.is_connected(),
        };

        if needs_connect {
            tracing::debug!(addr = %self.addr, "connecting to OVSDB");
            let client = Client::connect(&self.addr).await?;
            *guard = Some(client);
            tracing::debug!(addr = %self.addr, "OVSDB connection established");
        }

        Ok(OvsdbGuard { guard })
    }

    /// Force a disconnect. The next call to [`client()`](Self::client) will reconnect.
    pub async fn disconnect(&self) {
        let mut guard = self.client.lock().await;
        *guard = None;
    }
}

impl std::fmt::Debug for OvsdbHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OvsdbHandle")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

/// RAII guard providing mutable access to the OVSDB [`Client`].
///
/// Created by [`OvsdbHandle::client()`]. The underlying mutex is released
/// when the guard is dropped, so avoid holding it across `.await` points
/// where possible.
pub struct OvsdbGuard<'a> {
    guard: tokio::sync::MutexGuard<'a, Option<Client>>,
}

impl std::ops::Deref for OvsdbGuard<'_> {
    type Target = Client;

    fn deref(&self) -> &Client {
        self.guard
            .as_ref()
            .expect("OvsdbGuard always holds a connected client")
    }
}

impl std::ops::DerefMut for OvsdbGuard<'_> {
    fn deref_mut(&mut self) -> &mut Client {
        self.guard
            .as_mut()
            .expect("OvsdbGuard always holds a connected client")
    }
}
