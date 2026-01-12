//! JSON-RPC 1.0 connection handling for OVSDB.
//!
//! This module implements JSON-RPC communication with OVSDB servers.
//!
//! # Important Implementation Detail
//!
//! OVSDB servers do **not** send newlines after JSON responses. Instead, this
//! implementation uses brace-depth tracking to detect complete JSON objects.
//! The parser tracks `{` and `}` characters (respecting string contents and escapes)
//! to determine when a complete message has been received.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};

use rovs_transport::Stream;

use crate::{Error, Message, Request, Response, Result, RpcId};

/// A JSON-RPC connection over a transport stream.
///
/// Handles request/response matching and buffers server notifications
/// received while waiting for responses.
pub struct Connection {
    reader: tokio::io::ReadHalf<Stream>,
    writer: BufWriter<tokio::io::WriteHalf<Stream>>,
    read_buf: Vec<u8>,
    next_id: AtomicU64,
    /// Pending notifications from server (received while waiting for response)
    pending_notifications: VecDeque<Request>,
}

impl Connection {
    /// Create a new connection from a transport stream.
    ///
    /// The stream is split into read/write halves for concurrent I/O.
    pub fn new(stream: Stream) -> Self {
        let (read_half, write_half) = tokio::io::split(stream);
        Self {
            reader: read_half,
            writer: BufWriter::new(write_half),
            read_buf: Vec::with_capacity(4096),
            next_id: AtomicU64::new(1),
            pending_notifications: VecDeque::new(),
        }
    }

    /// Get the next request ID.
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Send a request and wait for the matching response.
    ///
    /// Any notifications received while waiting are buffered and can be
    /// retrieved via [`pop_notification`](Self::pop_notification).
    ///
    /// Returns the result value, or an error if the RPC failed.
    pub async fn transact(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id();
        let request = Request::new(method, params, id);

        self.send_message(&Message::Request(request)).await?;
        let response = self.recv_response(id).await?;

        if let Some(error) = response.error {
            return Err(Error::Rpc(error));
        }

        Ok(response.result.unwrap_or(Value::Null))
    }

    /// Send a notification (no response expected).
    pub async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        let request = Request::notification(method, params);
        self.send_message(&Message::Request(request)).await
    }

    /// Send a message.
    pub async fn send_message(&mut self, msg: &Message) -> Result<()> {
        let json = serde_json::to_string(msg)?;
        tracing::debug!("TX: {}", json);
        self.writer.write_all(json.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Receive a response, buffering any notifications received in the meantime.
    async fn recv_response(&mut self, expected_id: u64) -> Result<Response> {
        let expected = RpcId::Number(expected_id);
        loop {
            let msg = self.recv_message().await?;

            match msg {
                Message::Response(resp) => {
                    if resp.id != expected {
                        return Err(Error::UnexpectedId {
                            expected: expected_id,
                            got: resp.id.as_u64().unwrap_or(0),
                        });
                    }
                    return Ok(resp);
                }
                Message::Request(req) => {
                    // Server notification - buffer it for later processing
                    tracing::debug!("Buffering notification: {}", req.method);
                    self.pending_notifications.push_back(req);
                }
            }
        }
    }

    /// Receive a single message from the connection.
    ///
    /// Uses brace-depth tracking to detect complete JSON objects since
    /// OVSDB servers don't send newlines after responses. Handles:
    /// - Nested JSON objects
    /// - Strings containing braces
    /// - Escaped characters within strings
    ///
    /// Returns either a [`Request`] (notification from server) or
    /// [`Response`] (reply to our request).
    pub async fn recv_message(&mut self) -> Result<Message> {
        tracing::debug!("Waiting to receive message...");

        // Read bytes until we have a complete JSON object
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape_next = false;
        let mut json_start = None;

        loop {
            // Try to parse from existing buffer first
            for (i, &byte) in self.read_buf.iter().enumerate() {
                if json_start.is_none() && byte == b'{' {
                    json_start = Some(i);
                }

                if json_start.is_some() {
                    if escape_next {
                        escape_next = false;
                        continue;
                    }

                    match byte {
                        b'\\' if in_string => escape_next = true,
                        b'"' => in_string = !in_string,
                        b'{' if !in_string => depth += 1,
                        b'}' if !in_string => {
                            depth -= 1;
                            if depth == 0 {
                                // Found complete JSON object
                                let start = json_start.unwrap();
                                let json_bytes = &self.read_buf[start..=i];
                                let json_str = String::from_utf8_lossy(json_bytes);
                                tracing::debug!("RX: {}", json_str);

                                let msg: Message = serde_json::from_slice(json_bytes)?;

                                // Remove consumed bytes from buffer
                                self.read_buf.drain(..=i);

                                return Ok(msg);
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Need more data - reset parsing state since we'll re-scan from start
            depth = 0;
            in_string = false;
            escape_next = false;
            json_start = None;

            let mut buf = [0u8; 4096];
            tracing::debug!("Reading more data...");
            let n = self.reader.read(&mut buf).await?;
            tracing::debug!("Read {} bytes", n);

            if n == 0 {
                return Err(Error::ConnectionClosed);
            }

            self.read_buf.extend_from_slice(&buf[..n]);
            tracing::debug!("Buffer now has {} bytes", self.read_buf.len());
        }
    }

    /// Check if there are buffered notifications from the server.
    ///
    /// Notifications received while waiting for a response are buffered here.
    pub fn has_pending_notifications(&self) -> bool {
        !self.pending_notifications.is_empty()
    }

    /// Get the count of pending notifications.
    pub fn pending_notification_count(&self) -> usize {
        self.pending_notifications.len()
    }

    /// Pop the next buffered notification (FIFO order).
    ///
    /// Returns `None` if no notifications are pending.
    pub fn pop_notification(&mut self) -> Option<Request> {
        self.pending_notifications.pop_front()
    }

    /// Drain all buffered notifications, returning an iterator.
    ///
    /// Clears the notification buffer and yields all pending notifications.
    pub fn drain_notifications(&mut self) -> impl Iterator<Item = Request> + '_ {
        self.pending_notifications.drain(..)
    }
}
