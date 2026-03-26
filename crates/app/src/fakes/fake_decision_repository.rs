use async_trait::async_trait;
use std::sync::Mutex;

use syswall_domain::entities::Decision;
use syswall_domain::errors::DomainError;
use syswall_domain::ports::DecisionRepository;

/// In-memory fake decision repository for testing.
/// Dépôt factice en mémoire des décisions résolues pour les tests.
pub struct FakeDecisionRepository {
    pub decisions: Mutex<Vec<Decision>>,
}

impl Default for FakeDecisionRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeDecisionRepository {
    pub fn new() -> Self {
        Self {
            decisions: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl DecisionRepository for FakeDecisionRepository {
    async fn save(&self, decision: &Decision) -> Result<(), DomainError> {
        self.decisions.lock().unwrap().push(decision.clone());
        Ok(())
    }
}
