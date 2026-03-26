use std::net::IpAddr;

use syswall_domain::value_objects::Protocol;

use super::types::{ConntrackEvent, ConntrackEventType};

/// Parse a single conntrack event output line into a ConntrackEvent.
/// Returns None if the line cannot be parsed.
///
/// Analyse une seule ligne de sortie d'evenement conntrack en ConntrackEvent.
/// Retourne None si la ligne ne peut pas etre parsee.
pub fn parse_conntrack_line(line: &str) -> Option<ConntrackEvent> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Parse timestamp: [1711468800.123456]
    let ts_end = line.find(']')?;
    let ts_str = &line[1..ts_end];
    let timestamp: f64 = ts_str.parse().ok()?;

    let rest = &line[ts_end + 1..];

    // Tokenize the rest
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.len() < 5 {
        return None;
    }

    // Find event type: [NEW], [UPDATE], [DESTROY]
    let mut event_type = None;
    let mut event_type_idx = 0;
    for (i, token) in tokens.iter().enumerate() {
        if let Some(et) = parse_event_type(token) {
            event_type = Some(et);
            event_type_idx = i;
            break;
        }
    }
    let event_type = event_type?;

    // Protocol is next token after event type
    let proto_idx = event_type_idx + 1;
    if proto_idx >= tokens.len() {
        return None;
    }
    let protocol = parse_protocol(tokens[proto_idx])?;

    // Protocol number is next
    let proto_num_idx = proto_idx + 1;
    let proto_number: u8 = if proto_num_idx < tokens.len() {
        tokens[proto_num_idx].parse().unwrap_or(0)
    } else {
        0
    };

    // Find state: known TCP states appearing before first key=value pair
    let kv_tokens = &tokens[proto_num_idx + 1..];
    let mut state = None;
    let known_states = [
        "SYN_SENT",
        "SYN_RECV",
        "ESTABLISHED",
        "FIN_WAIT",
        "CLOSE_WAIT",
        "LAST_ACK",
        "TIME_WAIT",
        "CLOSE",
        "LISTEN",
    ];

    for token in kv_tokens {
        if known_states.contains(token) {
            state = Some(token.to_string());
            break;
        }
        // Stop looking once we hit key=value pairs
        if token.contains('=') {
            break;
        }
    }

    // Extract key=value pairs -- there are two sets separated by [UNREPLIED] or similar markers
    // First set is the original direction, second set is the reply
    let all_kv: Vec<&str> = tokens.iter().copied().filter(|t| t.contains('=')).collect();

    // First occurrence of src, dst, sport, dport
    let src_str = extract_kv_from_list(&all_kv, "src", 0)?;
    let dst_str = extract_kv_from_list(&all_kv, "dst", 0)?;
    let sport_str = extract_kv_from_list(&all_kv, "sport", 0)?;
    let dport_str = extract_kv_from_list(&all_kv, "dport", 0)?;

    let src: IpAddr = src_str.parse().ok()?;
    let dst: IpAddr = dst_str.parse().ok()?;
    let sport: u16 = sport_str.parse().ok()?;
    let dport: u16 = dport_str.parse().ok()?;

    // Second occurrence is the reply direction
    let reply_src = extract_kv_from_list(&all_kv, "src", 1).and_then(|s| s.parse::<IpAddr>().ok());
    let reply_dst = extract_kv_from_list(&all_kv, "dst", 1).and_then(|s| s.parse::<IpAddr>().ok());
    let reply_sport =
        extract_kv_from_list(&all_kv, "sport", 1).and_then(|s| s.parse::<u16>().ok());
    let reply_dport =
        extract_kv_from_list(&all_kv, "dport", 1).and_then(|s| s.parse::<u16>().ok());

    Some(ConntrackEvent {
        timestamp,
        event_type,
        protocol,
        proto_number,
        state,
        src,
        dst,
        sport,
        dport,
        reply_src,
        reply_dst,
        reply_sport,
        reply_dport,
    })
}

/// Parse the event type token ([NEW], [UPDATE], [DESTROY]).
/// Analyse le jeton de type d'evenement ([NEW], [UPDATE], [DESTROY]).
fn parse_event_type(token: &str) -> Option<ConntrackEventType> {
    match token {
        "[NEW]" => Some(ConntrackEventType::New),
        "[UPDATE]" => Some(ConntrackEventType::Update),
        "[DESTROY]" => Some(ConntrackEventType::Destroy),
        _ => None,
    }
}

/// Parse a protocol name to our domain Protocol enum.
/// Analyse un nom de protocole vers notre enum Protocol du domaine.
fn parse_protocol(name: &str) -> Option<Protocol> {
    match name {
        "tcp" => Some(Protocol::Tcp),
        "udp" => Some(Protocol::Udp),
        "icmp" => Some(Protocol::Icmp),
        _ => None,
    }
}

/// Extract the Nth occurrence of a key=value pair from the token list.
/// Extrait la Nieme occurrence d'une paire cle=valeur depuis la liste de jetons.
fn extract_kv_from_list<'a>(tokens: &[&'a str], key: &str, occurrence: usize) -> Option<&'a str> {
    let prefix = format!("{}=", key);
    tokens
        .iter()
        .filter(|t| t.starts_with(&prefix))
        .nth(occurrence)
        .map(|t| &t[prefix.len()..])
}

/// Extract a key=value pair from the token list (first occurrence).
/// Extrait une paire cle=valeur depuis la liste de jetons (premiere occurrence).
#[allow(dead_code)]
fn extract_kv<'a>(tokens: &'a [&str], key: &str) -> Option<&'a str> {
    extract_kv_from_list(tokens, key, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_new_tcp_event() {
        let line = "[1711468800.123456]      [NEW] tcp      6 120 SYN_SENT src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 [UNREPLIED] src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.event_type, ConntrackEventType::New);
        assert_eq!(event.protocol, Protocol::Tcp);
        assert_eq!(event.src, "192.168.1.100".parse::<IpAddr>().unwrap());
        assert_eq!(event.dst, "93.184.216.34".parse::<IpAddr>().unwrap());
        assert_eq!(event.sport, 45000);
        assert_eq!(event.dport, 443);
        assert!((event.timestamp - 1711468800.123456).abs() < 0.001);
    }

    #[test]
    fn parse_destroy_event() {
        let line = "[1711468800.345678]  [DESTROY] tcp      6 src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.event_type, ConntrackEventType::Destroy);
        assert!(event.state.is_none());
    }

    #[test]
    fn parse_update_established() {
        let line = "[1711468800.234567]   [UPDATE] tcp      6 60 ESTABLISHED src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.event_type, ConntrackEventType::Update);
        assert_eq!(event.state, Some("ESTABLISHED".to_string()));
    }

    #[test]
    fn parse_udp_event() {
        let line = "[1711468800.456789]      [NEW] udp      17 30 src=192.168.1.100 dst=8.8.8.8 sport=52000 dport=53 [UNREPLIED] src=8.8.8.8 dst=192.168.1.100 sport=53 dport=52000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.protocol, Protocol::Udp);
        assert_eq!(event.proto_number, 17);
        assert_eq!(event.dport, 53);
    }

    #[test]
    fn malformed_line_returns_none() {
        assert!(parse_conntrack_line("garbage data").is_none());
    }

    #[test]
    fn missing_port_returns_none() {
        let line =
            "[1711468800.123456]      [NEW] tcp      6 120 SYN_SENT src=192.168.1.100 dst=93.184.216.34";
        assert!(parse_conntrack_line(line).is_none());
    }

    #[test]
    fn ipv6_addresses_parsed() {
        let line = "[1711468800.123456]      [NEW] tcp      6 120 SYN_SENT src=::1 dst=::1 sport=45000 dport=8080 [UNREPLIED] src=::1 dst=::1 sport=8080 dport=45000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.src, "::1".parse::<IpAddr>().unwrap());
        assert_eq!(event.dst, "::1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn empty_line_returns_none() {
        assert!(parse_conntrack_line("").is_none());
    }

    #[test]
    fn reply_addresses_parsed() {
        let line = "[1711468800.123456]      [NEW] tcp      6 120 SYN_SENT src=192.168.1.100 dst=93.184.216.34 sport=45000 dport=443 [UNREPLIED] src=93.184.216.34 dst=192.168.1.100 sport=443 dport=45000";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(
            event.reply_src,
            Some("93.184.216.34".parse::<IpAddr>().unwrap())
        );
        assert_eq!(
            event.reply_dst,
            Some("192.168.1.100".parse::<IpAddr>().unwrap())
        );
        assert_eq!(event.reply_sport, Some(443));
        assert_eq!(event.reply_dport, Some(45000));
    }
}
