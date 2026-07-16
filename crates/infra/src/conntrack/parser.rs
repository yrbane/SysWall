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

    // Deux formats sont acceptés :
    //   - `conntrack -E` (événements) : préfixe horodatage `[epoch.usec]` puis type `[NEW]`/…
    //   - `conntrack -L -o extended` (instantané) : PAS d'horodatage ni de type, mais un label L3
    //     (`ipv4 2` / `ipv6 10`) précède le protocole L4.
    // On consomme un horodatage entre crochets seulement s'il parse en flottant ; sinon toute la
    // ligne est le corps. Le type d'événement est optionnel (défaut New pour les entrées actives).
    // Two formats are accepted:
    //   - `conntrack -E` (events): `[epoch.usec]` timestamp prefix then a `[NEW]`/… event type
    //   - `conntrack -L -o extended` (snapshot): NO timestamp nor event type, but an L3 label
    //     (`ipv4 2` / `ipv6 10`) precedes the L4 protocol.
    // We consume a bracketed timestamp only when it parses as a float; otherwise the whole line is
    // the body. The event type is optional (defaults to New for active/snapshot entries).
    let (timestamp, body) = match line.strip_prefix('[') {
        Some(after) => match after.split_once(']') {
            Some((maybe_ts, rest)) => match maybe_ts.parse::<f64>() {
                Ok(ts) => (ts, rest),
                Err(_) => (0.0, line),
            },
            None => (0.0, line),
        },
        None => (0.0, line),
    };

    // Tokenize the body
    let tokens: Vec<&str> = body.split_whitespace().collect();
    if tokens.len() < 5 {
        return None;
    }

    // Event type: [NEW]/[UPDATE]/[DESTROY] en mode `-E`, absent en mode `-L` (défaut New).
    // Event type: [NEW]/[UPDATE]/[DESTROY] in `-E` mode, absent in `-L` mode (defaults to New).
    let event_type = tokens
        .iter()
        .find_map(|token| parse_event_type(token))
        .unwrap_or(ConntrackEventType::New);

    // Protocole L4 repéré par nom (tcp/udp/icmp) : robuste au label L3 optionnel de `-o extended`.
    // L4 protocol located by name (tcp/udp/icmp): robust to the optional `-o extended` L3 label.
    let proto_idx = tokens.iter().position(|t| parse_protocol(t).is_some())?;
    let protocol = parse_protocol(tokens[proto_idx])?;

    // Protocol number is the next token
    let proto_num_idx = proto_idx + 1;
    let proto_number: u8 = if proto_num_idx < tokens.len() {
        tokens[proto_num_idx].parse().unwrap_or(0)
    } else {
        0
    };

    // Find state: known TCP states appearing before first key=value pair
    let kv_tokens = tokens.get(proto_num_idx + 1..).unwrap_or(&[]);
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
    let reply_sport = extract_kv_from_list(&all_kv, "sport", 1).and_then(|s| s.parse::<u16>().ok());
    let reply_dport = extract_kv_from_list(&all_kv, "dport", 1).and_then(|s| s.parse::<u16>().ok());

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
        let line = "[1711468800.123456]      [NEW] tcp      6 120 SYN_SENT src=192.168.1.100 dst=93.184.216.34";
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
    fn parse_list_extended_ipv6_line() {
        // VRAIE ligne `conntrack -L -o extended` en IPv6 : pas d'horodatage entre crochets ni de
        // type d'événement ([NEW]/…), mais un label L3 (`ipv6 10`) précède le protocole L4. Le
        // parser doit extraire adresses v6, protocole et ports correctement.
        // REAL `conntrack -L -o extended` IPv6 line: no bracketed timestamp nor event type
        // ([NEW]/…), but an L3 label (`ipv6 10`) precedes the L4 protocol. The parser must extract
        // the v6 addresses, protocol and ports correctly.
        let line = "ipv6     10 tcp      6 431999 ESTABLISHED src=2001:db8::1 dst=2001:db8::2 sport=45000 dport=443 src=2001:db8::2 dst=2001:db8::1 sport=443 dport=45000 [ASSURED] mark=0 use=1";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.protocol, Protocol::Tcp);
        assert_eq!(event.proto_number, 6);
        assert_eq!(event.src, "2001:db8::1".parse::<IpAddr>().unwrap());
        assert_eq!(event.dst, "2001:db8::2".parse::<IpAddr>().unwrap());
        assert!(event.src.is_ipv6());
        assert_eq!(event.sport, 45000);
        assert_eq!(event.dport, 443);
        assert_eq!(event.state, Some("ESTABLISHED".to_string()));
        assert_eq!(
            event.reply_src,
            Some("2001:db8::2".parse::<IpAddr>().unwrap())
        );
        assert_eq!(event.reply_dport, Some(45000));
    }

    #[test]
    fn parse_list_extended_ipv6_udp_line() {
        // Ligne `-L -o extended` UDP IPv6 sans marqueur [ASSURED] : couvre le cas sans aucun
        // crochet dans la ligne.
        // `-L -o extended` IPv6 UDP line without an [ASSURED] marker: covers the case where the
        // line has no bracket at all.
        let line = "ipv6     10 udp      17 29 src=2001:db8::a dst=2001:db8::b sport=52000 dport=53 src=2001:db8::b dst=2001:db8::a sport=53 dport=52000 mark=0 use=1";
        let event = parse_conntrack_line(line).unwrap();
        assert_eq!(event.protocol, Protocol::Udp);
        assert_eq!(event.proto_number, 17);
        assert_eq!(event.src, "2001:db8::a".parse::<IpAddr>().unwrap());
        assert_eq!(event.dport, 53);
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
