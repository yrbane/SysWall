use syswall_domain::errors::DomainError;

use super::types::NftRuleEntry;

/// Parse the JSON output of `nft -j list table inet syswall` to extract rule entries.
/// Analyse la sortie JSON de `nft -j list table inet syswall` pour extraire les entrees de regles.
pub fn parse_nft_table_rules(json: &str) -> Result<Vec<NftRuleEntry>, DomainError> {
    let root: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| DomainError::Infrastructure(format!("Failed to parse nft JSON: {}", e)))?;

    let nftables = root
        .get("nftables")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            DomainError::Infrastructure("Missing 'nftables' array in nft output".to_string())
        })?;

    let mut entries = Vec::new();

    for item in nftables {
        if let Some(rule_obj) = item.get("rule") {
            let chain = rule_obj
                .get("chain")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let handle = rule_obj
                .get("handle")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            let comment = rule_obj
                .get("comment")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            entries.push(NftRuleEntry {
                chain,
                handle,
                comment,
            });
        }
    }

    Ok(entries)
}

/// Extract a SysWall rule UUID from a comment string like "syswall:550e8400-...".
/// Extrait un UUID de regle SysWall depuis une chaine de commentaire comme "syswall:550e8400-...".
pub fn extract_rule_id_from_comment(comment: &str) -> Option<uuid::Uuid> {
    comment
        .strip_prefix("syswall:")
        .and_then(|uuid_str| uuid::Uuid::parse_str(uuid_str).ok())
}

/// Check if a given JSON output indicates that the syswall table exists.
/// Verifie si une sortie JSON donnee indique que la table syswall existe.
pub fn table_exists_in_json(json: &str, table_name: &str) -> bool {
    let root: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let nftables = match root.get("nftables").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return false,
    };

    nftables.iter().any(|item| {
        item.get("table")
            .and_then(|t| t.get("name"))
            .and_then(|n| n.as_str())
            .is_some_and(|name| name == table_name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_table_with_one_rule() {
        let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}, {"table": {"family": "inet", "name": "syswall", "handle": 1}}, {"chain": {"family": "inet", "table": "syswall", "name": "output", "handle": 2, "type": "filter", "hook": "output", "prio": 0, "policy": "accept"}}, {"rule": {"family": "inet", "table": "syswall", "chain": "output", "handle": 5, "comment": "syswall:550e8400-e29b-41d4-a716-446655440000", "expr": [{"match": {"op": "==", "left": {"meta": {"key": "l4proto"}}, "right": "tcp"}}, {"match": {"op": "==", "left": {"payload": {"protocol": "tcp", "field": "dport"}}, "right": 443}}, {"accept": null}]}}]}"#;
        let rules = parse_nft_table_rules(json).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].handle, 5);
        assert_eq!(rules[0].chain, "output");
        assert_eq!(
            rules[0].comment,
            Some("syswall:550e8400-e29b-41d4-a716-446655440000".to_string())
        );
    }

    #[test]
    fn parse_empty_table() {
        let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}, {"table": {"family": "inet", "name": "syswall", "handle": 1}}]}"#;
        let rules = parse_nft_table_rules(json).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let result = parse_nft_table_rules("not json");
        assert!(result.is_err());
    }

    #[test]
    fn parse_missing_nftables_key_returns_error() {
        let json = r#"{"other": []}"#;
        let result = parse_nft_table_rules(json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_multiple_rules() {
        let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}, {"rule": {"family": "inet", "table": "syswall", "chain": "input", "handle": 3, "comment": "syswall:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"}}, {"rule": {"family": "inet", "table": "syswall", "chain": "output", "handle": 4, "comment": "syswall:11111111-2222-3333-4444-555555555555"}}]}"#;
        let rules = parse_nft_table_rules(json).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].chain, "input");
        assert_eq!(rules[1].chain, "output");
    }

    #[test]
    fn parse_rule_without_comment() {
        let json = r#"{"nftables": [{"rule": {"family": "inet", "table": "syswall", "chain": "output", "handle": 7}}]}"#;
        let rules = parse_nft_table_rules(json).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].comment.is_none());
    }

    #[test]
    fn extract_uuid_from_valid_comment() {
        let uuid =
            extract_rule_id_from_comment("syswall:550e8400-e29b-41d4-a716-446655440000");
        assert!(uuid.is_some());
        assert_eq!(
            uuid.unwrap().to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn extract_uuid_from_invalid_comment() {
        assert!(extract_rule_id_from_comment("not-syswall").is_none());
        assert!(extract_rule_id_from_comment("syswall:not-a-uuid").is_none());
        assert!(extract_rule_id_from_comment("").is_none());
    }

    #[test]
    fn table_exists_returns_true_for_existing_table() {
        let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}, {"table": {"family": "inet", "name": "syswall", "handle": 1}}]}"#;
        assert!(table_exists_in_json(json, "syswall"));
    }

    #[test]
    fn table_exists_returns_false_for_missing_table() {
        let json = r#"{"nftables": [{"metainfo": {"version": "1.0.6"}}]}"#;
        assert!(!table_exists_in_json(json, "syswall"));
    }

    #[test]
    fn table_exists_returns_false_for_different_table() {
        let json = r#"{"nftables": [{"table": {"family": "inet", "name": "filter", "handle": 1}}]}"#;
        assert!(!table_exists_in_json(json, "syswall"));
    }

    #[test]
    fn table_exists_returns_false_for_invalid_json() {
        assert!(!table_exists_in_json("not json", "syswall"));
    }

    #[test]
    fn parse_rules_preserves_handle_order() {
        let json = r#"{"nftables": [{"rule": {"chain": "output", "handle": 10}}, {"rule": {"chain": "output", "handle": 20}}, {"rule": {"chain": "output", "handle": 30}}]}"#;
        let rules = parse_nft_table_rules(json).unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].handle, 10);
        assert_eq!(rules[1].handle, 20);
        assert_eq!(rules[2].handle, 30);
    }

    #[test]
    fn extract_uuid_from_comment_with_extra_whitespace() {
        // The comment should be exact, no whitespace tolerance
        assert!(extract_rule_id_from_comment(" syswall:550e8400-e29b-41d4-a716-446655440000").is_none());
    }

    #[test]
    fn parse_mixed_syswall_and_non_syswall_rules() {
        let json = r#"{"nftables": [{"rule": {"chain": "output", "handle": 5, "comment": "syswall:550e8400-e29b-41d4-a716-446655440000"}}, {"rule": {"chain": "output", "handle": 6, "comment": "other-system-rule"}}, {"rule": {"chain": "input", "handle": 7}}]}"#;
        let rules = parse_nft_table_rules(json).unwrap();
        assert_eq!(rules.len(), 3);

        // Only the first rule should have a syswall UUID
        let syswall_rules: Vec<_> = rules
            .iter()
            .filter(|r| {
                r.comment
                    .as_ref()
                    .and_then(|c| extract_rule_id_from_comment(c))
                    .is_some()
            })
            .collect();
        assert_eq!(syswall_rules.len(), 1);
        assert_eq!(syswall_rules[0].handle, 5);
    }
}
