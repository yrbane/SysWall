//! Règles de la chaîne d'interception NFQUEUE.
//! NFQUEUE interception chain rules.

/// Build the interception chain that forwards new outbound flows to NFQUEUE.
/// Construit la chaîne d'interception qui transfère les nouveaux flux sortants vers NFQUEUE.
///
/// Retourne une liste de commandes nft à exécuter séquentiellement (idempotentes).
/// Returns a list of nft commands to execute sequentially (idempotent).
pub fn build_interception_chain_rules(table_name: &str, queue_num: u16) -> Vec<String> {
    vec![
        // Crée la chaîne (si pas déjà existante).
        // Create the chain (if not already present).
        format!(
            "add chain inet {table_name} interception {{ type filter hook output priority 0 ; policy accept ; }}"
        ),
        // Bypass loopback : évite tout deadlock IPC.
        // Bypass loopback: avoid any IPC deadlock.
        format!("add rule inet {table_name} interception iif lo accept"),
        // Active interception : queue le premier paquet de chaque nouveau flux.
        // bypass = fail-open si le démon ne consomme plus la queue.
        // Active interception: queue first packet of every new flow.
        // bypass = fail-open if the daemon stops consuming.
        format!(
            "add rule inet {table_name} interception ct state new queue num {queue_num} bypass"
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_declaration_contains_table_name() {
        let rules = build_interception_chain_rules("syswall", 0);
        assert!(rules[0].contains("syswall"));
        assert!(rules[0].contains("interception"));
        assert!(rules[0].contains("output"));
    }

    #[test]
    fn loopback_bypass_rule_is_second() {
        let rules = build_interception_chain_rules("syswall", 0);
        assert!(rules[1].contains("iif lo accept"));
    }

    #[test]
    fn queue_rule_uses_correct_queue_num() {
        let rules = build_interception_chain_rules("syswall", 7);
        assert!(rules[2].contains("queue num 7"));
        assert!(rules[2].contains("bypass"));
        assert!(rules[2].contains("ct state new"));
    }

    #[test]
    fn returns_three_rules() {
        let rules = build_interception_chain_rules("syswall", 0);
        assert_eq!(rules.len(), 3);
    }
}
