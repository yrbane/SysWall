use std::collections::HashMap;
use std::time::Instant;

use syswall_domain::entities::RuleId;

/// Handle assigned by nftables to a specific rule in a chain.
/// Handle assigne par nftables a une regle specifique dans une chaine.
#[derive(Debug, Clone)]
pub struct NftRuleHandle {
    pub chain: String,
    pub handle: u64,
}

/// Tracks the mapping between domain RuleId and nftables handles.
/// Suit la correspondance entre RuleId du domaine et les handles nftables.
#[derive(Debug, Default)]
pub struct HandleMap {
    handles: HashMap<RuleId, Vec<NftRuleHandle>>,
}

impl HandleMap {
    /// Create a new empty handle map.
    /// Cree une nouvelle table de correspondance vide.
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    /// Insert handles for a domain rule.
    /// Insere les handles pour une regle du domaine.
    pub fn insert(&mut self, rule_id: RuleId, rule_handles: Vec<NftRuleHandle>) {
        self.handles.insert(rule_id, rule_handles);
    }

    /// Get handles for a domain rule.
    /// Retourne les handles pour une regle du domaine.
    pub fn get(&self, rule_id: &RuleId) -> Option<&Vec<NftRuleHandle>> {
        self.handles.get(rule_id)
    }

    /// Remove handles for a domain rule.
    /// Supprime les handles pour une regle du domaine.
    pub fn remove(&mut self, rule_id: &RuleId) -> Option<Vec<NftRuleHandle>> {
        self.handles.remove(rule_id)
    }

    /// Clear all handles (used during sync).
    /// Efface tous les handles (utilise lors de la synchronisation).
    pub fn clear(&mut self) {
        self.handles.clear();
    }
}

/// Saved state for rollback on failure.
/// Etat sauvegarde pour retour arriere en cas d'echec.
#[derive(Debug)]
pub struct RollbackState {
    /// JSON output of `nft -j list table inet syswall` before the operation.
    /// Sortie JSON de `nft -j list table inet syswall` avant l'operation.
    pub table_state: String,
    /// When the state was saved.
    /// Quand l'etat a ete sauvegarde.
    pub saved_at: Instant,
}

/// A parsed rule entry from nft JSON output.
/// Une entree de regle parsee depuis la sortie JSON de nft.
#[derive(Debug, Clone)]
pub struct NftRuleEntry {
    pub chain: String,
    pub handle: u64,
    pub comment: Option<String>,
}

/// The result of translating a domain Rule into nft expressions.
/// Returns None if the rule should not be expressed in nftables (e.g., Ask effect).
///
/// Le resultat de la traduction d'une Rule du domaine en expressions nft.
/// Retourne None si la regle ne doit pas etre exprimee dans nftables (ex. effet Ask).
#[derive(Debug, Clone)]
pub struct TranslatedRule {
    /// Target chain(s): "input", "output", or both.
    /// Chaine(s) cible : "input", "output", ou les deux.
    pub chains: Vec<String>,
    /// nft expression arguments (protocol match, port match, verdict, comment).
    /// Arguments d'expression nft (correspondance protocole, port, verdict, commentaire).
    pub expressions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_map_insert_and_get() {
        let mut map = HandleMap::new();
        let rule_id = RuleId::new();
        map.insert(
            rule_id,
            vec![NftRuleHandle {
                chain: "output".to_string(),
                handle: 42,
            }],
        );
        let handles = map.get(&rule_id).unwrap();
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].handle, 42);
        assert_eq!(handles[0].chain, "output");
    }

    #[test]
    fn handle_map_remove() {
        let mut map = HandleMap::new();
        let rule_id = RuleId::new();
        map.insert(
            rule_id,
            vec![NftRuleHandle {
                chain: "input".to_string(),
                handle: 7,
            }],
        );
        let removed = map.remove(&rule_id);
        assert!(removed.is_some());
        assert!(map.get(&rule_id).is_none());
    }

    #[test]
    fn handle_map_clear() {
        let mut map = HandleMap::new();
        map.insert(
            RuleId::new(),
            vec![NftRuleHandle {
                chain: "output".to_string(),
                handle: 1,
            }],
        );
        map.insert(
            RuleId::new(),
            vec![NftRuleHandle {
                chain: "input".to_string(),
                handle: 2,
            }],
        );
        map.clear();
        // After clear, get on any key should return None
        assert!(map.get(&RuleId::new()).is_none());
    }

    #[test]
    fn handle_map_get_nonexistent_returns_none() {
        let map = HandleMap::new();
        assert!(map.get(&RuleId::new()).is_none());
    }

    #[test]
    fn handle_map_multiple_handles_per_rule() {
        let mut map = HandleMap::new();
        let rule_id = RuleId::new();
        map.insert(
            rule_id,
            vec![
                NftRuleHandle {
                    chain: "input".to_string(),
                    handle: 10,
                },
                NftRuleHandle {
                    chain: "output".to_string(),
                    handle: 11,
                },
            ],
        );
        let handles = map.get(&rule_id).unwrap();
        assert_eq!(handles.len(), 2);
    }
}
