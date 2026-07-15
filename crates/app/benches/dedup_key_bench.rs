//! Benchmark de `LearningService::dedup_key` avec Criterion.
//! La dedup_key est calculee a chaque verdict pending — hot path.
//!
//! Benchmark of `LearningService::dedup_key` with Criterion.
//! The dedup_key is computed for each pending verdict — hot path.
//!
//! Cible : sub-microseconde, le formatage de chaine est le seul cout.
//! Target: sub-microsecond, string formatting is the only cost.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use syswall_app::services::learning_service::LearningService;
use syswall_domain::entities::ConnectionSnapshot;
use syswall_domain::value_objects::*;

fn snapshot_with_app(app: &str) -> ConnectionSnapshot {
    ConnectionSnapshot {
        protocol: Protocol::Tcp,
        source: SocketAddress::new("192.168.1.100".parse().unwrap(), Port::new(45000).unwrap()),
        destination: SocketAddress::new("93.184.216.34".parse().unwrap(), Port::new(443).unwrap()),
        direction: Direction::Outbound,
        process_name: Some(app.to_string()),
        process_path: Some(ExecutablePath::new("/usr/bin/firefox".into()).unwrap()),
        user: Some("seb".to_string()),
        hostname: None,
    }
}

fn dedup_key_benchmark(c: &mut Criterion) {
    let snapshot = snapshot_with_app("firefox");
    c.bench_function("LearningService::dedup_key", |b| {
        b.iter(|| LearningService::dedup_key(black_box(&snapshot)));
    });
}

criterion_group!(benches, dedup_key_benchmark);
criterion_main!(benches);
