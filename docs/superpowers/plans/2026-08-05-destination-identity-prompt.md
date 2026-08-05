# Identité de destination dans le prompt — Plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Afficher dans le prompt de nouvelle connexion le nom de la destination (domaine snoopé, sinon reverse-DNS, sinon IP) avec une ligne résumé « AppX → github.com:443 ».

**Architecture:** Le reverse-DNS remplit déjà `snapshot.hostname` (via `ConnectionService::enrich_and_evaluate` → `Connection::snapshot()`), mais l'UI l'ignore. On (1) affiche le hostname côté UI, (2) ajoute un **front-cache de snooping DNS** *dans* l'impl `DnsResolver` (snoop d'abord, reverse ensuite) pour un nom plus précis, alimenté par (3) un **consommateur NFQUEUE dédié** lisant les réponses DNS, autorisé par (4) une **chaîne nft `input`**. Le tout câblé dans le bootstrap du daemon.

**Tech Stack:** Rust (hexagonal : domain/app/infra/daemon), `dashmap`, `etherparse`, `nfq`, nftables ; UI SvelteKit/TypeScript.

## Global Constraints

- **Version cible : 0.3.9** — bump `Cargo.toml` `[workspace.package] version`, `cargo update --workspace` (Cargo.lock), entrée en tête de `CHANGELOG.md` (FR) **et** `CHANGELOG.en.md` (EN), format `## [0.3.9] - AAAA-MM-JJ · « titre »`.
- **Code en anglais, commentaires bilingues FR+EN**, commits en français, **aucune mention d'assistant** (pas de Co-Authored-By, pas d'emoji robot).
- **`git add` ciblé**, jamais `-A`.
- **Ne jamais casser le DNS** : la queue nft DNS utilise `bypass` (fail-open) ; tout échec du consommateur → verdict ACCEPT ; paquet malformé → aucune panic.
- **Zéro nouvelle capability** (préserve le durcissement systemd + son check CI) : on réutilise `CAP_NET_ADMIN`/NFQUEUE.
- **TDD** : test qui échoue → implémentation minimale → test qui passe → commit.
- Tests Rust : `cargo test --workspace --exclude ui` (l'UI est exclue du workspace de test). Lint CI : `cargo fmt --all --check` + `cargo clippy --workspace -- -D warnings` (utiliser des let-chains edition 2024, pas de `collapsible_if`).

---

## File Structure

- `crates/ui/src/lib/types/index.ts` — ajoute `hostname?` au type `ConnectionSnapshot`.
- `crates/ui/src/lib/components/learning/DecisionPrompt.svelte` — parsing + affichage (ligne résumé, host primaire, IP secondaire).
- `crates/infra/src/dns/mod.rs` — `DnsResolver` consulte un `Arc<DnsSnoopCache>` optionnel avant le reverse.
- `crates/infra/src/dns/observer.rs` *(neuf)* — extraction du payload DNS + boucle consommateur NFQUEUE alimentant `DnsSnoopCache`.
- `crates/infra/src/dns/snooper.rs` — existant, inchangé (`DnsSnoopCache`, `parse_dns_response`).
- `crates/infra/src/nftables/translator/dns_observe_chain.rs` *(neuf)* — règles de la chaîne `input` d'observation DNS.
- `crates/daemon/src/bootstrap.rs` — instancie le cache partagé, câble resolver + consommateur, tâche d'éviction.
- `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `CHANGELOG.en.md` — versionnage.

---

### Task 1 : Afficher le hostname dans le prompt (UI)

Le reverse-DNS est déjà dans `snapshot_json` ; cette tâche seule livre déjà des noms.

**Files:**
- Modify: `crates/ui/src/lib/types/index.ts:56-65` (interface `ConnectionSnapshot`)
- Modify: `crates/ui/src/lib/components/learning/DecisionPrompt.svelte` (dérivation snapshot + template)

**Interfaces:**
- Consumes: `snapshot_json` contient déjà `hostname: string | null` (sérialisé depuis le domaine).
- Produces: rien pour d'autres tâches (feuille UI).

- [ ] **Step 1 : Ajouter le champ au type TS**

Dans `crates/ui/src/lib/types/index.ts`, interface `ConnectionSnapshot`, après `icon?: string;` :

```typescript
  hostname?: string;
```

- [ ] **Step 2 : Parser le hostname dans le snapshot dérivé**

Dans `DecisionPrompt.svelte`, dans l'objet retourné par `$derived.by` (après la ligne `icon: raw.process?.icon || raw.icon || undefined,`) :

```typescript
        hostname: raw.hostname || undefined,
```

- [ ] **Step 3 : Ligne résumé + destination lisible**

Dans `DecisionPrompt.svelte`, ajouter une dérivation (près des autres `$derived`) :

```typescript
  // Cible lisible : hostname si connu, sinon IP. / Human-readable target: hostname if known, else IP.
  const destLabel = $derived(snapshot?.hostname || snapshot?.destination?.ip || '--');
  const appLabel = $derived(snapshot?.process_name || fr.conn_unknown);
```

Dans le template, juste après le `<div class="app-header">…</div>` (avant `<div class="detail-grid">`), insérer la ligne résumé :

```svelte
      <div class="summary-line">
        <span class="summary-app">{appLabel}</span>
        <span class="summary-arrow">→</span>
        <span class="summary-target font-mono">{destLabel}:{snapshot.destination?.port || '--'}</span>
      </div>
```

Et remplacer la valeur de la ligne « destination » du `detail-grid` (`{snapshot.destination?.ip || '--'}:{snapshot.destination?.port || '--'}`) par un rendu host-primaire :

```svelte
          <span class="detail-value font-mono">
            {#if snapshot.hostname}{snapshot.hostname}<span class="text-tertiary text-xs"> ({snapshot.destination?.ip})</span>{:else}{snapshot.destination?.ip || '--'}{/if}:{snapshot.destination?.port || '--'}
          </span>
```

Ajouter dans le `<style>` :

```css
  .summary-line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--font-size-base);
    padding: var(--space-2) 0;
  }
  .summary-app { font-weight: var(--font-weight-semibold); color: var(--text-primary); }
  .summary-arrow { color: var(--text-tertiary); }
  .summary-target { color: var(--accent-cyan); }
```

- [ ] **Step 4 : Vérifier le build UI**

Run: `cd crates/ui && npm run build`
Expected: build réussi, aucune erreur TypeScript/Svelte.

- [ ] **Step 5 : Commit**

```bash
git add crates/ui/src/lib/types/index.ts crates/ui/src/lib/components/learning/DecisionPrompt.svelte
git commit -m "feat(ui): affiche le hostname de destination et une ligne resume dans le prompt"
```

---

### Task 2 : `DnsResolver` consulte le snoop-cache avant le reverse

**Files:**
- Modify: `crates/infra/src/dns/mod.rs` (struct `DnsResolver`, `new`, `resolve`)
- Test: même fichier, `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `DnsSnoopCache::get(&self, ip: &IpAddr) -> Option<String>` (de `snooper.rs`).
- Produces: `DnsResolver::with_snoop(capacity: usize, ttl_secs: u64, snoop: Arc<DnsSnoopCache>) -> Self` ; comportement inchangé de `resolve` quand aucun snoop n'est fourni.

- [ ] **Step 1 : Test qui échoue — le snoop a priorité**

Dans `crates/infra/src/dns/mod.rs`, `mod tests`, ajouter :

```rust
    #[tokio::test]
    async fn resolve_prefers_snoop_cache_over_reverse() {
        use crate::dns::snooper::DnsSnoopCache;
        let snoop = Arc::new(DnsSnoopCache::new(300));
        let ip: IpAddr = "93.184.216.34".parse().unwrap();
        snoop.insert(ip, "example.com".to_string(), Some(300));
        let resolver = DnsResolver::with_snoop(128, 300, snoop);
        let got = resolver.resolve(ip).await.unwrap();
        assert_eq!(got, Some("example.com".to_string()));
    }
```

- [ ] **Step 2 : Lancer le test (échec de compilation attendu)**

Run: `cargo test -p syswall-infra dns::mod::tests::resolve_prefers_snoop_cache_over_reverse 2>&1 | tail -20`
Expected: FAIL — `with_snoop` n'existe pas.

- [ ] **Step 3 : Implémenter le front-cache**

Dans `struct DnsResolver`, ajouter le champ :

```rust
    /// Cache de snooping DNS consulté avant le reverse-DNS (None = désactivé).
    /// DNS snoop cache consulted before reverse-DNS (None = disabled).
    snoop: Option<Arc<crate::dns::snooper::DnsSnoopCache>>,
```

Dans `impl DnsResolver`, initialiser `snoop: None` dans `new`, et ajouter :

```rust
    /// Comme `new`, mais consulte d'abord un cache de snooping DNS.
    /// Like `new`, but consults a DNS snoop cache first.
    pub fn with_snoop(
        capacity: usize,
        ttl_secs: u64,
        snoop: Arc<crate::dns::snooper::DnsSnoopCache>,
    ) -> Self {
        let mut r = Self::new(capacity, ttl_secs);
        r.snoop = Some(snoop);
        r
    }
```

Au tout début de `async fn resolve` (avant la consultation du cache LRU) :

```rust
        // Le domaine réellement demandé par l'appli prime sur le PTR reverse.
        // The domain the app actually requested wins over the reverse PTR.
        if let Some(ref snoop) = self.snoop
            && let Some(host) = snoop.get(&ip)
        {
            return Ok(Some(host));
        }
```

- [ ] **Step 4 : Le test passe**

Run: `cargo test -p syswall-infra dns:: 2>&1 | tail -20`
Expected: PASS (tous les tests dns, dont le nouveau).

- [ ] **Step 5 : Commit**

```bash
git add crates/infra/src/dns/mod.rs
git commit -m "feat(dns): le resolver consulte le cache de snooping avant le reverse-DNS"
```

---

### Task 3 : Extraction du payload DNS + insertion cache (consommateur, partie testable)

**Files:**
- Create: `crates/infra/src/dns/observer.rs`
- Modify: `crates/infra/src/dns/mod.rs:1` (déclarer `pub mod observer;`)
- Test: dans `observer.rs`

**Interfaces:**
- Consumes: `parse_dns_response(&[u8]) -> Vec<(String, IpAddr, u32)>` et `DnsSnoopCache` (de `snooper.rs`) ; `etherparse` (déjà dépendance).
- Produces:
  - `pub fn extract_udp_payload(packet: &[u8]) -> Option<&[u8]>` — payload UDP d'un paquet IPv4/IPv6 (couche 3), ou `None`.
  - `pub fn ingest_dns_packet(packet: &[u8], cache: &DnsSnoopCache) -> usize` — parse + insère, renvoie le nombre d'entrées ajoutées.

- [ ] **Step 1 : Tests qui échouent**

Créer `crates/infra/src/dns/observer.rs` :

```rust
//! Observation DNS : extrait les réponses DNS des paquets NFQUEUE et alimente le cache.
//! DNS observation: extracts DNS responses from NFQUEUE packets and feeds the cache.

use crate::dns::snooper::{parse_dns_response, DnsSnoopCache};

/// Extrait le payload UDP d'un paquet IP (couche 3). Retourne None si non-UDP ou tronqué.
/// Extract the UDP payload from an IP packet (layer 3). Returns None if non-UDP or truncated.
pub fn extract_udp_payload(packet: &[u8]) -> Option<&[u8]> {
    let headers = etherparse::PacketHeaders::from_ip_slice(packet).ok()?;
    match headers.transport {
        Some(etherparse::TransportHeader::Udp(_)) => Some(headers.payload),
        _ => None,
    }
}

/// Parse un paquet (couche 3) comme réponse DNS et insère chaque A/AAAA dans le cache.
/// Parse a (layer-3) packet as a DNS response and insert each A/AAAA into the cache.
/// Retourne le nombre d'associations insérées. / Returns the number of mappings inserted.
pub fn ingest_dns_packet(packet: &[u8], cache: &DnsSnoopCache) -> usize {
    let Some(payload) = extract_udp_payload(packet) else {
        return 0;
    };
    let records = parse_dns_response(payload);
    let n = records.len();
    for (host, ip, ttl) in records {
        cache.insert(ip, host, Some(ttl));
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    /// Construit un paquet IPv4/UDP (sport 53) encapsulant `dns_payload`.
    /// Build an IPv4/UDP (sport 53) packet wrapping `dns_payload`.
    fn ipv4_udp_dns(dns_payload: &[u8]) -> Vec<u8> {
        let builder = etherparse::PacketBuilder::ipv4([8, 8, 8, 8], [10, 0, 0, 1], 64)
            .udp(53, 54321);
        let mut out = Vec::new();
        builder.write(&mut out, dns_payload).unwrap();
        out
    }

    /// Réponse DNS A pour example.com → 93.184.216.34 (repris du test de snooper.rs).
    /// DNS A answer for example.com → 93.184.216.34.
    #[rustfmt::skip]
    fn dns_a_response() -> Vec<u8> {
        vec![
            0x00, 0x01, 0x81, 0x80, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
            0x07, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 0x03, b'c', b'o', b'm', 0x00,
            0x00, 0x01, 0x00, 0x01,
            0xC0, 0x0C, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x01, 0x2C, 0x00, 0x04,
            93, 184, 216, 34,
        ]
    }

    #[test]
    fn ingest_populates_cache_from_ipv4_udp_dns() {
        let cache = DnsSnoopCache::new(300);
        let packet = ipv4_udp_dns(&dns_a_response());
        let n = ingest_dns_packet(&packet, &cache);
        assert_eq!(n, 1);
        let ip: IpAddr = "93.184.216.34".parse().unwrap();
        assert_eq!(cache.get(&ip), Some("example.com".to_string()));
    }

    #[test]
    fn ingest_ignores_non_udp() {
        let cache = DnsSnoopCache::new(300);
        let tcp = etherparse::PacketBuilder::ipv4([8, 8, 8, 8], [10, 0, 0, 1], 64)
            .tcp(53, 54321, 0, 1024);
        let mut packet = Vec::new();
        tcp.write(&mut packet, &[]).unwrap();
        assert_eq!(ingest_dns_packet(&packet, &cache), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn ingest_ignores_garbage() {
        let cache = DnsSnoopCache::new(300);
        assert_eq!(ingest_dns_packet(&[0xff, 0x00, 0x01], &cache), 0);
    }
}
```

Déclarer le module dans `crates/infra/src/dns/mod.rs` (après `pub mod snooper;`) : ajouter `pub mod observer;`.

- [ ] **Step 2 : Lancer les tests (échec attendu si l'API etherparse diffère)**

Run: `cargo test -p syswall-infra dns::observer 2>&1 | tail -30`
Expected: d'abord vérifier la compilation. Si `PacketHeaders`/`PacketBuilder` diffèrent dans la version d'etherparse du lockfile, adapter selon `crates/infra/src/nfqueue/parser.rs` (qui utilise déjà etherparse) puis relancer.

- [ ] **Step 3 : (au besoin) aligner sur l'API etherparse de `parser.rs`**

Ouvrir `crates/infra/src/nfqueue/parser.rs`, repérer comment il parse un paquet couche-3 (types `PacketHeaders`/`from_ip_slice` ou équivalent `Ipv4Slice`/`UdpSlice`), et ajuster `extract_udp_payload` en conséquence. Ne change pas la signature publique.

- [ ] **Step 4 : Les tests passent**

Run: `cargo test -p syswall-infra dns::observer 2>&1 | tail -20`
Expected: PASS (3 tests).

- [ ] **Step 5 : Commit**

```bash
git add crates/infra/src/dns/observer.rs crates/infra/src/dns/mod.rs
git commit -m "feat(dns): extraction du payload DNS et alimentation du cache de snooping"
```

---

### Task 4 : Chaîne nft `input` d'observation DNS

**Files:**
- Create: `crates/infra/src/nftables/translator/dns_observe_chain.rs`
- Modify: `crates/infra/src/nftables/translator/mod.rs:5` (déclarer le module)
- Test: dans `dns_observe_chain.rs`

**Interfaces:**
- Consumes: modèle de `crates/infra/src/nftables/translator/interception_chain.rs` (`build_interception_chain_rules(table, queue_num)`).
- Produces: `pub fn build_dns_observe_chain_rules(table_name: &str, queue_num: u16) -> Vec<String>` — règles nft créant une chaîne `input` qui queue les réponses DNS.

- [ ] **Step 1 : Test qui échoue**

Créer `crates/infra/src/nftables/translator/dns_observe_chain.rs` :

```rust
//! Chaîne nft d'observation des réponses DNS (hook input), pour le snooping IP→domaine.
//! nft DNS-response observation chain (input hook), for IP→domain snooping.

/// Construit les règles de la chaîne `dns_observe` : queue les réponses DNS (udp sport 53)
/// vers `queue_num` avec `bypass` (fail-open — n'interrompt jamais la résolution DNS).
/// Build the `dns_observe` chain rules: queue DNS responses (udp sport 53) to `queue_num`
/// with `bypass` (fail-open — never disrupts DNS resolution).
pub fn build_dns_observe_chain_rules(table_name: &str, queue_num: u16) -> Vec<String> {
    vec![
        format!(
            "add chain inet {table_name} dns_observe {{ type filter hook input priority 0 ; policy accept ; }}"
        ),
        format!(
            "add rule inet {table_name} dns_observe udp sport 53 queue num {queue_num} bypass"
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_input_hook_chain() {
        let rules = build_dns_observe_chain_rules("syswall", 7);
        assert!(rules[0].contains("hook input"));
        assert!(rules[0].contains("dns_observe"));
    }

    #[test]
    fn queues_dns_responses_with_bypass() {
        let rules = build_dns_observe_chain_rules("syswall", 7);
        assert!(rules[1].contains("udp sport 53"));
        assert!(rules[1].contains("queue num 7"));
        assert!(rules[1].contains("bypass"));
    }
}
```

Déclarer le module dans `crates/infra/src/nftables/translator/mod.rs` (à côté de `pub mod interception_chain;`) : `pub mod dns_observe_chain;`.

- [ ] **Step 2 : Lancer le test (échec attendu)**

Run: `cargo test -p syswall-infra dns_observe_chain 2>&1 | tail -20`
Expected: FAIL puis PASS après création (fonction pure).

- [ ] **Step 3 : Vérifier que les règles sont appliquées au montage**

Repérer où `build_interception_chain_rules` est invoqué (assemblage du ruleset au démarrage) :
Run: `grep -rn "build_interception_chain_rules" crates/infra/src crates/daemon/src`
Ajouter `build_dns_observe_chain_rules(table, DNS_QUEUE_NUM)` au même endroit (mêmes `extend`/`push`), avec une constante `pub const DNS_QUEUE_NUM: u16 = 7;` définie près de la constante de queue d'interception existante. La chaîne est supprimée avec la table syswall existante (teardown `delete table` / flush déjà en place — vérifier qu'aucun teardown ne cible les chaînes nommément ; sinon ajouter `dns_observe`).

- [ ] **Step 4 : Tests infra verts**

Run: `cargo test -p syswall-infra nftables 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5 : Commit**

```bash
git add crates/infra/src/nftables/translator/dns_observe_chain.rs crates/infra/src/nftables/translator/mod.rs
git commit -m "feat(nft): chaine input d'observation des reponses DNS (queue bypass)"
```

---

### Task 5 : Câblage bootstrap (cache partagé + resolver + consommateur + éviction)

**Files:**
- Modify: `crates/daemon/src/bootstrap.rs`
- Modify: `crates/daemon/src/main.rs` (spawn de la boucle consommateur + tâche d'éviction, via le `Supervisor` existant)

**Interfaces:**
- Consumes: `DnsSnoopCache::new`, `DnsResolver::with_snoop`, `observer::ingest_dns_packet`, `NfqueueInterceptor::new(queue_num, max_queued, OverflowPolicy)`, `dns_observe_chain::DNS_QUEUE_NUM`, `DnsSnoopCache::evict_expired`.
- Produces: le `ConnectionService` (via son `DnsResolver`) voit désormais les domaines snoopés ; le daemon consomme la queue DNS et évince périodiquement.

- [ ] **Step 1 : Cache partagé + resolver snoop-aware**

Dans `bootstrap.rs`, là où le `DnsResolver` est construit (repérer `DnsResolver::new` — actuellement injecté dans `ConnectionService`), remplacer par :

```rust
    // Cache de snooping DNS partagé entre le consommateur NFQUEUE et le resolver.
    // DNS snoop cache shared between the NFQUEUE consumer and the resolver.
    let dns_snoop = Arc::new(syswall_infra::dns::snooper::DnsSnoopCache::new(300));
    let dns_resolver = Arc::new(syswall_infra::dns::DnsResolver::with_snoop(
        4096,
        300,
        dns_snoop.clone(),
    ));
```

Retourner/rendre accessible `dns_snoop` au niveau où les tâches de fond sont lancées (l'ajouter au contexte/bundle renvoyé par le bootstrap, à côté des autres `Arc`).

- [ ] **Step 2 : Boucle consommateur DNS (fail-open)**

Là où les tâches de fond du daemon sont enregistrées auprès du `Supervisor` (`main.rs`), ajouter une tâche qui ouvre la queue `DNS_QUEUE_NUM` et, pour chaque paquet, appelle `ingest_dns_packet` puis ACCEPT. Le `NfqueueInterceptor` étant orienté « Connection », lire les paquets bruts via `nfq::Queue` directement dans `observer.rs` — ajouter dans `observer.rs` :

```rust
/// Ouvre la queue `queue_num`, ingère chaque réponse DNS, verdict ACCEPT (toujours).
/// Boucle bloquante jusqu'à annulation. / Open queue `queue_num`, ingest each DNS response,
/// verdict ACCEPT (always). Blocking loop until cancelled.
pub fn run_dns_observer(
    queue_num: u16,
    cache: std::sync::Arc<DnsSnoopCache>,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<(), syswall_domain::errors::DomainError> {
    use nfq::{Queue, Verdict};
    let mut queue = Queue::open()
        .map_err(|e| syswall_domain::errors::DomainError::Infrastructure(format!("dns queue open: {e}")))?;
    queue
        .bind(queue_num)
        .map_err(|e| syswall_domain::errors::DomainError::Infrastructure(format!("dns queue bind: {e}")))?;
    while !cancel.is_cancelled() {
        match queue.recv() {
            Ok(mut msg) => {
                // Best-effort : une erreur d'ingestion ne doit jamais bloquer le DNS.
                // Best-effort: an ingestion error must never block DNS.
                let _ = ingest_dns_packet(msg.get_payload(), &cache);
                msg.set_verdict(Verdict::Accept);
                let _ = queue.verdict(msg);
            }
            Err(_) => break,
        }
    }
    Ok(())
}
```

Vérifier l'API `nfq` exacte (`get_payload`, `recv`, `set_verdict`, `verdict`) contre `crates/infra/src/nfqueue/interceptor.rs` et aligner si besoin (mêmes appels que la boucle existante). Enregistrer un thread bloquant dédié (comme l'interception NFQUEUE actuelle) auprès du `Supervisor`, en lui passant `dns_snoop.clone()` et le `CancellationToken` existant.

- [ ] **Step 3 : Tâche d'éviction périodique**

Toujours au niveau des tâches de fond, ajouter une tâche async (mêmes patterns que les autres tâches périodiques du daemon) :

```rust
    // Éviction périodique des entrées DNS expirées. / Periodic eviction of expired DNS entries.
    let evict_cache = dns_snoop.clone();
    let evict_cancel = cancel.clone();
    supervisor.spawn("dns-evict", async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = evict_cancel.cancelled() => break,
                _ = tick.tick() => evict_cache.evict_expired(),
            }
        }
    });
```

Adapter `supervisor.spawn(...)` à la signature réelle du `Supervisor` (vérifier dans `crates/daemon/src/supervisor.rs`).

- [ ] **Step 4 : Compilation + tests daemon**

Run: `cargo build -p syswall-daemon 2>&1 | tail -20` puis `cargo test -p syswall-daemon 2>&1 | tail -20`
Expected: build OK, tests PASS.

- [ ] **Step 5 : Commit**

```bash
git add crates/daemon/src/bootstrap.rs crates/daemon/src/main.rs crates/infra/src/dns/observer.rs
git commit -m "feat(daemon): cable le snooping DNS (cache partage, consommateur NFQUEUE, eviction)"
```

---

### Task 6 : Versionnage, CHANGELOG, vérifications finales

**Files:**
- Modify: `Cargo.toml:19`, `Cargo.lock`, `CHANGELOG.md`, `CHANGELOG.en.md`

- [ ] **Step 1 : Bump de version**

Dans `Cargo.toml`, `[workspace.package]` : `version = "0.3.9"`. Puis :
Run: `PATH="$HOME/.cargo/bin:$PATH" cargo update --workspace 2>&1 | tail -5`
Expected: les crates syswall passent en 0.3.9 dans `Cargo.lock`.

- [ ] **Step 2 : Entrées CHANGELOG (FR + EN)**

En tête de `CHANGELOG.md`, avant `## [0.3.8]` :

```markdown
## [0.3.9] - 2026-08-05 · « Le prompt dit enfin ce que c'est »

### Ajoute

- **Identité de destination dans le prompt de nouvelle connexion** : la fenêtre affiche désormais le nom d'hôte de la destination (`github.com`) plutôt qu'une simple IP, avec une ligne résumé « Application → hôte:port ». Le nom provient d'abord du **snooping DNS** (le domaine réellement demandé par l'application, capturé en observant les réponses DNS via une chaîne nftables `input` dédiée en `queue … bypass` fail-open), avec repli sur le reverse-DNS puis sur l'IP brute. Câblé : le résolveur consulte le cache de snooping avant le reverse ; un consommateur NFQUEUE alimente ce cache ; une tâche de fond évince les entrées expirées. Aucune nouvelle capability (réutilise `CAP_NET_ADMIN`).
```

En tête de `CHANGELOG.en.md`, avant `## [0.3.8]`, la traduction :

```markdown
## [0.3.9] - 2026-08-05 · "The prompt finally says what it is"

### Added

- **Destination identity in the new-connection prompt**: the window now shows the destination hostname (`github.com`) instead of a bare IP, with an "Application → host:port" summary line. The name comes first from **DNS snooping** (the domain the application actually requested, captured by observing DNS responses through a dedicated `input` nftables chain in fail-open `queue … bypass`), falling back to reverse-DNS then to the raw IP. Wiring: the resolver consults the snoop cache before the reverse lookup; an NFQUEUE consumer feeds that cache; a background task evicts expired entries. No new capability (reuses `CAP_NET_ADMIN`).
```

- [ ] **Step 3 : Suite complète + lint**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo fmt --all --check && cargo clippy --workspace -- -D warnings 2>&1 | tail -20 && cargo test --workspace --exclude ui 2>&1 | tail -20`
Expected: fmt clean, clippy sans warning, tous les tests PASS.

- [ ] **Step 4 : Vérification E2E (comportement UI modifié)**

Lancer le daemon + l'UI, générer une connexion sortante (ex. `curl https://example.com` depuis une autre appli non whitelistée), vérifier que le prompt affiche `example.com` (snoop) et la ligne résumé. Documenter le résultat.

- [ ] **Step 5 : Commit**

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md CHANGELOG.en.md
git commit -m "chore(release): 0.3.9 — identite de destination dans le prompt"
```

- [ ] **Step 6 : Release (après CI verte)**

Pousser `main`, **attendre la CI verte (9/9, filtrer `gh run list` par SHA)**, puis tag annoté `git tag -a v0.3.9 -m "v0.3.9 — Le prompt dit enfin ce que c'est"`, push du tag, et `gh release create v0.3.9` avec les notes extraites de la section 0.3.9 du CHANGELOG.

---

## Auto-revue (couverture spec)

- ✅ Observation DNS via NFQUEUE entrant → Task 3 (consommateur) + Task 4 (chaîne nft) + Task 5 (boucle).
- ✅ Enrichissement snoop-puis-reverse → Task 2 (front-cache dans `DnsResolver`), sans toucher la couche app (hexagonal préservé).
- ✅ Sérialisation hostname → déjà en place (`Connection::snapshot()` copie `remote_hostname`) ; rien à faire.
- ✅ UI (ligne résumé, host primaire, IP secondaire) → Task 1.
- ✅ Invariants (bypass fail-open, ACCEPT sur erreur, TTL, éviction, zéro régression) → Tasks 4/5.
- ✅ Versionnage 0.3.9 + CHANGELOG FR/EN + E2E → Task 6.
