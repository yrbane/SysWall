/// Benchmarks du PolicyEngine avec Criterion.
/// Mesure le temps d'évaluation d'une connexion contre N règles.
///
/// PolicyEngine benchmarks with Criterion.
/// Measures connection evaluation time against N rules.
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use chrono::Utc;
use syswall_domain::entities::*;
use syswall_domain::events::DefaultPolicy;
use syswall_domain::services::PolicyEngine;
use syswall_domain::value_objects::*;

fn make_connection() -> Connection {
    Connection {
        id: ConnectionId::new(),
        protocol: Protocol::Tcp,
        source: SocketAddress::new(
            "192.168.1.100".parse().unwrap(),
            Port::new(45000).unwrap(),
        ),
        destination: SocketAddress::new(
            "93.184.216.34".parse().unwrap(),
            Port::new(443).unwrap(),
        ),
        direction: Direction::Outbound,
        state: ConnectionState::New,
        process: Some(ProcessInfo {
            pid: 1234,
            name: "firefox".to_string(),
            path: None,
            cmdline: None,
            icon: None,
        }),
        user: Some(SystemUser {
            uid: 1000,
            name: "seb".to_string(),
        }),
        bytes_sent: 0,
        bytes_received: 0,
        started_at: Utc::now(),
        verdict: ConnectionVerdict::Unknown,
        matched_rule: None,
        remote_hostname: None,
    }
}

fn make_rules(count: usize) -> Vec<Rule> {
    (0..count)
        .map(|i| Rule {
            id: RuleId::new(),
            name: format!("Rule {}", i),
            priority: RulePriority::new(i as u32),
            enabled: true,
            criteria: RuleCriteria {
                remote_port: Some(PortMatcher::Exact(
                    Port::new((8000 + i % 50000) as u16).unwrap(),
                )),
                protocol: Some(Protocol::Tcp),
                ..Default::default()
            },
            effect: if i % 2 == 0 {
                RuleEffect::Allow
            } else {
                RuleEffect::Block
            },
            scope: RuleScope::Permanent,
            source: RuleSource::Manual,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .collect()
}

fn policy_engine_benchmark(c: &mut Criterion) {
    let conn = make_connection();
    let mut group = c.benchmark_group("PolicyEngine::evaluate");

    for rule_count in [10, 100, 500, 1000] {
        let rules = make_rules(rule_count);
        group.bench_with_input(
            BenchmarkId::new("rules", rule_count),
            &rules,
            |b, rules| {
                b.iter(|| {
                    PolicyEngine::evaluate(
                        black_box(&conn),
                        black_box(rules),
                        DefaultPolicy::Block,
                    )
                });
            },
        );
    }

    group.finish();
}

/// Benchmark du cas favorable : règle trouvée rapidement (priorité basse = premier match).
/// Best-case benchmark: rule found quickly (low priority = first match).
fn policy_engine_best_case(c: &mut Criterion) {
    let conn = make_connection();

    // Crée 1000 règles, la première matche directement (port 443)
    // Create 1000 rules, first one matches directly (port 443)
    let mut rules = make_rules(1000);
    rules[0].criteria = RuleCriteria {
        remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
        protocol: Some(Protocol::Tcp),
        ..Default::default()
    };

    c.bench_function("PolicyEngine::evaluate/best_case_1000_rules", |b| {
        b.iter(|| {
            PolicyEngine::evaluate(black_box(&conn), black_box(&rules), DefaultPolicy::Block)
        });
    });
}

criterion_group!(benches, policy_engine_benchmark, policy_engine_best_case);
criterion_main!(benches);
