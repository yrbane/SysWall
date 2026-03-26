use std::time::Duration;

/// Typed nft command builder. Never concatenates strings into shell commands.
/// All arguments are passed as separate entries, preventing injection.
///
/// Constructeur type de commandes nft. Ne concatene jamais de chaines en commandes shell.
/// Tous les arguments sont passes separement, empechant l'injection.
pub struct NftCommandBuilder {
    args: Vec<String>,
    timeout: Duration,
    max_output_bytes: usize,
}

impl NftCommandBuilder {
    /// Create a new empty command builder.
    /// Cree un nouveau constructeur de commande vide.
    pub fn new() -> Self {
        Self {
            args: vec![],
            timeout: Duration::from_secs(5),
            max_output_bytes: 1_048_576,
        }
    }

    /// Create a builder with custom timeout and output limit.
    /// Cree un constructeur avec un delai et une limite de sortie personnalises.
    pub fn with_limits(timeout: Duration, max_output_bytes: usize) -> Self {
        Self {
            args: vec![],
            timeout,
            max_output_bytes,
        }
    }

    /// List the syswall table in JSON format.
    /// Liste la table syswall au format JSON.
    pub fn list_table(table: &str) -> Self {
        Self::new()
            .arg("-j")
            .arg("list")
            .arg("table")
            .arg("inet")
            .arg(table)
    }

    /// Add a rule to a chain.
    /// Ajoute une regle a une chaine.
    pub fn add_rule(table: &str, chain: &str) -> Self {
        Self::new()
            .arg("add")
            .arg("rule")
            .arg("inet")
            .arg(table)
            .arg(chain)
    }

    /// Delete a rule by handle.
    /// Supprime une regle par handle.
    pub fn delete_rule(table: &str, chain: &str, handle: u64) -> Self {
        Self::new()
            .arg("delete")
            .arg("rule")
            .arg("inet")
            .arg(table)
            .arg(chain)
            .arg("handle")
            .arg(handle.to_string())
    }

    /// Create the table if it does not exist.
    /// Cree la table si elle n'existe pas.
    pub fn create_table(table: &str) -> Self {
        Self::new()
            .arg("add")
            .arg("table")
            .arg("inet")
            .arg(table)
    }

    /// Create a chain with the given hook and priority.
    /// Cree une chaine avec le hook et la priorite donnes.
    pub fn create_chain(table: &str, chain: &str, hook: &str, priority: i32) -> Self {
        Self::new()
            .arg("add")
            .arg("chain")
            .arg("inet")
            .arg(table)
            .arg(chain)
            .arg(format!(
                "{{ type filter hook {} priority {}; policy accept; }}",
                hook, priority
            ))
    }

    /// Save the full ruleset for rollback (JSON format).
    /// Sauvegarde l'ensemble des regles pour retour arriere (format JSON).
    pub fn list_ruleset_json() -> Self {
        Self::new().arg("-j").arg("list").arg("ruleset")
    }

    /// Append an argument.
    /// Ajoute un argument.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Get the built argument list (for testing and execution).
    /// Retourne la liste d'arguments construite (pour tests et execution).
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Get the configured timeout.
    /// Retourne le delai d'attente configure.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Get the max output bytes limit.
    /// Retourne la limite maximale d'octets en sortie.
    pub fn max_output_bytes(&self) -> usize {
        self.max_output_bytes
    }
}

impl Default for NftCommandBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty_builder_with_defaults() {
        let cmd = NftCommandBuilder::new();
        assert!(cmd.args().is_empty());
        assert_eq!(cmd.timeout(), Duration::from_secs(5));
        assert_eq!(cmd.max_output_bytes(), 1_048_576);
    }

    #[test]
    fn list_table_produces_correct_args() {
        let cmd = NftCommandBuilder::list_table("syswall");
        assert_eq!(
            cmd.args(),
            &["-j", "list", "table", "inet", "syswall"]
        );
    }

    #[test]
    fn add_rule_produces_correct_base_args() {
        let cmd = NftCommandBuilder::add_rule("syswall", "output");
        assert_eq!(
            cmd.args(),
            &["add", "rule", "inet", "syswall", "output"]
        );
    }

    #[test]
    fn add_rule_with_extra_args() {
        let cmd = NftCommandBuilder::add_rule("syswall", "output")
            .arg("meta")
            .arg("l4proto")
            .arg("tcp")
            .arg("tcp")
            .arg("dport")
            .arg("443")
            .arg("accept")
            .arg("comment")
            .arg("\"syswall:550e8400-e29b-41d4-a716-446655440000\"");
        assert!(cmd.args().contains(&"443".to_string()));
        assert!(cmd.args().contains(&"accept".to_string()));
        assert!(cmd.args().contains(&"output".to_string()));
    }

    #[test]
    fn delete_rule_includes_handle() {
        let cmd = NftCommandBuilder::delete_rule("syswall", "input", 42);
        assert_eq!(
            cmd.args(),
            &["delete", "rule", "inet", "syswall", "input", "handle", "42"]
        );
    }

    #[test]
    fn create_table_produces_correct_args() {
        let cmd = NftCommandBuilder::create_table("syswall");
        assert_eq!(
            cmd.args(),
            &["add", "table", "inet", "syswall"]
        );
    }

    #[test]
    fn create_chain_produces_correct_args() {
        let cmd = NftCommandBuilder::create_chain("syswall", "input", "input", 0);
        assert_eq!(
            cmd.args(),
            &[
                "add", "chain", "inet", "syswall", "input",
                "{ type filter hook input priority 0; policy accept; }"
            ]
        );
    }

    #[test]
    fn list_ruleset_json_produces_correct_args() {
        let cmd = NftCommandBuilder::list_ruleset_json();
        assert_eq!(
            cmd.args(),
            &["-j", "list", "ruleset"]
        );
    }

    #[test]
    fn arg_chaining_preserves_order() {
        let cmd = NftCommandBuilder::new()
            .arg("one")
            .arg("two")
            .arg("three");
        assert_eq!(cmd.args(), &["one", "two", "three"]);
    }

    #[test]
    fn with_limits_sets_custom_values() {
        let cmd = NftCommandBuilder::with_limits(
            Duration::from_secs(10),
            2_097_152,
        );
        assert_eq!(cmd.timeout(), Duration::from_secs(10));
        assert_eq!(cmd.max_output_bytes(), 2_097_152);
    }

    #[test]
    fn default_trait_matches_new() {
        let cmd = NftCommandBuilder::default();
        assert!(cmd.args().is_empty());
        assert_eq!(cmd.timeout(), Duration::from_secs(5));
        assert_eq!(cmd.max_output_bytes(), 1_048_576);
    }

    #[test]
    fn create_chain_with_negative_priority() {
        let cmd = NftCommandBuilder::create_chain("syswall", "prerouting", "prerouting", -100);
        assert_eq!(
            cmd.args(),
            &[
                "add", "chain", "inet", "syswall", "prerouting",
                "{ type filter hook prerouting priority -100; policy accept; }"
            ]
        );
    }

    #[test]
    fn delete_rule_large_handle() {
        let cmd = NftCommandBuilder::delete_rule("syswall", "output", u64::MAX);
        assert!(cmd.args().contains(&u64::MAX.to_string()));
    }

    #[test]
    fn add_rule_different_table() {
        let cmd = NftCommandBuilder::add_rule("mytable", "mychain");
        assert_eq!(
            cmd.args(),
            &["add", "rule", "inet", "mytable", "mychain"]
        );
    }
}
