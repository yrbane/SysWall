//! Benchmarks du parser de paquets NFQUEUE avec Criterion.
//! Mesure le cout par paquet du parsing IPv4/IPv6 + TCP/UDP en bytes -> Connection.
//!
//! NFQUEUE packet parser benchmarks with Criterion.
//! Measures the per-packet cost of parsing IPv4/IPv6 + TCP/UDP bytes -> Connection.
//!
//! Le parser est le hot path : appele 1 fois par paquet capture par NFQUEUE.
//! Cible : >= 1 microseconde / paquet (= ~1M paquets/s en peak parsing).
//!
//! The parser is the hot path: called once per packet captured by NFQUEUE.
//! Target: >= 1 microsecond / packet (~ 1M peak parsed packets/s).

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use syswall_infra::nfqueue::parser::parse_packet;

/// Construit un paquet IPv4 + TCP SYN minimaliste.
/// Builds a minimal IPv4 + TCP SYN packet.
fn ipv4_tcp_syn() -> Vec<u8> {
    let mut buffer = Vec::new();
    let builder =
        etherparse::PacketBuilder::ipv4([10, 0, 0, 1], [1, 2, 3, 4], 64).tcp(12345, 443, 0, 1024);
    let payload: &[u8] = &[];
    builder
        .write(&mut buffer, payload)
        .expect("etherparse builder");
    buffer
}

/// Construit un paquet IPv4 + UDP avec petit payload.
/// Builds an IPv4 + UDP packet with small payload.
fn ipv4_udp() -> Vec<u8> {
    let mut buffer = Vec::new();
    let builder = etherparse::PacketBuilder::ipv4([10, 0, 0, 1], [1, 1, 1, 1], 64).udp(54321, 53);
    let payload: &[u8] = b"\x00\x00\x01\x00\x00\x01\x00\x00";
    builder
        .write(&mut buffer, payload)
        .expect("etherparse builder");
    buffer
}

/// Construit un paquet IPv6 + UDP.
/// Builds an IPv6 + UDP packet.
fn ipv6_udp() -> Vec<u8> {
    let mut buffer = Vec::new();
    let builder = etherparse::PacketBuilder::ipv6(
        [0x20, 0x01, 0xdb, 0x8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [0x20, 0x01, 0xdb, 0x8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
        64,
    )
    .udp(12345, 53);
    let payload: &[u8] = b"\x12\x34";
    builder
        .write(&mut buffer, payload)
        .expect("etherparse builder");
    buffer
}

fn parser_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("nfq_parser::parse_packet");

    let packets = [
        ("ipv4_tcp", ipv4_tcp_syn()),
        ("ipv4_udp", ipv4_udp()),
        ("ipv6_udp", ipv6_udp()),
    ];

    for (label, bytes) in &packets {
        group.bench_with_input(BenchmarkId::new("kind", label), bytes, |b, bytes| {
            b.iter(|| parse_packet(black_box(bytes)).expect("parse ok"));
        });
    }

    group.finish();
}

criterion_group!(benches, parser_benchmark);
criterion_main!(benches);
