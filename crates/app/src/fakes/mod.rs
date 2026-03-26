/// Fake (in-memory) implementations of all domain ports for testing.
/// Implémentations factices (en mémoire) de tous les ports du domaine pour les tests.
pub mod fake_audit_repository;
pub mod fake_connection_monitor;
pub mod fake_decision_repository;
pub mod fake_event_bus;
pub mod fake_firewall_engine;
pub mod fake_pending_decision_repository;
pub mod fake_process_resolver;
pub mod fake_rule_repository;
pub mod fake_user_notifier;

pub use fake_audit_repository::*;
pub use fake_connection_monitor::*;
pub use fake_decision_repository::*;
pub use fake_event_bus::*;
pub use fake_firewall_engine::*;
pub use fake_pending_decision_repository::*;
pub use fake_process_resolver::*;
pub use fake_rule_repository::*;
pub use fake_user_notifier::*;
