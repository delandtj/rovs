//! Async stream abstraction.

use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpStream, UnixStream};

use crate::{Address, Error, Result};

/// A transport stream that can be Unix, TCP, or TLS.
pub enum Stream {
    /// Unix domain socket
    Unix(UnixStream),
    /// TCP socket
    Tcp(TcpStream),
    /// TLS over TCP
    Tls(Box<tokio_rustls::client::TlsStream<TcpStream>>),
}

impl Stream {
    /// Connect to the given address.
    pub async fn connect(addr: &Address) -> Result<Self> {
        match addr {
            Address::Unix(path) => {
                let stream = UnixStream::connect(path).await?;
                Ok(Self::Unix(stream))
            }
            Address::Tcp { host, port } => {
                let stream = TcpStream::connect(format!("{host}:{port}")).await?;
                Ok(Self::Tcp(stream))
            }
            Address::Tls { host: _, port: _ } => {
                // TODO: Implement TLS connection
                Err(Error::Tls("TLS not yet implemented".into()))
            }
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Unix(s) => Pin::new(s).poll_read(cx, buf),
            Self::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            Self::Tls(s) => Pin::new(s.as_mut()).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Self::Unix(s) => Pin::new(s).poll_write(cx, buf),
            Self::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            Self::Tls(s) => Pin::new(s.as_mut()).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Unix(s) => Pin::new(s).poll_flush(cx),
            Self::Tcp(s) => Pin::new(s).poll_flush(cx),
            Self::Tls(s) => Pin::new(s.as_mut()).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Unix(s) => Pin::new(s).poll_shutdown(cx),
            Self::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            Self::Tls(s) => Pin::new(s.as_mut()).poll_shutdown(cx),
        }
    }
}
