//! Traduction de l'action de règle en verdict nft (accept, drop, log+accept).
//! Translation of rule action into nft verdict (accept, drop, log+accept).

use syswall_domain::entities::RuleEffect;

/// Construit l'expression de verdict nft pour un effet de règle.
/// Build the nft verdict expression for a rule effect.
pub(super) fn build_verdict(effect: RuleEffect) -> Vec<String> {
    match effect {
        RuleEffect::Allow => vec!["accept".to_string()],
        RuleEffect::Block => vec!["drop".to_string()],
        RuleEffect::Observe => vec![
            "log".to_string(),
            "prefix".to_string(),
            "\"syswall-observe: \"".to_string(),
            "accept".to_string(),
        ],
        RuleEffect::Ask => vec![], // should never reach here
    }
}
