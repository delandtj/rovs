//! Connection address parsing.

use std::path::PathBuf;
use std::str::FromStr;

use crate::Error;

/// A parsed connection address.
///
/// Supports the following formats:
/// - `unix:/path/to/socket` - Unix domain socket
/// - `tcp:host:port` - TCP connection
/// - `ssl:host:port` - TLS over TCP
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    /// Unix domain socket
    Unix(PathBuf),
    /// TCP connection
    Tcp { host: String, port: u16 },
    /// TLS over TCP
    Tls { host: String, port: u16 },
}

impl FromStr for Address {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(path) = s.strip_prefix("unix:") {
            Ok(Self::Unix(PathBuf::from(path)))
        } else if let Some(rest) = s.strip_prefix("tcp:") {
            let (host, port) = parse_host_port(rest)?;
            Ok(Self::Tcp { host, port })
        } else if let Some(rest) = s.strip_prefix("ssl:") {
            let (host, port) = parse_host_port(rest)?;
            Ok(Self::Tls { host, port })
        } else {
            Err(Error::InvalidAddress(format!(
                "unknown scheme in address: {s}"
            )))
        }
    }
}

fn parse_host_port(s: &str) -> Result<(String, u16), Error> {
    let (host, port_str) = s
        .rsplit_once(':')
        .ok_or_else(|| Error::InvalidAddress(format!("missing port in address: {s}")))?;

    let port = port_str
        .parse()
        .map_err(|_| Error::InvalidAddress(format!("invalid port: {port_str}")))?;

    Ok((host.to_owned(), port))
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unix(path) => write!(f, "unix:{}", path.display()),
            Self::Tcp { host, port } => write!(f, "tcp:{host}:{port}"),
            Self::Tls { host, port } => write!(f, "ssl:{host}:{port}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_unix() {
        let addr: Address = "unix:/var/run/openvswitch/db.sock".parse().unwrap();
        assert_eq!(
            addr,
            Address::Unix(PathBuf::from("/var/run/openvswitch/db.sock"))
        );
    }

    #[test]
    fn parse_tcp() {
        let addr: Address = "tcp:127.0.0.1:6640".parse().unwrap();
        assert_eq!(
            addr,
            Address::Tcp {
                host: "127.0.0.1".to_owned(),
                port: 6640
            }
        );
    }

    #[test]
    fn parse_ssl() {
        let addr: Address = "ssl:ovs.example.com:6640".parse().unwrap();
        assert_eq!(
            addr,
            Address::Tls {
                host: "ovs.example.com".to_owned(),
                port: 6640
            }
        );
    }
}
