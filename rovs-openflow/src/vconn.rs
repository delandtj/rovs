//! Virtual connection (VConn) for OpenFlow.

use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use rovs_transport::{Address, Stream};

use crate::{Error, FlowMod, Header, Message, MessageType, Result, Version};

/// An OpenFlow virtual connection.
pub struct VConn {
    stream: Stream,
    version: Version,
    next_xid: u32,
}

impl VConn {
    /// Connect to an OpenFlow switch.
    pub async fn connect(addr: &Address) -> Result<Self> {
        let stream = Stream::connect(addr).await?;

        let mut conn = Self {
            stream,
            version: Version::Of13, // Will be negotiated
            next_xid: 1,
        };

        conn.handshake().await?;
        Ok(conn)
    }

    /// Get the negotiated OpenFlow version.
    pub fn version(&self) -> Version {
        self.version
    }

    /// Perform the OpenFlow handshake.
    async fn handshake(&mut self) -> Result<()> {
        // Send Hello
        let hello = Message::new(
            Version::Of13,
            MessageType::Hello,
            self.next_xid(),
            Bytes::new(),
        );
        self.send_message(&hello).await?;

        // Receive Hello
        let reply = self.recv_message().await?;
        if reply.header.msg_type != MessageType::Hello {
            return Err(Error::InvalidMessage("expected Hello".into()));
        }

        // Use the lower of the two versions
        self.version = std::cmp::min(self.version, reply.header.version);

        Ok(())
    }

    /// Get the next transaction ID.
    fn next_xid(&mut self) -> u32 {
        let xid = self.next_xid;
        self.next_xid = self.next_xid.wrapping_add(1);
        xid
    }

    /// Send a message.
    pub async fn send_message(&mut self, msg: &Message) -> Result<()> {
        let bytes = msg.encode();
        self.stream.write_all(&bytes).await?;
        self.stream.flush().await?;
        Ok(())
    }

    /// Receive a message.
    pub async fn recv_message(&mut self) -> Result<Message> {
        // Read header
        let mut header_buf = [0u8; Header::SIZE];
        self.stream.read_exact(&mut header_buf).await?;
        let header = Header::decode(&mut Bytes::copy_from_slice(&header_buf))?;

        // Read body
        let body_len = header.length as usize - Header::SIZE;
        let mut body = BytesMut::zeroed(body_len);
        if body_len > 0 {
            self.stream.read_exact(&mut body).await?;
        }

        Ok(Message {
            header,
            body: body.freeze(),
        })
    }

    /// Send a flow mod.
    pub async fn send_flow_mod(&mut self, _flow_mod: &FlowMod) -> Result<()> {
        // TODO: Encode FlowMod to bytes
        // For now, just a placeholder
        todo!("FlowMod encoding not yet implemented")
    }

    /// Send an echo request and wait for reply.
    pub async fn echo(&mut self) -> Result<()> {
        let xid = self.next_xid();
        let request = Message::new(self.version, MessageType::EchoRequest, xid, Bytes::new());
        self.send_message(&request).await?;

        let reply = self.recv_message().await?;
        if reply.header.msg_type != MessageType::EchoReply {
            return Err(Error::InvalidMessage("expected EchoReply".into()));
        }
        if reply.header.xid != xid {
            return Err(Error::InvalidMessage("xid mismatch".into()));
        }

        Ok(())
    }

    /// Send a barrier and wait for reply.
    pub async fn barrier(&mut self) -> Result<()> {
        let xid = self.next_xid();
        let request = Message::new(self.version, MessageType::BarrierRequest, xid, Bytes::new());
        self.send_message(&request).await?;

        let reply = self.recv_message().await?;
        if reply.header.msg_type != MessageType::BarrierReply {
            return Err(Error::InvalidMessage("expected BarrierReply".into()));
        }

        Ok(())
    }
}
