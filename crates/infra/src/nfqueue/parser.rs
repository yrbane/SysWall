//! Parsing de paquets : octets bruts NFQUEUE -> Connection domain.
//! Packet parsing: raw bytes from NFQUEUE -> domain Connection.

use etherparse::{NetSlice, SlicedPacket, TransportSlice};
use std::net::IpAddr;
use thiserror::Error;

use syswall_domain::entities::connection::{
    Connection, ConnectionId, ConnectionState, ConnectionVerdict,
};
use syswall_domain::errors::DomainError;
use syswall_domain::value_objects::{Direction, Port, Protocol, SocketAddress};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("paquet trop court ou malformé")]
    Malformed,
    #[error("protocole L3 non supporté")]
    UnsupportedL3,
    #[error("protocole L4 non supporté")]
    UnsupportedL4,
    #[error("port invalide (zéro) dans l'en-tête")]
    InvalidPort,
    #[error("etherparse: {0}")]
    Etherparse(String),
}

impl From<ParseError> for DomainError {
    fn from(e: ParseError) -> Self {
        DomainError::Validation(format!("packet parse: {e}"))
    }
}

/// Parse un paquet brut (commençant à l'en-tête IP) en Connection domain.
/// Parse a raw packet (starting at the IP header) into a domain Connection.
pub fn parse_packet(bytes: &[u8]) -> Result<Connection, ParseError> {
    // Pré-check de troncature famille-conscient : on lit le nibble de version (4 bits de poids
    // fort du 1er octet) pour exiger la taille minimale du bon en-tête — 20 octets pour IPv4,
    // 40 pour IPv6. etherparse revalide entièrement derrière ; ce garde reste purement défensif.
    // Family-aware truncation pre-check: read the version nibble (high 4 bits of the first byte)
    // to require the correct minimum header size — 20 bytes for IPv4, 40 for IPv6. etherparse
    // fully re-validates afterwards; this guard stays purely defensive.
    match bytes.first().map(|&b| b >> 4) {
        Some(4) if bytes.len() >= 20 => {}
        Some(6) if bytes.len() >= 40 => {}
        _ => return Err(ParseError::Malformed),
    }
    let parsed = SlicedPacket::from_ip(bytes).map_err(|e| ParseError::Etherparse(e.to_string()))?;

    let (src_ip, dst_ip): (IpAddr, IpAddr) = match &parsed.net {
        Some(NetSlice::Ipv4(h)) => {
            let header = h.header();
            (
                IpAddr::V4(header.source_addr()),
                IpAddr::V4(header.destination_addr()),
            )
        }
        Some(NetSlice::Ipv6(h)) => {
            let header = h.header();
            (
                IpAddr::V6(header.source_addr()),
                IpAddr::V6(header.destination_addr()),
            )
        }
        _ => return Err(ParseError::UnsupportedL3),
    };

    // Seuls TCP et UDP sont décodés ici : ce sont les seuls protocoles pour lesquels une décision
    // par-connexion (par port) a du sens dans NFQUEUE. ICMP, ICMPv6 et NDP (Neighbor Discovery)
    // ne sont volontairement PAS interceptés au niveau paquet — parité stricte avec ICMPv4, hors
    // périmètre de la décision par-connexion. Aucune variante Protocol::Icmpv6 n'est introduite.
    // Only TCP and UDP are decoded here: they are the only protocols for which a per-connection
    // (per-port) decision is meaningful in NFQUEUE. ICMP, ICMPv6 and NDP (Neighbor Discovery) are
    // intentionally NOT intercepted at packet level — strict parity with ICMPv4, out of scope for
    // per-connection decisions. No Protocol::Icmpv6 variant is introduced.
    let (protocol, src_port_raw, dst_port_raw) = match &parsed.transport {
        Some(TransportSlice::Tcp(t)) => (Protocol::Tcp, t.source_port(), t.destination_port()),
        Some(TransportSlice::Udp(u)) => (Protocol::Udp, u.source_port(), u.destination_port()),
        _ => return Err(ParseError::UnsupportedL4),
    };

    // Port::new rejette 0 — on traite le cas limite comme une erreur de parsing.
    // Port::new rejects 0 — treat this edge case as a parse error.
    let src_port = Port::new(src_port_raw).map_err(|_| ParseError::InvalidPort)?;
    let dst_port = Port::new(dst_port_raw).map_err(|_| ParseError::InvalidPort)?;

    let source = SocketAddress::new(src_ip, src_port);
    let destination = SocketAddress::new(dst_ip, dst_port);

    // On suppose un trafic sortant : la source est locale, la destination est distante.
    // Les couches supérieures (HybridProcessResolver) enrichissent le PID et la direction.
    // Assumes outbound traffic: source is local, destination is remote.
    // Upper layers (HybridProcessResolver) enrich PID and direction.
    Ok(Connection {
        id: ConnectionId::new(),
        protocol,
        source,
        destination,
        direction: Direction::Outbound,
        state: ConnectionState::New,
        process: None,
        user: None,
        bytes_sent: 0,
        bytes_received: 0,
        started_at: chrono::Utc::now(),
        verdict: ConnectionVerdict::PendingDecision,
        matched_rule: None,
        remote_hostname: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_ipv4_tcp() -> Vec<u8> {
        let mut buffer = Vec::new();
        let builder = etherparse::PacketBuilder::ipv4([10, 0, 0, 1], [1, 2, 3, 4], 64)
            .tcp(12345, 443, 0, 1024);
        let payload: &[u8] = &[];
        builder
            .write(&mut buffer, payload)
            .expect("etherparse builder ok");
        buffer
    }

    fn build_ipv4_udp() -> Vec<u8> {
        let mut buffer = Vec::new();
        let builder =
            etherparse::PacketBuilder::ipv4([10, 0, 0, 1], [1, 1, 1, 1], 64).udp(54321, 53);
        let payload: &[u8] = b"\x00\x00";
        builder
            .write(&mut buffer, payload)
            .expect("etherparse builder ok");
        buffer
    }

    fn build_ipv6_udp() -> Vec<u8> {
        let mut buffer = Vec::new();
        let builder = etherparse::PacketBuilder::ipv6(
            [
                0x20, 0x01, 0xdb, 0x08, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
            ],
            [
                0x20, 0x01, 0xdb, 0x08, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
            ],
            64,
        )
        .udp(12345, 53);
        let payload: &[u8] = b"\x12\x34";
        builder
            .write(&mut buffer, payload)
            .expect("etherparse builder ok");
        buffer
    }

    #[test]
    fn parses_ipv4_tcp() {
        let bytes = build_ipv4_tcp();
        let conn = parse_packet(&bytes).expect("parse ok");
        assert_eq!(conn.protocol, Protocol::Tcp);
        assert_eq!(conn.destination.port.value(), 443);
        assert_eq!(conn.source.port.value(), 12345);
        assert_eq!(conn.direction, Direction::Outbound);
        assert_eq!(conn.state, ConnectionState::New);
        assert!(conn.process.is_none());
    }

    #[test]
    fn parses_ipv4_udp() {
        let bytes = build_ipv4_udp();
        let conn = parse_packet(&bytes).expect("parse ok");
        assert_eq!(conn.protocol, Protocol::Udp);
        assert_eq!(conn.destination.port.value(), 53);
        assert_eq!(conn.source.port.value(), 54321);
    }

    #[test]
    fn parses_ipv6_udp() {
        let bytes = build_ipv6_udp();
        let conn = parse_packet(&bytes).expect("parse ok");
        assert_eq!(conn.protocol, Protocol::Udp);
        assert_eq!(conn.destination.port.value(), 53);
        assert!(conn.source.ip.is_ipv6());
    }

    #[test]
    fn rejects_truncated_packet() {
        let result = parse_packet(&[0u8; 4]);
        assert!(result.is_err());
    }

    #[test]
    fn parses_ipv6_truncated_returns_malformed() {
        // L'en-tête IPv6 fait 40 octets. Un paquet dont le nibble de version vaut 6 mais tronqué
        // à 20-39 octets doit être rejeté proprement (Malformed) : pas de panique, pas de faux
        // parse. Vérifie que le pré-check de troncature est bien famille-conscient.
        // The IPv6 header is 40 bytes. A packet whose version nibble is 6 but truncated to 20-39
        // bytes must be cleanly rejected (Malformed): no panic, no false parse. Ensures the
        // truncation pre-check is family-aware.
        let full = build_ipv6_udp();
        assert!(full.len() >= 40, "fixture v6 doit dépasser l'en-tête");
        for len in [20usize, 30, 39] {
            let truncated = &full[..len];
            // Premier octet 0x60 -> version 6 ; longueur < 40 -> Malformed.
            // First byte 0x60 -> version 6; length < 40 -> Malformed.
            assert!(
                matches!(parse_packet(truncated), Err(ParseError::Malformed)),
                "un paquet v6 de {len} octets doit être Malformed"
            );
        }
    }
}
