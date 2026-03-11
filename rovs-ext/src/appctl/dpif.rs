//! Datapath flow types and parsing for `dpif/dump-flows` output.

use std::fmt;

use crate::{Error, Result};

/// A datapath flow from `dpif/dump-flows`.
///
/// Represents a single entry in the OVS datapath flow table, including
/// the match key, actions, and packet/byte statistics.
///
/// # Display
///
/// The `Display` impl produces a human-readable summary:
///
/// ```text
/// in_port(1),eth_type(0x0800) => output:2  [150 pkts, 12.3 KB, used 0.500s ago]
/// in_port(2),eth_type(0x0806) => drop  [0 pkts, 0 B, never used]
/// ```
#[derive(Debug, Clone)]
pub struct DpifFlow {
    /// Full match key (e.g., `recirc_id(0),in_port(1),eth_type(0x0806)`)
    pub key: String,
    /// Wildcard mask (present with `-m` flag)
    pub mask: Option<String>,
    /// Actions (e.g., `2` or `set(eth(src=02:00:00:00:00:01)),1`)
    pub actions: String,
    /// Packet count
    pub packets: u64,
    /// Byte count
    pub bytes: u64,
    /// Seconds since last used (`None` if never matched)
    pub used: Option<f64>,
}

impl fmt::Display for DpifFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let actions_display = if self.actions.is_empty() {
            "drop"
        } else {
            &self.actions
        };

        write!(f, "{} => {actions_display}", self.key)?;

        write!(f, "  [{}, {}", format_packets(self.packets), format_bytes(self.bytes))?;

        match self.used {
            Some(secs) => write!(f, ", used {secs:.3}s ago]")?,
            None => write!(f, ", never used]")?,
        }

        if let Some(mask) = &self.mask {
            write!(f, " mask: {mask}")?;
        }

        Ok(())
    }
}

/// Parse the output of `dpif/dump-flows` into structured flows.
///
/// Each line has the format:
/// ```text
/// <key>, packets:<n>, bytes:<n>, used:<n>s, actions:<actions>
/// ```
/// With optional `mask(<mask>),` after the key when `-m` is used.
pub(crate) fn parse_dpif_flows(output: &str) -> Vec<DpifFlow> {
    let mut flows = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match parse_one_flow(line) {
            Ok(flow) => flows.push(flow),
            Err(e) => {
                tracing::warn!("skipping unparseable dpif flow line: {e} — {line}");
            }
        }
    }

    flows
}

/// Parse a single dpif flow line.
///
/// Format: `<key>[, mask(<mask>)], packets:<n>, bytes:<n>, used:<n>s, actions:<actions>`
///
/// The tricky part is that the key itself contains commas and parentheses,
/// so we parse the stats fields from the right side first.
fn parse_one_flow(line: &str) -> Result<DpifFlow> {
    // Find "actions:" — it's always last and the value can contain commas/parens
    let actions_pos = line
        .rfind(", actions:")
        .ok_or_else(|| Error::Parse(format!("missing 'actions:' in dpif flow: {line}")))?;
    let actions = line[actions_pos + ", actions:".len()..].to_owned();
    let remainder = &line[..actions_pos];

    // Parse stats fields from the right: used:<n>s, bytes:<n>, packets:<n>
    let mut packets = 0u64;
    let mut bytes = 0u64;
    let mut used = None;

    // Walk backwards through the comma-separated stats
    let mut rest = remainder;
    for field_name in &["used:", "bytes:", "packets:"] {
        if let Some(pos) = rest.rfind(&format!(", {field_name}")) {
            let field_value = &rest[pos + 2 + field_name.len()..].trim();
            match *field_name {
                "packets:" => packets = field_value.parse().unwrap_or(0),
                "bytes:" => bytes = field_value.parse().unwrap_or(0),
                "used:" => {
                    let secs_str = field_value.strip_suffix('s').unwrap_or(field_value);
                    if secs_str != "never" {
                        used = secs_str.parse().ok();
                    }
                }
                _ => {}
            }
            rest = &rest[..pos];
        }
    }

    let key_section = rest.trim();

    // Check for mask — appears as a separate section after the key when -m is used.
    // Format varies: sometimes "recirc_id(0),..., packets:..." with mask on next segment
    // Most commonly the mask appears inline as a second set of fields after the key.
    // For simplicity, we look for a ", mask(" separator.
    let (key, mask) = if let Some(mask_pos) = key_section.find(", mask(") {
        let k = key_section[..mask_pos].to_owned();
        // mask value runs until the matching close paren at depth 0
        let mask_start = mask_pos + ", mask(".len();
        let mask_val = extract_balanced_parens(&key_section[mask_start - 1..]);
        (k, Some(mask_val))
    } else {
        (key_section.to_owned(), None)
    };

    Ok(DpifFlow {
        key,
        mask,
        actions,
        packets,
        bytes,
        used,
    })
}

/// Extract content within balanced parentheses starting at the opening paren.
/// Returns the content including the outer parens.
fn extract_balanced_parens(s: &str) -> String {
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return s[1..i].to_owned();
                }
            }
            _ => {}
        }
    }
    // If unbalanced, return everything after the opening paren
    s[1..].to_owned()
}

fn format_packets(n: u64) -> String {
    if n == 1 {
        "1 pkt".to_owned()
    } else {
        format!("{n} pkts")
    }
}

#[allow(clippy::cast_precision_loss)]
fn format_bytes(n: u64) -> String {
    if n < 1_024 {
        format!("{n} B")
    } else if n < 1_048_576 {
        format!("{:.1} KB", n as f64 / 1_024.0)
    } else if n < 1_073_741_824 {
        format!("{:.1} MB", n as f64 / 1_048_576.0)
    } else {
        format!("{:.1} GB", n as f64 / 1_073_741_824.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_flow() {
        let line = "recirc_id(0),in_port(1),eth(src=00:01:02:03:04:05,dst=ff:ff:ff:ff:ff:ff),eth_type(0x0806), packets:5, bytes:210, used:1.234s, actions:2";
        let flows = parse_dpif_flows(line);
        assert_eq!(flows.len(), 1);

        let flow = &flows[0];
        assert_eq!(
            flow.key,
            "recirc_id(0),in_port(1),eth(src=00:01:02:03:04:05,dst=ff:ff:ff:ff:ff:ff),eth_type(0x0806)"
        );
        assert_eq!(flow.actions, "2");
        assert_eq!(flow.packets, 5);
        assert_eq!(flow.bytes, 210);
        assert_eq!(flow.used, Some(1.234));
        assert!(flow.mask.is_none());
    }

    #[test]
    fn parse_flow_with_complex_actions() {
        let line = "recirc_id(0),in_port(2),eth_type(0x0800), packets:100, bytes:10000, used:0.500s, actions:set(eth(src=02:00:00:00:00:01)),1";
        let flows = parse_dpif_flows(line);
        let flow = &flows[0];
        assert_eq!(flow.actions, "set(eth(src=02:00:00:00:00:01)),1");
        assert_eq!(flow.packets, 100);
        assert_eq!(flow.bytes, 10000);
    }

    #[test]
    fn parse_never_used_flow() {
        let line = "recirc_id(0),in_port(3), packets:0, bytes:0, used:never, actions:drop";
        let flows = parse_dpif_flows(line);
        let flow = &flows[0];
        assert_eq!(flow.packets, 0);
        assert_eq!(flow.bytes, 0);
        assert!(flow.used.is_none());
        assert_eq!(flow.actions, "drop");
    }

    #[test]
    fn parse_multiple_flows() {
        let output = "\
recirc_id(0),in_port(1),eth_type(0x0806), packets:5, bytes:210, used:1.234s, actions:2
recirc_id(0),in_port(2),eth_type(0x0800), packets:100, bytes:10000, used:0.500s, actions:1

";
        let flows = parse_dpif_flows(output);
        assert_eq!(flows.len(), 2);
    }

    #[test]
    fn parse_empty_output() {
        let flows = parse_dpif_flows("");
        assert!(flows.is_empty());

        let flows = parse_dpif_flows("\n\n");
        assert!(flows.is_empty());
    }

    #[test]
    fn display_basic_flow() {
        let flow = DpifFlow {
            key: "in_port(1),eth_type(0x0800)".to_owned(),
            mask: None,
            actions: "2".to_owned(),
            packets: 150,
            bytes: 12_600,
            used: Some(0.5),
        };
        let s = flow.to_string();
        assert!(s.contains("in_port(1),eth_type(0x0800) => 2"));
        assert!(s.contains("150 pkts"));
        assert!(s.contains("12.3 KB"));
        assert!(s.contains("used 0.500s ago"));
    }

    #[test]
    fn display_empty_actions_shows_drop() {
        let flow = DpifFlow {
            key: "in_port(1)".to_owned(),
            mask: None,
            actions: String::new(),
            packets: 0,
            bytes: 0,
            used: None,
        };
        let s = flow.to_string();
        assert!(s.contains("=> drop"));
        assert!(s.contains("never used"));
    }

    #[test]
    fn display_single_packet() {
        let flow = DpifFlow {
            key: "in_port(1)".to_owned(),
            mask: None,
            actions: "2".to_owned(),
            packets: 1,
            bytes: 64,
            used: Some(0.001),
        };
        let s = flow.to_string();
        assert!(s.contains("1 pkt,"));
    }

    #[test]
    fn format_bytes_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1_500_000), "1.4 MB");
        assert_eq!(format_bytes(2_000_000_000), "1.9 GB");
    }
}
