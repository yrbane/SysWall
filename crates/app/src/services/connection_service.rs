use std::sync::Arc;

use syswall_domain::entities::Connection;
use syswall_domain::errors::DomainError;
use syswall_domain::events::{DefaultPolicy, DomainEvent};
use syswall_domain::ports::{EventBus, ProcessResolver, RuleRepository};
use syswall_domain::services::PolicyEngine;

/// Service for processing network connections (enrichment + policy evaluation).
/// Service de traitement des connexions réseau (enrichissement + évaluation des politiques).
pub struct ConnectionService {
    /// Kept for sub-project 3 (conntrack process resolution).
    /// Conservé pour le sous-projet 3 (résolution de processus conntrack).
    #[allow(dead_code)]
    process_resolver: Arc<dyn ProcessResolver>,
    rule_repo: Arc<dyn RuleRepository>,
    event_bus: Arc<dyn EventBus>,
    default_policy: DefaultPolicy,
}

impl ConnectionService {
    pub fn new(
        process_resolver: Arc<dyn ProcessResolver>,
        rule_repo: Arc<dyn RuleRepository>,
        event_bus: Arc<dyn EventBus>,
        default_policy: DefaultPolicy,
    ) -> Self {
        Self {
            process_resolver,
            rule_repo,
            event_bus,
            default_policy,
        }
    }

    /// Enrich a raw connection with process info and evaluate against rules.
    /// Enrichit une connexion brute avec les infos processus et évalue les règles.
    pub async fn process_connection(
        &self,
        mut connection: Connection,
    ) -> Result<Connection, DomainError> {
        // Best-effort process enrichment via /proc/net lookup
        // Enrichissement best-effort du processus via /proc/net
        if connection.process.is_none() {
            let local_ip = connection.source.ip;
            let local_port = connection.source.port.value();
            let remote_ip = connection.destination.ip;
            let remote_port = connection.destination.port.value();

            match self
                .process_resolver
                .resolve_by_connection(
                    connection.protocol,
                    local_ip,
                    local_port,
                    remote_ip,
                    remote_port,
                )
                .await
            {
                Ok(Some(info)) => {
                    tracing::debug!(
                        "Resolved process for {}:{} -> {}:{}: {} (PID {})",
                        local_ip, local_port, remote_ip, remote_port,
                        info.name, info.pid
                    );
                    connection.process = Some(info);
                }
                Ok(None) => {
                    tracing::trace!(
                        "No process found for {}:{} -> {}:{}",
                        local_ip, local_port, remote_ip, remote_port
                    );
                }
                Err(e) => {
                    tracing::debug!("Process resolution failed: {}", e);
                }
            }
        }

        // Load rules and evaluate
        let rules = self.rule_repo.list_enabled_ordered().await?;
        let evaluation = PolicyEngine::evaluate(&connection, &rules, self.default_policy);

        connection.verdict = evaluation.verdict;
        connection.matched_rule = evaluation.matched_rule_id;

        // Publish event
        let _ = self
            .event_bus
            .publish(DomainEvent::ConnectionDetected(connection.clone()))
            .await;

        if let Some(rule_id) = evaluation.matched_rule_id {
            let _ = self
                .event_bus
                .publish(DomainEvent::RuleMatched {
                    connection_id: connection.id,
                    rule_id,
                    verdict: connection.verdict,
                })
                .await;
        }

        Ok(connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CreateRuleCommand;
    use crate::fakes::*;
    use crate::services::rule_service::RuleService;
    use chrono::Utc;
    use syswall_domain::entities::*;
    use syswall_domain::value_objects::*;

    fn test_connection() -> Connection {
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
        }
    }

    #[tokio::test]
    async fn process_connection_with_no_rules_uses_default_policy() {
        let process_resolver = Arc::new(FakeProcessResolver::new());
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let event_bus = Arc::new(FakeEventBus::new());

        let service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus,
            DefaultPolicy::Block,
        );

        let conn = service.process_connection(test_connection()).await.unwrap();
        assert_eq!(conn.verdict, ConnectionVerdict::Blocked);
    }

    #[tokio::test]
    async fn process_connection_matches_rule() {
        let process_resolver = Arc::new(FakeProcessResolver::new());
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let firewall = Arc::new(FakeFirewallEngine::new());

        // Create an allow rule via RuleService
        let rule_service = RuleService::new(rule_repo.clone(), firewall, event_bus.clone());
        let rule = rule_service
            .create_rule(CreateRuleCommand {
                name: "Allow HTTPS".to_string(),
                priority: 10,
                criteria: RuleCriteria {
                    remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
                    protocol: Some(Protocol::Tcp),
                    ..Default::default()
                },
                effect: RuleEffect::Allow,
                scope: RuleScope::Permanent,
                source: RuleSource::Manual,
            })
            .await
            .unwrap();

        let service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus,
            DefaultPolicy::Block,
        );

        let conn = service.process_connection(test_connection()).await.unwrap();
        assert_eq!(conn.verdict, ConnectionVerdict::Allowed);
        assert_eq!(conn.matched_rule, Some(rule.id));
    }
}
