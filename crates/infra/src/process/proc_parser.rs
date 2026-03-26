use std::net::IpAddr;

/// A parsed entry from /proc/net/tcp or /proc/net/udp.
/// Une entree parsee de /proc/net/tcp ou /proc/net/udp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcNetEntry {
    pub local_ip: IpAddr,
    pub local_port: u16,
    pub remote_ip: IpAddr,
    pub remote_port: u16,
    pub state: u8,
    pub uid: u32,
    pub inode: u64,
}

/// Parsed fields from /proc/<pid>/status.
/// Champs parses de /proc/<pid>/status.
#[derive(Debug, Clone)]
pub struct ProcStatus {
    pub name: String,
    pub uid: u32,
}

/// Parse /proc/net/tcp content into entries.
/// Analyse le contenu de /proc/net/tcp en entrees.
pub fn parse_proc_net_tcp(content: &str) -> Vec<ProcNetEntry> {
    parse_proc_net(content, false)
}

/// Parse /proc/net/tcp6 content into entries.
/// Analyse le contenu de /proc/net/tcp6 en entrees.
pub fn parse_proc_net_tcp6(content: &str) -> Vec<ProcNetEntry> {
    parse_proc_net(content, true)
}

/// Parse /proc/net/udp content into entries.
/// Analyse le contenu de /proc/net/udp en entrees.
#[allow(dead_code)]
pub fn parse_proc_net_udp(content: &str) -> Vec<ProcNetEntry> {
    parse_proc_net(content, false)
}

fn parse_proc_net(content: &str, is_v6: bool) -> Vec<ProcNetEntry> {
    content
        .lines()
        .skip(1) // skip header
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 {
                return None;
            }

            let (local_ip, local_port) = parse_addr_port(fields[1], is_v6)?;
            let (remote_ip, remote_port) = parse_addr_port(fields[2], is_v6)?;
            let state = u8::from_str_radix(fields[3], 16).ok()?;
            let uid: u32 = fields[7].parse().ok()?;
            let inode: u64 = fields[9].parse().ok()?;

            Some(ProcNetEntry {
                local_ip,
                local_port,
                remote_ip,
                remote_port,
                state,
                uid,
                inode,
            })
        })
        .collect()
}

/// Parse a hex address:port string like "0100007F:0050".
/// Analyse une chaine adresse:port hexadecimale comme "0100007F:0050".
fn parse_addr_port(s: &str, is_v6: bool) -> Option<(IpAddr, u16)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let ip = if is_v6 {
        parse_hex_ipv6(parts[0])?
    } else {
        parse_hex_ip(parts[0])?
    };
    let port = u16::from_str_radix(parts[1], 16).ok()?;

    Some((ip, port))
}

/// Parse a hex-encoded IPv4 address (little-endian).
/// Analyse une adresse IPv4 encodee en hexadecimal (little-endian).
pub fn parse_hex_ip(hex: &str) -> Option<IpAddr> {
    if hex.len() != 8 {
        return None;
    }
    let val = u32::from_str_radix(hex, 16).ok()?;
    // /proc/net/tcp uses host byte order (little-endian on x86)
    let ip = std::net::Ipv4Addr::from(val.to_be());
    Some(IpAddr::V4(ip))
}

/// Parse a hex-encoded IPv6 address from /proc/net/tcp6.
/// Analyse une adresse IPv6 encodee en hexadecimal de /proc/net/tcp6.
pub fn parse_hex_ipv6(hex: &str) -> Option<IpAddr> {
    if hex.len() != 32 {
        return None;
    }
    // /proc/net/tcp6 stores as 4 groups of 32-bit values in host byte order.
    // The hex string represents the raw memory bytes, so each group is in native endianness.
    // We byte-swap each group to get network byte order (big-endian) for Ipv6Addr.
    let mut octets = [0u8; 16];
    for i in 0..4 {
        let group_hex = &hex[i * 8..(i + 1) * 8];
        let val = u32::from_str_radix(group_hex, 16).ok()?;
        // Same swap as IPv4: the hex is host-order, we need network order
        let swapped = val.to_be();
        let bytes = swapped.to_be_bytes();
        octets[i * 4] = bytes[0];
        octets[i * 4 + 1] = bytes[1];
        octets[i * 4 + 2] = bytes[2];
        octets[i * 4 + 3] = bytes[3];
    }
    Some(IpAddr::V6(std::net::Ipv6Addr::from(octets)))
}

/// Parse /proc/<pid>/status content.
/// Analyse le contenu de /proc/<pid>/status.
pub fn parse_proc_status(content: &str) -> Option<ProcStatus> {
    let mut name = None;
    let mut uid = None;

    for line in content.lines() {
        if let Some(n) = line.strip_prefix("Name:\t") {
            name = Some(n.to_string());
        } else if let Some(uid_line) = line.strip_prefix("Uid:\t") {
            // Format: real effective saved fs
            if let Some(real_uid) = uid_line.split_whitespace().next() {
                uid = real_uid.parse().ok();
            }
        }
    }

    Some(ProcStatus {
        name: name?,
        uid: uid?,
    })
}

/// Parse /proc/<pid>/cmdline bytes (NUL-separated).
/// Analyse les octets de /proc/<pid>/cmdline (separes par NUL).
pub fn parse_cmdline(bytes: &[u8]) -> String {
    bytes
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse /proc/<pid>/cmdline bytes, returning None if empty.
/// Analyse les octets de /proc/<pid>/cmdline, retournant None si vide.
pub fn parse_cmdline_opt(bytes: &[u8]) -> Option<String> {
    let result = parse_cmdline(bytes);
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_proc_net_tcp_single_entry() {
        let content = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 0100007F:0050 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 12345 1 0000000000000000 100 0 0 10 0\n";
        let entries = parse_proc_net_tcp(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].local_ip, "127.0.0.1".parse::<IpAddr>().unwrap());
        assert_eq!(entries[0].local_port, 80);
        assert_eq!(entries[0].inode, 12345);
        assert_eq!(entries[0].uid, 0);
    }

    #[test]
    fn parse_hex_ip_loopback() {
        let ip = parse_hex_ip("0100007F").unwrap();
        assert_eq!(ip, "127.0.0.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn parse_hex_ip_invalid_length() {
        assert!(parse_hex_ip("0100").is_none());
    }

    #[test]
    fn parse_proc_status_name_and_uid() {
        let content = "Name:\tfirefox\nUmask:\t0022\nState:\tS (sleeping)\nTgid:\t1234\nNgid:\t0\nPid:\t1234\nPPid:\t1000\nUid:\t1000\t1000\t1000\t1000\nGid:\t1000\t1000\t1000\t1000\n";
        let info = parse_proc_status(content).unwrap();
        assert_eq!(info.name, "firefox");
        assert_eq!(info.uid, 1000);
    }

    #[test]
    fn parse_cmdline_with_args() {
        let bytes = b"firefox\0--no-remote\0https://example.com\0";
        let cmdline = parse_cmdline(bytes);
        assert_eq!(cmdline, "firefox --no-remote https://example.com");
    }

    #[test]
    fn parse_empty_cmdline() {
        assert!(parse_cmdline_opt(b"").is_none());
    }

    #[test]
    fn parse_proc_net_tcp6_loopback() {
        let content = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 00000000000000000000000001000000:0050 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 12345 1 0000000000000000 100 0 0 10 0\n";
        let entries = parse_proc_net_tcp6(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].local_ip, "::1".parse::<IpAddr>().unwrap());
        assert_eq!(entries[0].local_port, 80);
    }

    #[test]
    fn parse_proc_net_tcp_empty() {
        let content = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n";
        let entries = parse_proc_net_tcp(content);
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_proc_status_missing_name_returns_none() {
        let content = "Uid:\t1000\t1000\t1000\t1000\n";
        assert!(parse_proc_status(content).is_none());
    }
}
