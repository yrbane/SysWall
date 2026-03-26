//! End-to-end test that verifies the full connection processing pipeline
//! using fake adapters: connection event -> enrichment -> policy evaluation -> verdict.
//!
//! Test de bout en bout verifiant le pipeline complet de traitement des connexions
//! avec des adaptateurs factices : evenement connexion -> enrichissement -> evaluation -> verdict.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;

    use syswall_domain::entities::*;
    use syswall_domain::events::DefaultPolicy;
    use syswall_domain::ports::PendingDecisionRepository;
    use syswall_domain::value_objects::*;

    use crate::commands::CreateRuleCommand;
    use crate::fakes::*;
    use crate::services::connection_service::ConnectionService;
    use crate::services::learning_service::{LearningConfig, LearningService};
    use crate::services::rule_service::RuleService;
    use crate::services::whitelist::ensure_system_whitelist;

    fn make_connection(
        protocol: Protocol,
        dst_ip: &str,
        dst_port: u16,
        direction: Direction,
    ) -> Connection {
        Connection {
            id: ConnectionId::new(),
            protocol,
            source: SocketAddress::new(
                "192.168.1.100".parse().unwrap(),
                Port::new(45000).unwrap(),
            ),
            destination: SocketAddress::new(
                dst_ip.parse().unwrap(),
                Port::new(dst_port).unwrap(),
            ),
            direction,
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
        }
    }

    #[tokio::test]
    async fn full_pipeline_with_matching_rule() {
        // Setup
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let process_resolver = Arc::new(FakeProcessResolver::new());

        let rule_service = RuleService::new(
            rule_repo.clone(),
            firewall.clone(),
            event_bus.clone(),
        );

        // Create an allow rule for HTTPS
        let _rule = rule_service
            .create_rule(CreateRuleCommand {
                name: "Allow HTTPS".to_string(),
                priority: 10,
                criteria: RuleCriteria {
                    protocol: Some(Protocol::Tcp),
                    remote_port: Some(PortMatcher::Exact(Port::new(443).unwrap())),
                    direction: Some(Direction::Outbound),
                    ..Default::default()
                },
                effect: RuleEffect::Allow,
                scope: RuleScope::Permanent,
                source: RuleSource::Manual,
            })
            .await
            .unwrap();

        let connection_service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus,
            DefaultPolicy::Block,
        );

        // Simulate a connection event
        let conn = make_connection(Protocol::Tcp, "93.184.216.34", 443, Direction::Outbound);

        // Process through the pipeline
        let result = connection_service.process_connection(conn).await.unwrap();

        // Verify verdict
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);
        assert!(result.matched_rule.is_some());
    }

    #[tokio::test]
    async fn full_pipeline_no_matching_rule_uses_default_policy() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let process_resolver = Arc::new(FakeProcessResolver::new());

        let connection_service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus,
            DefaultPolicy::Ask,
        );

        let conn = make_connection(Protocol::Tcp, "93.184.216.34", 443, Direction::Outbound);
        let result = connection_service.process_connection(conn).await.unwrap();

        assert_eq!(result.verdict, ConnectionVerdict::PendingDecision);
        assert!(result.matched_rule.is_none());
    }

    #[tokio::test]
    async fn full_pipeline_with_system_whitelist() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let process_resolver = Arc::new(FakeProcessResolver::new());

        let rule_service = RuleService::new(
            rule_repo.clone(),
            firewall.clone(),
            event_bus.clone(),
        );

        // Create system whitelist
        ensure_system_whitelist(&rule_service, rule_repo.as_ref())
            .await
            .unwrap();

        let connection_service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus,
            DefaultPolicy::Block,
        );

        // DNS query should be allowed by whitelist
        let dns_conn = make_connection(Protocol::Udp, "8.8.8.8", 53, Direction::Outbound);
        let result = connection_service.process_connection(dns_conn).await.unwrap();
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);

        // NTP should be allowed by whitelist
        let ntp_conn = make_connection(Protocol::Udp, "129.6.15.28", 123, Direction::Outbound);
        let result = connection_service.process_connection(ntp_conn).await.unwrap();
        assert_eq!(result.verdict, ConnectionVerdict::Allowed);

        // Random HTTP should be blocked (default policy = Block)
        let http_conn = make_connection(Protocol::Tcp, "93.184.216.34", 80, Direction::Outbound);
        let result = connection_service.process_connection(http_conn).await.unwrap();
        assert_eq!(result.verdict, ConnectionVerdict::Blocked);
    }

    #[tokio::test]
    async fn full_pipeline_ask_effect_triggers_learning() {
        let rule_repo = Arc::new(FakeRuleRepository::new());
        let firewall = Arc::new(FakeFirewallEngine::new());
        let event_bus = Arc::new(FakeEventBus::new());
        let process_resolver = Arc::new(FakeProcessResolver::new());
        let pending_repo = Arc::new(FakePendingDecisionRepository::new());
        let decision_repo = Arc::new(FakeDecisionRepository::new());
        let notifier = Arc::new(FakeUserNotifier::new());

        let rule_service = RuleService::new(
            rule_repo.clone(),
            firewall.clone(),
            event_bus.clone(),
        );

        // Create an Ask rule
        rule_service
            .create_rule(CreateRuleCommand {
                name: "Ask for SSH".to_string(),
                priority: 10,
                criteria: RuleCriteria {
                    protocol: Some(Protocol::Tcp),
                    remote_port: Some(PortMatcher::Exact(Port::new(22).unwrap())),
                    ..Default::default()
                },
                effect: RuleEffect::Ask,
                scope: RuleScope::Permanent,
                source: RuleSource::Manual,
            })
            .await
            .unwrap();

        let connection_service = ConnectionService::new(
            process_resolver,
            rule_repo,
            event_bus.clone(),
            DefaultPolicy::Block,
        );

        let learning_service = LearningService::new(
            pending_repo.clone(),
            decision_repo,
            notifier,
            event_bus,
            LearningConfig {
                prompt_timeout_secs: 60,
                max_pending_decisions: 50,
            },
        );

        // Process SSH connection
        let conn = make_connection(Protocol::Tcp, "10.0.0.1", 22, Direction::Outbound);
        let result = connection_service.process_connection(conn).await.unwrap();
        assert_eq!(result.verdict, ConnectionVerdict::PendingDecision);

        // Feed to learning service (like the daemon pipeline would)
        let snapshot = result.snapshot();
        learning_service
            .handle_unknown_connection(snapshot)
            .await
            .unwrap();

        // Verify a pending decision was created
        let pending = pending_repo.list_pending().await.unwrap();
        assert_eq!(pending.len(), 1);
    }
}
