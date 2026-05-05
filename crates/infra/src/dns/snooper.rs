use std::net::IpAddr;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tracing::debug;

/// Cache d'associations IP → hostname capturées par snooping DNS.
/// Cache of IP → hostname associations captured by DNS snooping.
pub struct DnsSnoopCache {
    /// Associe IP → (hostname, expiration) / Maps IP → (hostname, expiry)
    cache: DashMap<IpAddr, (String, Instant)>,
    /// TTL par défaut si le DNS n'en fournit pas / Default TTL if DNS doesn't provide one
    default_ttl: Duration,
}

impl DnsSnoopCache {
    pub fn new(default_ttl_secs: u64) -> Self {
        Self {
            cache: DashMap::new(),
            default_ttl: Duration::from_secs(default_ttl_secs),
        }
    }

    /// Insère une association IP → hostname avec un TTL donné.
    /// Insert an IP → hostname mapping with a given TTL.
    pub fn insert(&self, ip: IpAddr, hostname: String, ttl_secs: Option<u32>) {
        let ttl = ttl_secs
            .map(|t| Duration::from_secs(t as u64))
            .unwrap_or(self.default_ttl);
        let expiry = Instant::now() + ttl;
        debug!("DNS snoop: {} → {} (TTL {}s)", ip, hostname, ttl.as_secs());
        self.cache.insert(ip, (hostname, expiry));
    }

    /// Cherche le hostname pour une IP. Retourne None si absent ou expiré.
    /// Look up the hostname for an IP. Returns None if absent or expired.
    pub fn get(&self, ip: &IpAddr) -> Option<String> {
        if let Some(entry) = self.cache.get(ip) {
            let (hostname, expiry) = entry.value();
            if Instant::now() < *expiry {
                return Some(hostname.clone());
            }
            // Expiré — on ne supprime pas ici pour éviter le deadlock
            // Expired — don't remove here to avoid deadlock
        }
        None
    }

    /// Nombre d'entrées dans le cache / Number of entries in cache
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Indique si le cache est vide / Returns true if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Nettoie les entrées expirées / Clean expired entries
    pub fn evict_expired(&self) {
        let now = Instant::now();
        self.cache.retain(|_, (_, expiry)| *expiry > now);
    }
}

/// Parse un paquet DNS réponse (wire format) et extrait les A/AAAA records.
/// Parse a DNS response packet (wire format) and extract A/AAAA records.
///
/// Retourne une liste de (hostname, IP, TTL en secondes).
/// Returns a list of (hostname, IP, TTL in seconds).
pub fn parse_dns_response(packet: &[u8]) -> Vec<(String, IpAddr, u32)> {
    let mut results = Vec::new();

    // Le paquet DNS minimum fait 12 bytes (header)
    // Minimum DNS packet is 12 bytes (header)
    if packet.len() < 12 {
        return results;
    }

    // Flags: bit 15 = QR (1 = response), bits 11-14 = opcode, bit 0-3 = RCODE
    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    let is_response = (flags & 0x8000) != 0;
    let rcode = flags & 0x000F;

    if !is_response || rcode != 0 {
        return results; // Pas une réponse ou erreur / Not a response or error
    }

    let qdcount = u16::from_be_bytes([packet[4], packet[5]]) as usize;
    let ancount = u16::from_be_bytes([packet[6], packet[7]]) as usize;

    if ancount == 0 {
        return results;
    }

    // Sauter les questions / Skip questions
    let mut offset = 12;
    for _ in 0..qdcount {
        offset = skip_dns_name(packet, offset);
        if offset == 0 || offset + 4 > packet.len() {
            return results;
        }
        offset += 4; // QTYPE (2) + QCLASS (2)
    }

    // Parser les réponses / Parse answers
    for _ in 0..ancount {
        if offset >= packet.len() {
            break;
        }

        // Lire le nom / Read name
        let name = read_dns_name(packet, offset);
        offset = skip_dns_name(packet, offset);
        if offset == 0 || offset + 10 > packet.len() {
            break;
        }

        let rtype = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
        // rclass at offset+2..offset+4
        let ttl = u32::from_be_bytes([
            packet[offset + 4],
            packet[offset + 5],
            packet[offset + 6],
            packet[offset + 7],
        ]);
        let rdlength = u16::from_be_bytes([packet[offset + 8], packet[offset + 9]]) as usize;
        offset += 10;

        if offset + rdlength > packet.len() {
            break;
        }

        match rtype {
            1 if rdlength == 4 => {
                // A record (IPv4)
                let ip = IpAddr::V4(std::net::Ipv4Addr::new(
                    packet[offset],
                    packet[offset + 1],
                    packet[offset + 2],
                    packet[offset + 3],
                ));
                if let Some(ref hostname) = name {
                    results.push((hostname.clone(), ip, ttl));
                }
            }
            28 if rdlength == 16 => {
                // AAAA record (IPv6)
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&packet[offset..offset + 16]);
                let ip = IpAddr::V6(std::net::Ipv6Addr::from(octets));
                if let Some(ref hostname) = name {
                    results.push((hostname.clone(), ip, ttl));
                }
            }
            _ => {} // CNAME, MX, etc. — on ignore / we skip
        }

        offset += rdlength;
    }

    results
}

/// Saute un nom DNS (avec compression possible). Retourne le nouvel offset.
/// Skip a DNS name (with possible compression). Returns new offset.
fn skip_dns_name(packet: &[u8], mut offset: usize) -> usize {
    loop {
        if offset >= packet.len() {
            return 0;
        }
        let len = packet[offset] as usize;
        if len == 0 {
            return offset + 1;
        }
        if len & 0xC0 == 0xC0 {
            // Pointeur de compression / Compression pointer
            return offset + 2;
        }
        offset += 1 + len;
    }
}

/// Lit un nom DNS complet (avec décompression).
/// Read a full DNS name (with decompression).
fn read_dns_name(packet: &[u8], mut offset: usize) -> Option<String> {
    let mut parts = Vec::new();
    let mut jumps = 0;
    let max_jumps = 10;

    loop {
        if offset >= packet.len() || jumps > max_jumps {
            return None;
        }
        let len = packet[offset] as usize;
        if len == 0 {
            break;
        }
        if len & 0xC0 == 0xC0 {
            if offset + 1 >= packet.len() {
                return None;
            }
            let ptr = ((len & 0x3F) << 8) | packet[offset + 1] as usize;
            offset = ptr;
            jumps += 1;
            continue;
        }
        offset += 1;
        if offset + len > packet.len() {
            return None;
        }
        let label = std::str::from_utf8(&packet[offset..offset + len]).ok()?;
        parts.push(label.to_string());
        offset += len;
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_insert_and_get() {
        let cache = DnsSnoopCache::new(300);
        let ip: IpAddr = "93.184.216.34".parse().unwrap();
        cache.insert(ip, "example.com".to_string(), Some(60));
        assert_eq!(cache.get(&ip), Some("example.com".to_string()));
    }

    #[test]
    fn cache_miss_returns_none() {
        let cache = DnsSnoopCache::new(300);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        assert_eq!(cache.get(&ip), None);
    }

    #[test]
    fn cache_expired_returns_none() {
        let cache = DnsSnoopCache::new(0); // TTL de 0s = expire immédiatement
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        cache.insert(ip, "test.com".to_string(), Some(0));
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(cache.get(&ip), None);
    }

    #[test]
    fn parse_a_record() {
        // Paquet DNS minimal avec une réponse A pour "example.com" → 93.184.216.34
        // Minimal DNS packet with an A answer for "example.com" → 93.184.216.34
        #[rustfmt::skip]
        let packet: Vec<u8> = vec![
            // Header
            0x00, 0x01, // ID
            0x81, 0x80, // Flags: QR=1, RCODE=0 (no error)
            0x00, 0x01, // QDCOUNT = 1
            0x00, 0x01, // ANCOUNT = 1
            0x00, 0x00, // NSCOUNT = 0
            0x00, 0x00, // ARCOUNT = 0
            // Question: example.com A IN
            0x07, b'e', b'x', b'a', b'm', b'p', b'l', b'e',
            0x03, b'c', b'o', b'm',
            0x00,       // end of name
            0x00, 0x01, // QTYPE = A
            0x00, 0x01, // QCLASS = IN
            // Answer: example.com A 93.184.216.34 TTL=300
            0xC0, 0x0C, // Name pointer to offset 12
            0x00, 0x01, // TYPE = A
            0x00, 0x01, // CLASS = IN
            0x00, 0x00, 0x01, 0x2C, // TTL = 300
            0x00, 0x04, // RDLENGTH = 4
            93, 184, 216, 34, // RDATA = 93.184.216.34
        ];

        let results = parse_dns_response(&packet);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "example.com");
        assert_eq!(results[0].1, "93.184.216.34".parse::<IpAddr>().unwrap());
        assert_eq!(results[0].2, 300);
    }

    #[test]
    fn parse_empty_response() {
        let results = parse_dns_response(&[]);
        assert!(results.is_empty());
    }

    #[test]
    fn parse_non_response_ignored() {
        // Query (QR=0), pas une réponse
        let packet: Vec<u8> = vec![
            0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let results = parse_dns_response(&packet);
        assert!(results.is_empty());
    }

    #[test]
    fn evict_expired_removes_old_entries() {
        let cache = DnsSnoopCache::new(0);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        cache.insert(ip, "old.com".to_string(), Some(0));
        std::thread::sleep(Duration::from_millis(10));
        cache.evict_expired();
        assert_eq!(cache.len(), 0);
    }
}
