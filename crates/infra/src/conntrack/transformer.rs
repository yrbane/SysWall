use std::net::IpAddr;

use chrono::Utc;

use syswall_domain::entities::{Connection, ConnectionId, ConnectionState, ConnectionVerdict};
use syswall_domain::value_objects::{Direction, Port, SocketAddress};

use super::types::{ConntrackEvent, ConntrackEventType};

/// Transform a raw ConntrackEvent into a domain Connection.
/// The connection is created with process=None and verdict=Unknown.
/// Process resolution and policy evaluation happen downstream.
///
/// Transforme un ConntrackEvent brut en Connection du domaine.
/// La connexion est creee avec process=None et verdict=Unknown.
/// La resolution de processus et l'evaluation de politique se font en aval.
pub fn conntrack_to_connection(event: ConntrackEvent, local_ips: &[IpAddr]) -> Option<Connection> {
    let source_port = Port::new(event.sport).ok()?;
    let dest_port = Port::new(event.dport).ok()?;

    let direction = if local_ips.contains(&event.src) {
        Direction::Outbound
    } else if local_ips.contains(&event.dst) {
        Direction::Inbound
    } else {
        // Neither src nor dst is local -- could be forwarded traffic, default to outbound
        Direction::Outbound
    };

    let state = match event.event_type {
        ConntrackEventType::New => ConnectionState::New,
        ConntrackEventType::Update => match event.state.as_deref() {
            Some("ESTABLISHED") => ConnectionState::Established,
            Some("TIME_WAIT") | Some("CLOSE_WAIT") | Some("LAST_ACK") | Some("CLOSE") => {
                ConnectionState::Closing
            }
            Some("SYN_SENT") | Some("SYN_RECV") => ConnectionState::New,
            _ => ConnectionState::Established,
        },
        ConntrackEventType::Destroy => ConnectionState::Closed,
    };

    Some(Connection {
        id: ConnectionId::new(),
        protocol: event.protocol,
        source: SocketAddress::new(event.src, source_port),
        destination: SocketAddress::new(event.dst, dest_port),
        direction,
        state,
        process: None,
        user: None,
        bytes_sent: 0,
        bytes_received: 0,
        started_at: Utc::now(),
        verdict: ConnectionVerdict::Unknown,
        matched_rule: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use syswall_domain::value_objects::Protocol;

    fn make_event(src: &str, dst: &str) -> ConntrackEvent {
        ConntrackEvent {
            timestamp: 1711468800.0,
            event_type: ConntrackEventType::New,
            protocol: Protocol::Tcp,
            proto_number: 6,
            state: Some("SYN_SENT".to_string()),
            src: src.parse().unwrap(),
            dst: dst.parse().unwrap(),
            sport: 45000,
            dport: 443,
            reply_src: None,
            reply_dst: None,
            reply_sport: None,
            reply_dport: None,
        }
    }

    #[test]
    fn new_event_becomes_new_connection() {
        let event = make_event("192.168.1.100", "93.184.216.34");
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.state, ConnectionState::New);
        assert_eq!(conn.direction, Direction::Outbound);
        assert_eq!(conn.verdict, ConnectionVerdict::Unknown);
        assert!(conn.process.is_none());
    }

    #[test]
    fn direction_outbound_when_src_is_local() {
        let event = make_event("192.168.1.100", "8.8.8.8");
        let local_ips: Vec<IpAddr> = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.direction, Direction::Outbound);
    }

    #[test]
    fn direction_inbound_when_dst_is_local() {
        let event = make_event("8.8.8.8", "192.168.1.100");
        let local_ips: Vec<IpAddr> = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.direction, Direction::Inbound);
    }

    #[test]
    fn destroy_event_becomes_closed() {
        let mut event = make_event("192.168.1.100", "93.184.216.34");
        event.event_type = ConntrackEventType::Destroy;
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.state, ConnectionState::Closed);
    }

    #[test]
    fn update_established_becomes_established() {
        let mut event = make_event("192.168.1.100", "93.184.216.34");
        event.event_type = ConntrackEventType::Update;
        event.state = Some("ESTABLISHED".to_string());
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.state, ConnectionState::Established);
    }

    #[test]
    fn update_time_wait_becomes_closing() {
        let mut event = make_event("192.168.1.100", "93.184.216.34");
        event.event_type = ConntrackEventType::Update;
        event.state = Some("TIME_WAIT".to_string());
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.state, ConnectionState::Closing);
    }

    #[test]
    fn port_zero_returns_none() {
        let mut event = make_event("192.168.1.100", "93.184.216.34");
        event.sport = 0;
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        assert!(conntrack_to_connection(event, &local_ips).is_none());
    }

    #[test]
    fn connection_has_correct_addresses() {
        let event = make_event("192.168.1.100", "93.184.216.34");
        let local_ips = vec!["192.168.1.100".parse().unwrap()];
        let conn = conntrack_to_connection(event, &local_ips).unwrap();
        assert_eq!(conn.source.ip, "192.168.1.100".parse::<IpAddr>().unwrap());
        assert_eq!(
            conn.destination.ip,
            "93.184.216.34".parse::<IpAddr>().unwrap()
        );
        assert_eq!(conn.source.port.value(), 45000);
        assert_eq!(conn.destination.port.value(), 443);
    }
}
