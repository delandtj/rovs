//! MAC and IP address conversion utilities.

use crate::{Error, Result};

/// Convert MAC address bytes to u64 for use with OpenFlow actions.
///
/// The MAC address is stored in the lower 48 bits of the u64.
///
/// # Example
///
/// ```
/// use rovs_ext::util::mac_to_u64;
///
/// let mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
/// assert_eq!(mac_to_u64(&mac), 0x0002_0000_0000_0001);
/// ```
#[must_use]
pub fn mac_to_u64(mac: &[u8; 6]) -> u64 {
    ((mac[0] as u64) << 40)
        | ((mac[1] as u64) << 32)
        | ((mac[2] as u64) << 24)
        | ((mac[3] as u64) << 16)
        | ((mac[4] as u64) << 8)
        | (mac[5] as u64)
}

/// Convert u64 to MAC address bytes.
///
/// Extracts the lower 48 bits as a MAC address.
///
/// # Example
///
/// ```
/// use rovs_ext::util::u64_to_mac;
///
/// let mac = u64_to_mac(0x0002_0000_0000_0001);
/// assert_eq!(mac, [0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
/// ```
#[must_use]
pub fn u64_to_mac(value: u64) -> [u8; 6] {
    [
        ((value >> 40) & 0xff) as u8,
        ((value >> 32) & 0xff) as u8,
        ((value >> 24) & 0xff) as u8,
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    ]
}

/// Convert IPv4 address bytes to u32.
///
/// # Example
///
/// ```
/// use rovs_ext::util::ipv4_to_u32;
///
/// let ip = [10, 0, 0, 1];
/// assert_eq!(ipv4_to_u32(&ip), 0x0a000001);
/// ```
#[must_use]
pub fn ipv4_to_u32(ip: &[u8; 4]) -> u32 {
    ((ip[0] as u32) << 24) | ((ip[1] as u32) << 16) | ((ip[2] as u32) << 8) | (ip[3] as u32)
}

/// Convert u32 to IPv4 address bytes.
///
/// # Example
///
/// ```
/// use rovs_ext::util::u32_to_ipv4;
///
/// let ip = u32_to_ipv4(0x0a000001);
/// assert_eq!(ip, [10, 0, 0, 1]);
/// ```
#[must_use]
pub fn u32_to_ipv4(value: u32) -> [u8; 4] {
    [
        ((value >> 24) & 0xff) as u8,
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    ]
}

/// Format MAC address as colon-separated hex string.
///
/// # Example
///
/// ```
/// use rovs_ext::util::format_mac;
///
/// let mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
/// assert_eq!(format_mac(&mac), "02:00:00:00:00:01");
/// ```
#[must_use]
pub fn format_mac(mac: &[u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

/// Parse MAC address from colon or dash separated hex string.
///
/// Accepts formats like "02:00:00:00:00:01" or "02-00-00-00-00-01".
///
/// # Example
///
/// ```
/// use rovs_ext::util::parse_mac;
///
/// let mac = parse_mac("02:00:00:00:00:01").unwrap();
/// assert_eq!(mac, [0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
/// ```
pub fn parse_mac(s: &str) -> Result<[u8; 6]> {
    let parts: Vec<&str> = if s.contains(':') {
        s.split(':').collect()
    } else if s.contains('-') {
        s.split('-').collect()
    } else {
        return Err(Error::InvalidMac(format!(
            "invalid format, expected xx:xx:xx:xx:xx:xx: {s}"
        )));
    };

    if parts.len() != 6 {
        return Err(Error::InvalidMac(format!(
            "expected 6 octets, got {}: {s}",
            parts.len()
        )));
    }

    let mut mac = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(part, 16)
            .map_err(|e| Error::InvalidMac(format!("invalid hex octet '{part}': {e}")))?;
    }

    Ok(mac)
}

/// Format IPv4 address as dotted decimal string.
///
/// # Example
///
/// ```
/// use rovs_ext::util::format_ipv4;
///
/// let ip = [10, 0, 0, 1];
/// assert_eq!(format_ipv4(&ip), "10.0.0.1");
/// ```
#[must_use]
pub fn format_ipv4(ip: &[u8; 4]) -> String {
    format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
}

/// Parse IPv4 address from dotted decimal string.
///
/// # Example
///
/// ```
/// use rovs_ext::util::parse_ipv4;
///
/// let ip = parse_ipv4("10.0.0.1").unwrap();
/// assert_eq!(ip, [10, 0, 0, 1]);
/// ```
pub fn parse_ipv4(s: &str) -> Result<[u8; 4]> {
    let parts: Vec<&str> = s.split('.').collect();

    if parts.len() != 4 {
        return Err(Error::InvalidIp(format!(
            "expected 4 octets, got {}: {s}",
            parts.len()
        )));
    }

    let mut ip = [0u8; 4];
    for (i, part) in parts.iter().enumerate() {
        ip[i] = part
            .parse()
            .map_err(|e| Error::InvalidIp(format!("invalid octet '{part}': {e}")))?;
    }

    Ok(ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_round_trip() {
        let mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x99];
        let u = mac_to_u64(&mac);
        assert_eq!(u64_to_mac(u), mac);
    }

    #[test]
    fn ipv4_round_trip() {
        let ip = [192, 168, 1, 100];
        let u = ipv4_to_u32(&ip);
        assert_eq!(u32_to_ipv4(u), ip);
    }

    #[test]
    fn parse_mac_colon() {
        let mac = parse_mac("aa:bb:cc:dd:ee:ff").unwrap();
        assert_eq!(mac, [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    }

    #[test]
    fn parse_mac_dash() {
        let mac = parse_mac("aa-bb-cc-dd-ee-ff").unwrap();
        assert_eq!(mac, [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    }

    #[test]
    fn parse_mac_invalid() {
        assert!(parse_mac("invalid").is_err());
        assert!(parse_mac("aa:bb:cc:dd:ee").is_err());
        assert!(parse_mac("aa:bb:cc:dd:ee:ff:00").is_err());
        assert!(parse_mac("gg:bb:cc:dd:ee:ff").is_err());
    }

    #[test]
    fn parse_ipv4_valid() {
        let ip = parse_ipv4("10.20.30.40").unwrap();
        assert_eq!(ip, [10, 20, 30, 40]);
    }

    #[test]
    fn parse_ipv4_invalid() {
        assert!(parse_ipv4("invalid").is_err());
        assert!(parse_ipv4("10.20.30").is_err());
        assert!(parse_ipv4("10.20.30.40.50").is_err());
        assert!(parse_ipv4("256.0.0.1").is_err());
    }

    #[test]
    fn format_mac_test() {
        let mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
        assert_eq!(format_mac(&mac), "02:00:00:00:00:01");
    }

    #[test]
    fn format_ipv4_test() {
        let ip = [192, 168, 0, 1];
        assert_eq!(format_ipv4(&ip), "192.168.0.1");
    }
}
