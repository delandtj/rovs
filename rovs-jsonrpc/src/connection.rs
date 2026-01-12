//! JSON-RPC connection handling.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};

use rovs_transport::Stream;

use crate::{Error, Message, Request, Response, Result};

/// A JSON-RPC connection over a transport stream.
pub struct Connection {
    reader: tokio::io::ReadHalf<Stream>,
    writer: BufWriter<tokio::io::WriteHalf<Stream>>,
    read_buf: Vec<u8>,
    next_id: AtomicU64,
    /// Pending notifications from server (received while waiting for response)
    pending_notifications: VecDeque<Request>,
}

impl Connection {
    /// Create a new connection from a stream.
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

    /// Send a request and wait for the response.
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
        loop {
            let msg = self.recv_message().await?;

            match msg {
                Message::Response(resp) => {
                    if resp.id != expected_id {
                        return Err(Error::UnexpectedId {
                            expected: expected_id,
                            got: resp.id,
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
    /// Reads JSON incrementally, handling the fact that OVSDB doesn't
    /// send newlines after responses.
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

    /// Check if there are pending notifications from the server.
    pub fn has_pending_notifications(&self) -> bool {
        !self.pending_notifications.is_empty()
    }

    /// Get the next pending notification, if any.
    pub fn pop_notification(&mut self) -> Option<Request> {
        self.pending_notifications.pop_front()
    }

    /// Process all pending notifications with a callback.
    pub fn drain_notifications(&mut self) -> impl Iterator<Item = Request> + '_ {
        self.pending_notifications.drain(..)
    }
}
