//! Connection tracking types and parsing for `dpctl/dump-conntrack` output.

use std::fmt;

/// A connection tracking entry from `dpctl/dump-conntrack`.
///
/// Parses the most operationally useful fields from each conntrack line.
/// Fields that aren't explicitly parsed are preserved in [`raw`](Self::raw).
///
/// # Display
///
/// The `Display` impl produces a human-readable summary:
///
/// ```text
/// tcp ESTABLISHED 10.0.0.1:54321 -> 10.0.0.2:80 (zone=1)
/// udp 192.168.1.5:5060 -> 203.0.113.1:5060
/// icmp 10.0.0.1 -> 10.0.0.2
/// ```
#[derive(Debug, Clone)]
pub struct ConntrackEntry {
    /// Protocol name (e.g., `tcp`, `udp`, `icmp`)
    pub protocol: String,
    /// Connection state (e.g., `ESTABLISHED`, `SYN_SENT`, `ASSURED`)
    pub state: Option<String>,
    /// Source address
    pub src: String,
    /// Destination address
    pub dst: String,
    /// Source port (TCP/UDP only)
    pub sport: Option<u16>,
    /// Destination port (TCP/UDP only)
    pub dport: Option<u16>,
    /// Zone ID
    pub zone: Option<u16>,
    /// Mark value
    pub mark: Option<u32>,
    /// Full original line for fields we don't parse
    pub raw: String,
}

impl fmt::Display for ConntrackEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.protocol)?;

        if let Some(state) = &self.state {
            write!(f, " {state}")?;
        }

        // Source endpoint
        write!(f, " {}", self.src)?;
        if let Some(port) = self.sport {
            write!(f, ":{port}")?;
        }

        write!(f, " -> ")?;

        // Destination endpoint
        write!(f, "{}", self.dst)?;
        if let Some(port) = self.dport {
            write!(f, ":{port}")?;
        }

        // Zone and mark as annotations
        let mut annotations = Vec::new();
        if let Some(z) = self.zone {
            annotations.push(format!("zone={z}"));
        }
        if let Some(m) = self.mark {
            if m != 0 {
                annotations.push(format!("mark=0x{m:x}"));
            }
        }
        if !annotations.is_empty() {
            write!(f, " ({})", annotations.join(", "))?;
        }

        Ok(())
    }
}

/// Parse the output of `dpctl/dump-conntrack` into structured entries.
///
/// Each line looks like one of:
/// ```text
/// tcp,orig=(src=10.0.0.1,dst=10.0.0.2,sport=54321,dport=80),reply=(src=10.0.0.2,dst=10.0.0.1,sport=80,dport=54321),zone=1,mark=0,protoinfo=(state=ESTABLISHED)
/// udp,orig=(src=192.168.1.5,dst=203.0.113.1,sport=5060,dport=5060),reply=(...)
/// icmp,orig=(src=10.0.0.1,dst=10.0.0.2,id=1234),reply=(...)
/// ```
pub(crate) fn parse_conntrack_entries(output: &str) -> Vec<ConntrackEntry> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_one_entry)
        .collect()
}

/// Parse a single conntrack entry line.
fn parse_one_entry(line: &str) -> ConntrackEntry {
    let raw = line.to_owned();

    // Protocol is the first field before the first comma
    let protocol = line.split(',').next().unwrap_or("unknown").to_owned();

    // Extract fields from orig=(...) tuple
    let (src, dst, sport, dport) = parse_orig_tuple(line);

    // Extract zone
    let zone = extract_field(line, "zone=").and_then(|v| v.parse().ok());

    // Extract mark
    let mark = extract_field(line, "mark=").and_then(|v| {
        if let Some(hex) = v.strip_prefix("0x") {
            u32::from_str_radix(hex, 16).ok()
        } else {
            v.parse().ok()
        }
    });

    // Extract state from protoinfo=(state=...)
    let state = extract_protoinfo_state(line);

    ConntrackEntry {
        protocol,
        state,
        src: src.unwrap_or_default(),
        dst: dst.unwrap_or_default(),
        sport,
        dport,
        zone,
        mark,
        raw,
    }
}

/// Extract src, dst, sport, dport from the `orig=(...)` tuple.
fn parse_orig_tuple(line: &str) -> (Option<String>, Option<String>, Option<u16>, Option<u16>) {
    let orig_content = match extract_paren_content(line, "orig=") {
        Some(c) => c,
        None => return (None, None, None, None),
    };

    let src = extract_field(&orig_content, "src=").map(String::from);
    let dst = extract_field(&orig_content, "dst=").map(String::from);
    let sport = extract_field(&orig_content, "sport=").and_then(|v| v.parse().ok());
    let dport = extract_field(&orig_content, "dport=").and_then(|v| v.parse().ok());

    (src, dst, sport, dport)
}

/// Extract the state from `protoinfo=(state=...)`.
fn extract_protoinfo_state(line: &str) -> Option<String> {
    let content = extract_paren_content(line, "protoinfo=")?;
    extract_field(&content, "state=").map(String::from)
}

/// Extract content within parentheses after a prefix like `orig=(...)`.
fn extract_paren_content(line: &str, prefix: &str) -> Option<String> {
    let start = line.find(prefix)? + prefix.len();
    if line.as_bytes().get(start)? != &b'(' {
        return None;
    }

    let mut depth = 0i32;
    for (i, ch) in line[start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(line[start + 1..start + i].to_owned());
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract a simple `key=value` field, where value ends at `,` or `)` or end of string.
fn extract_field<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let start = s.find(key)? + key.len();
    let rest = &s[start..];
    let end = rest.find([',', ')']).unwrap_or(rest.len());
    let value = &rest[..end];
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tcp_entry() {
        let line = "tcp,orig=(src=10.0.0.1,dst=10.0.0.2,sport=54321,dport=80),reply=(src=10.0.0.2,dst=10.0.0.1,sport=80,dport=54321),zone=1,mark=0,protoinfo=(state=ESTABLISHED)";
        let entries = parse_conntrack_entries(line);
        assert_eq!(entries.len(), 1);

        let e = &entries[0];
        assert_eq!(e.protocol, "tcp");
        assert_eq!(e.state.as_deref(), Some("ESTABLISHED"));
        assert_eq!(e.src, "10.0.0.1");
        assert_eq!(e.dst, "10.0.0.2");
        assert_eq!(e.sport, Some(54321));
        assert_eq!(e.dport, Some(80));
        assert_eq!(e.zone, Some(1));
        assert_eq!(e.mark, Some(0));
    }

    #[test]
    fn parse_udp_entry() {
        let line = "udp,orig=(src=192.168.1.5,dst=203.0.113.1,sport=5060,dport=5060),reply=(src=203.0.113.1,dst=192.168.1.5,sport=5060,dport=5060),zone=2";
        let entries = parse_conntrack_entries(line);
        let e = &entries[0];
        assert_eq!(e.protocol, "udp");
        assert_eq!(e.src, "192.168.1.5");
        assert_eq!(e.dst, "203.0.113.1");
        assert_eq!(e.sport, Some(5060));
        assert_eq!(e.dport, Some(5060));
        assert_eq!(e.zone, Some(2));
        assert!(e.state.is_none());
    }

    #[test]
    fn parse_icmp_entry() {
        let line = "icmp,orig=(src=10.0.0.1,dst=10.0.0.2,id=1234,type=8,code=0),reply=(src=10.0.0.2,dst=10.0.0.1,id=1234,type=0,code=0)";
        let entries = parse_conntrack_entries(line);
        let e = &entries[0];
        assert_eq!(e.protocol, "icmp");
        assert_eq!(e.src, "10.0.0.1");
        assert_eq!(e.dst, "10.0.0.2");
        assert!(e.sport.is_none());
        assert!(e.dport.is_none());
        assert!(e.zone.is_none());
    }

    #[test]
    fn parse_empty_output() {
        let entries = parse_conntrack_entries("");
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_multiple_entries() {
        let output = "\
tcp,orig=(src=10.0.0.1,dst=10.0.0.2,sport=54321,dport=80),reply=(src=10.0.0.2,dst=10.0.0.1,sport=80,dport=54321),protoinfo=(state=ESTABLISHED)
udp,orig=(src=10.0.0.3,dst=10.0.0.4,sport=1234,dport=53),reply=(src=10.0.0.4,dst=10.0.0.3,sport=53,dport=1234)
";
        let entries = parse_conntrack_entries(output);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn display_tcp() {
        let entry = ConntrackEntry {
            protocol: "tcp".to_owned(),
            state: Some("ESTABLISHED".to_owned()),
            src: "10.0.0.1".to_owned(),
            dst: "10.0.0.2".to_owned(),
            sport: Some(54321),
            dport: Some(80),
            zone: Some(1),
            mark: Some(0),
            raw: String::new(),
        };
        assert_eq!(
            entry.to_string(),
            "tcp ESTABLISHED 10.0.0.1:54321 -> 10.0.0.2:80 (zone=1)"
        );
    }

    #[test]
    fn display_udp_no_zone() {
        let entry = ConntrackEntry {
            protocol: "udp".to_owned(),
            state: None,
            src: "192.168.1.5".to_owned(),
            dst: "203.0.113.1".to_owned(),
            sport: Some(5060),
            dport: Some(5060),
            zone: None,
            mark: None,
            raw: String::new(),
        };
        assert_eq!(
            entry.to_string(),
            "udp 192.168.1.5:5060 -> 203.0.113.1:5060"
        );
    }

    #[test]
    fn display_icmp() {
        let entry = ConntrackEntry {
            protocol: "icmp".to_owned(),
            state: None,
            src: "10.0.0.1".to_owned(),
            dst: "10.0.0.2".to_owned(),
            sport: None,
            dport: None,
            zone: None,
            mark: None,
            raw: String::new(),
        };
        assert_eq!(entry.to_string(), "icmp 10.0.0.1 -> 10.0.0.2");
    }

    #[test]
    fn display_with_nonzero_mark() {
        let entry = ConntrackEntry {
            protocol: "tcp".to_owned(),
            state: Some("SYN_SENT".to_owned()),
            src: "10.0.0.1".to_owned(),
            dst: "10.0.0.2".to_owned(),
            sport: Some(12345),
            dport: Some(443),
            zone: Some(2),
            mark: Some(0x1a),
            raw: String::new(),
        };
        assert_eq!(
            entry.to_string(),
            "tcp SYN_SENT 10.0.0.1:12345 -> 10.0.0.2:443 (zone=2, mark=0x1a)"
        );
    }
}
