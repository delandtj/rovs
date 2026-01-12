//! Reconnection state machine.
//!
//! Ported from python/ovs/reconnect.py

use std::time::{Duration, Instant};

/// Reconnection state machine.
///
/// Manages backoff and retry logic for connections.
#[derive(Debug)]
pub struct Reconnect {
    state: State,
    backoff: Duration,
    max_backoff: Duration,
    last_activity: Option<Instant>,
    last_connected: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Reconnecting variant reserved for future use
enum State {
    /// Not connected, not trying
    Void,
    /// Waiting before attempting connection
    Backoff,
    /// Connection attempt in progress
    Connecting,
    /// Currently connected
    Active,
    /// Connected but idle, probing
    Idle,
    /// Reconnecting after failure
    Reconnecting,
}

impl Default for Reconnect {
    fn default() -> Self {
        Self::new()
    }
}

impl Reconnect {
    /// Create a new reconnection state machine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: State::Void,
            backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(8),
            last_activity: None,
            last_connected: None,
        }
    }

    /// Set the maximum backoff duration.
    pub fn set_max_backoff(&mut self, max: Duration) {
        self.max_backoff = max;
    }

    /// Signal that a connection attempt is starting.
    pub fn connecting(&mut self) {
        self.state = State::Connecting;
    }

    /// Signal that the connection succeeded.
    pub fn connected(&mut self) {
        self.state = State::Active;
        self.last_connected = Some(Instant::now());
        self.last_activity = Some(Instant::now());
        self.backoff = Duration::from_secs(1);
    }

    /// Signal that the connection failed or was lost.
    pub fn disconnected(&mut self) {
        self.state = State::Backoff;
        self.last_activity = Some(Instant::now());
    }

    /// Signal activity on the connection.
    pub fn activity(&mut self) {
        self.last_activity = Some(Instant::now());
        if self.state == State::Idle {
            self.state = State::Active;
        }
    }

    /// Check if we should attempt to connect now.
    #[must_use]
    pub fn should_connect(&self) -> bool {
        match self.state {
            State::Void => true,
            State::Backoff => {
                if let Some(last) = self.last_activity {
                    last.elapsed() >= self.backoff
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    /// Check if we are currently connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self.state, State::Active | State::Idle)
    }

    /// Get the current backoff duration.
    #[must_use]
    pub fn current_backoff(&self) -> Duration {
        self.backoff
    }

    /// Increase the backoff (called after failed connection attempt).
    pub fn increase_backoff(&mut self) {
        self.backoff = (self.backoff * 2).min(self.max_backoff);
    }
}
