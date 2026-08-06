//! Observation DNS : extrait les réponses DNS des paquets NFQUEUE et alimente le cache.
//! DNS observation: extracts DNS responses from NFQUEUE packets and feeds the cache.

use std::sync::Arc;

use etherparse::{SlicedPacket, TransportSlice};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use syswall_domain::errors::DomainError;

use crate::dns::snooper::{parse_dns_response, DnsSnoopCache};

/// Extrait le payload UDP d'un paquet IP (couche 3). Retourne None si non-UDP ou tronqué.
/// Extract the UDP payload from an IP packet (layer 3). Returns None if non-UDP or truncated.
///
/// Le payload est copié (les paquets DNS sont petits) pour ne pas emprunter le parse local.
/// The payload is copied (DNS packets are small) to avoid borrowing the local parse.
pub fn extract_udp_payload(packet: &[u8]) -> Option<Vec<u8>> {
    let parsed = SlicedPacket::from_ip(packet).ok()?;
    match parsed.transport {
        Some(TransportSlice::Udp(u)) => Some(u.payload().to_vec()),
        _ => None,
    }
}

/// Parse un paquet (couche 3) comme réponse DNS et insère chaque A/AAAA dans le cache.
/// Parse a (layer-3) packet as a DNS response and insert each A/AAAA into the cache.
/// Retourne le nombre d'associations insérées. / Returns the number of mappings inserted.
pub fn ingest_dns_packet(packet: &[u8], cache: &DnsSnoopCache) -> usize {
    let Some(payload) = extract_udp_payload(packet) else {
        return 0;
    };
    let records = parse_dns_response(&payload);
    let n = records.len();
    for (host, ip, ttl) in records {
        cache.insert(ip, host, Some(ttl));
    }
    n
}

/// Ouvre la queue `queue_num`, ingère chaque réponse DNS puis verdict ACCEPT (toujours).
/// Boucle bloquante jusqu'à annulation — à lancer via `spawn_blocking`.
/// Open queue `queue_num`, ingest each DNS response then verdict ACCEPT (always).
/// Blocking loop until cancellation — launch via `spawn_blocking`.
pub fn run_dns_observer(
    queue_num: u16,
    cache: Arc<DnsSnoopCache>,
    cancel: CancellationToken,
) -> Result<(), DomainError> {
    let mut queue = nfq::Queue::open()
        .map_err(|e| DomainError::Infrastructure(format!("dns nfq::open: {e}")))?;
    queue
        .bind(queue_num)
        .map_err(|e| DomainError::Infrastructure(format!("dns nfq::bind({queue_num}): {e}")))?;
    // Mode non-bloquant pour pouvoir sonder le token d'annulation.
    // Non-blocking mode to be able to poll the cancel token.
    queue.set_nonblocking(true);

    info!(target: "dns_observe", queue_num, "queue d'observation DNS ouverte et liée");

    loop {
        if cancel.is_cancelled() {
            info!(target: "dns_observe", queue_num, "token annulé, fermeture de la queue DNS");
            break;
        }

        let mut msg = match queue.recv() {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
            // EINTR (signal reçu) : réessayer. / EINTR (signal received): retry.
            Err(e) if e.raw_os_error() == Some(4) => continue,
            Err(e) => {
                error!(target: "dns_observe", error = %e, "erreur recv nfq DNS");
                return Err(DomainError::Infrastructure(format!("dns nfq::recv: {e}")));
            }
        };

        // Best-effort : une erreur d'ingestion ne doit jamais bloquer le DNS.
        // Best-effort: an ingestion error must never block DNS.
        let _ = ingest_dns_packet(msg.get_payload(), &cache);
        msg.set_verdict(nfq::Verdict::Accept);
        if let Err(e) = queue.verdict(msg) {
            error!(target: "dns_observe", error = %e, "erreur verdict nfq DNS");
            return Err(DomainError::Infrastructure(format!("dns nfq::verdict: {e}")));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    /// Construit un paquet IPv4/UDP (sport 53) encapsulant `dns_payload`.
    /// Build an IPv4/UDP (sport 53) packet wrapping `dns_payload`.
    fn ipv4_udp_dns(dns_payload: &[u8]) -> Vec<u8> {
        let builder =
            etherparse::PacketBuilder::ipv4([8, 8, 8, 8], [10, 0, 0, 1], 64).udp(53, 54321);
        let mut out = Vec::new();
        builder.write(&mut out, dns_payload).unwrap();
        out
    }

    /// Réponse DNS A pour example.com → 93.184.216.34 (repris du test de snooper.rs).
    /// DNS A answer for example.com → 93.184.216.34.
    #[rustfmt::skip]
    fn dns_a_response() -> Vec<u8> {
        vec![
            0x00, 0x01, 0x81, 0x80, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
            0x07, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 0x03, b'c', b'o', b'm', 0x00,
            0x00, 0x01, 0x00, 0x01,
            0xC0, 0x0C, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x01, 0x2C, 0x00, 0x04,
            93, 184, 216, 34,
        ]
    }

    #[test]
    fn ingest_populates_cache_from_ipv4_udp_dns() {
        let cache = DnsSnoopCache::new(300);
        let packet = ipv4_udp_dns(&dns_a_response());
        let n = ingest_dns_packet(&packet, &cache);
        assert_eq!(n, 1);
        let ip: IpAddr = "93.184.216.34".parse().unwrap();
        assert_eq!(cache.get(&ip), Some("example.com".to_string()));
    }

    #[test]
    fn ingest_ignores_non_udp() {
        let cache = DnsSnoopCache::new(300);
        let tcp = etherparse::PacketBuilder::ipv4([8, 8, 8, 8], [10, 0, 0, 1], 64)
            .tcp(53, 54321, 0, 1024);
        let mut packet = Vec::new();
        tcp.write(&mut packet, &[]).unwrap();
        assert_eq!(ingest_dns_packet(&packet, &cache), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn ingest_ignores_garbage() {
        let cache = DnsSnoopCache::new(300);
        assert_eq!(ingest_dns_packet(&[0xff, 0x00, 0x01], &cache), 0);
    }
}
