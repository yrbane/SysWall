# Spec — Sous-projet C.2 : Blocage actif via NFQUEUE (block-while-pending)

> Date : 2026-05-05
> Branche cible : `main`
> Cycle : spec → plan → exécution TDD → commits incrémentaux
> Pré-requis : sous-projets A (sécurité) et B (hygiène) complétés (HEAD post-B)

## Contexte

L'audit du 2026-05-04 et le README revendiquent que SysWall « intercepte, analyse et **contrôle** le trafic réseau ». Or l'implémentation actuelle est **observation-only** :

- conntrack remonte les connexions **après** que le kernel les a déjà autorisées,
- les règles nft prennent effet sur les flux suivants, pas sur celui qui a déclenché la `PendingDecision`,
- le `default_policy = "ask"` produit un `ConnectionVerdict::PendingDecision` qui est purement symbolique côté kernel — le paquet est déjà passé.

Conséquence : un utilisateur qui voit le popup « Firefox veut joindre `analytics.example.com` » ne peut pas réellement empêcher la première requête de partir. Le « block-while-pending » à la Little Snitch n'existe pas.

## Objectifs

- Intercepter en temps réel les premiers paquets de chaque nouveau flux via **netfilter NFQUEUE**.
- Suspendre le verdict du paquet jusqu'à ce que :
  - une règle existante s'applique (verdict immédiat),
  - une `PendingDecision` existante non-expirée matche la `dedup_key` (verdict attaché à la même attente),
  - une nouvelle `PendingDecision` soit résolue par l'utilisateur (verdict de l'action choisie),
  - un timeout kernel (~30 s) expire (verdict block + audit).
- Déduplication baked-in : un seul popup par `dedup_key` (Firefox sur `cdn.example.com:443/tcp` ne spam pas l'UI).

Hors-scope (différé en C.1 pour un cycle ultérieur ou éventuellement un autre sous-projet) :
- Watchdog daemon (`watchdog_interval_secs`).
- Rotation journal (`journal_retention_days`).
- Câblage des autres champs config dormants (`log_dir`, `theme`, `refresh_interval_ms`, `default_timeout_action`, `overflow_action`).
- UI changes au-delà du minimum nécessaire pour afficher le popup avec le nouveau flux NFQUEUE.

## Décisions de conception (validées avec l'utilisateur)

- **Dep crate** : `nfq = "0.4"` (haut niveau, gère le netlink/locks).
- **Architecture** : nouveau port domain `PacketInterceptor` + adapter infra `NfqueueInterceptor`. Le `LearningService` dépend du port, l'adapter est injecté au bootstrap.
- **Verdict synchrone par paquet** : le worker NFQUEUE crée un `oneshot::Sender<NfqVerdict>` par paquet et l'attache à un slot dans une map indexée par `packet_id` (kernel-attribué). Le service résout la décision et envoie le verdict via le sender.
- **Debounce baked-in** : avant de créer une nouvelle `PendingDecision`, le service interroge le `PendingDecisionRepository` par `dedup_key`. Si une pending non-expirée existe, on attache le verdict du nouveau paquet au même destin que la décision existante (un `broadcast::Receiver<NfqVerdict>` par dedup_key).
- **Bypass kernel loopback** : règle nft prioritaire `iif lo accept` AVANT le `queue num 0`. Garantit que l'IPC Unix socket UI ↔ daemon ne se queue pas elle-même (deadlock).
- **Whitelist DNS/DHCP/NTP** : les règles whitelist du sous-projet A sont déjà chargées en priorité haute → leur `accept` court-circuite la queue, donc aucun paquet de ces protocoles ne va en NFQUEUE. Bonne propriété sans rien à faire.
- **Timeout** : 30 s côté kernel (`nfnetlink_queue.queue_total` default). Si la décision n'est pas résolue à temps, kernel jette le paquet (= block silent). Audit `Severity::Warning, Category::Decision, "decision timeout: paquet drop par kernel"`.
- **Backpressure** : une queue `nfq` interne de 1024 paquets max. Si saturée, `default_overflow_action = "block"` (déjà dans la config TOML, on l'active enfin) → drop direct + audit.
- **Failure mode** : si NFQUEUE ne peut pas être ouvert au boot (pas de `CAP_NET_ADMIN`, kernel sans `nfnetlink_queue` module), le daemon démarre en mode dégradé (observation conntrack uniquement) et log un `Severity::Error, Category::System` clairement identifiable. Pas de crash boot — le mode dégradé est explicite et restituable.

## Architecture

### Nouveau port domain

`crates/domain/src/ports/interception.rs` :

```rust
use async_trait::async_trait;

use crate::entities::Connection;

/// Verdict to apply to a captured packet.
/// Verdict à appliquer à un paquet capturé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketVerdict {
    /// Allow the packet through.
    /// Laisser passer le paquet.
    Accept,
    /// Drop the packet (kernel-side).
    /// Jeter le paquet (côté kernel).
    Drop,
}

/// Intercepts the first packet of every new flow and asks for a verdict.
/// Intercepte le premier paquet de chaque nouveau flux et demande un verdict.
#[async_trait]
pub trait PacketInterceptor: Send + Sync {
    /// Start the interception loop. Each captured packet triggers a call to
    /// `handler.decide(connection)` and the returned verdict is forwarded to the kernel.
    /// Démarre la boucle d'interception. Chaque paquet capturé déclenche un appel à
    /// `handler.decide(connection)` et le verdict retourné est appliqué au kernel.
    async fn run(
        &self,
        handler: std::sync::Arc<dyn PacketDecisionHandler>,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<(), super::super::errors::DomainError>;
}

/// Handler resolving a verdict for a captured connection.
/// Handler résolvant un verdict pour une connexion capturée.
#[async_trait]
pub trait PacketDecisionHandler: Send + Sync {
    async fn decide(&self, connection: &Connection) -> Result<PacketVerdict, super::super::errors::DomainError>;
}
```

### Adapter infra

`crates/infra/src/nfqueue/mod.rs` + `nfqueue/interceptor.rs` :

```rust
pub struct NfqueueInterceptor {
    queue_num: u16,                      // default 0
    max_queued_packets: u32,             // 1024
    overflow_policy: OverflowPolicy,     // Block | Allow (from config)
}

impl NfqueueInterceptor {
    pub fn new(queue_num: u16, max_queued: u32, overflow: OverflowPolicy) -> Self { ... }
}

#[async_trait]
impl PacketInterceptor for NfqueueInterceptor {
    async fn run(&self, handler: Arc<dyn PacketDecisionHandler>, cancel: CancellationToken) -> Result<(), DomainError> {
        // 1. Open NFQUEUE via `nfq::Queue::open()`.
        // 2. Bind to queue_num.
        // 3. Set CopyPacket mode with sufficient size for parsing IP+TCP/UDP headers (~80 bytes).
        // 4. Loop: read message, parse to Connection, spawn task that:
        //    a. calls handler.decide(&connection).await
        //    b. sets verdict via `msg.set_verdict(...)` and queue.verdict(msg).
        //    c. on cancel: verdict-drop pending packets, exit loop.
    }
}
```

Le parsing du paquet (IP + L4) doit extraire le 5-tuple pour construire un `Connection`. Le PID/UID associés sont obtenus via la résolution procfs/eBPF déjà en place (`HybridProcessResolver`).

### Refactor `LearningService`

Le service implémente `PacketDecisionHandler` :

```rust
#[async_trait]
impl PacketDecisionHandler for LearningService {
    async fn decide(&self, conn: &Connection) -> Result<PacketVerdict, DomainError> {
        let resolved_conn = self.process_resolver.enrich(conn).await?;
        let evaluation = self.policy_engine.evaluate(&resolved_conn, &self.rules.snapshot().await?, self.default_policy);
        match evaluation.verdict {
            ConnectionVerdict::Allowed => Ok(PacketVerdict::Accept),
            ConnectionVerdict::Blocked | ConnectionVerdict::Ignored => Ok(PacketVerdict::Drop),
            ConnectionVerdict::PendingDecision => self.pending_verdict_for(&resolved_conn).await,
        }
    }
}

async fn pending_verdict_for(&self, conn: &Connection) -> Result<PacketVerdict, DomainError> {
    let dedup_key = conn.deduplication_key();
    // Debounce: existing pending decision?
    if let Some(existing) = self.pending_repo.find_pending_by_dedup_key(&dedup_key).await? {
        // Subscribe to the broadcast: when existing decision resolves, all subscribers get the verdict.
        let mut rx = self.verdict_broadcasts.subscribe(existing.id).await;
        // Wait or timeout (kernel will drop after 30s).
        tokio::time::timeout(Duration::from_secs(28), rx.recv()).await
            .map_err(|_| DomainError::Infrastructure("verdict timeout (kernel will drop)".into()))?
            .map_err(|e| DomainError::Infrastructure(format!("broadcast recv: {e}")))
    } else {
        // Create new PendingDecision + broadcast channel.
        let pd = PendingDecision::new(conn, dedup_key.clone(), prompt_timeout_secs);
        self.pending_repo.create(&pd).await?;
        self.event_bus.publish(DomainEvent::PendingDecisionCreated(pd.id)).await;
        let (tx, mut rx) = tokio::sync::broadcast::channel(64);
        self.verdict_broadcasts.register(pd.id, tx).await;
        // Wait for user resolution.
        tokio::time::timeout(Duration::from_secs(28), rx.recv()).await
            .map_err(|_| {
                self.audit_timeout(pd.id);
                DomainError::Infrastructure("decision timeout".into())
            })?
            .map_err(|e| DomainError::Infrastructure(format!("broadcast recv: {e}")))
    }
}
```

Une nouvelle structure interne `VerdictBroadcasts` maintient `HashMap<PendingDecisionId, broadcast::Sender<PacketVerdict>>`. Quand l'utilisateur résout via `RespondToDecision`, le service envoie le verdict via le `Sender` puis `register(id, _).remove(id)`.

Mapping action utilisateur → verdict :
- `AllowOnce`, `AlwaysAllow`, `CreateRule(allow)` → `Accept`
- `BlockOnce`, `AlwaysBlock`, `CreateRule(block)` → `Drop`
- `Ignore` → `Drop` (le paquet pending est jeté ; les futurs flux retombent sur la default policy → re-popup)

### Bootstrap

Dans `crates/daemon/src/bootstrap.rs`, après la construction du `LearningService` :

```rust
let interceptor: Arc<dyn PacketInterceptor> = Arc::new(NfqueueInterceptor::new(
    config.nfqueue.queue_num,
    config.nfqueue.max_queued,
    config.nfqueue.overflow_policy,
));
let handler = learning_service.clone() as Arc<dyn PacketDecisionHandler>;
let cancel = supervisor_cancel_token.child_token();
tokio::spawn(async move {
    if let Err(e) = interceptor.run(handler, cancel).await {
        tracing::error!(target: "nfqueue", "interception failed (degraded mode): {e}");
        // The daemon stays alive — no crash, just degraded.
    }
});
```

### Règles nft initiales

Ajouter au boot, dans `NftablesAdapter::ensure_table_exists` (ou méthode dédiée `install_nfqueue_chain`) :

```text
table inet syswall {
    chain interception {
        type filter hook output priority filter;
        policy accept;

        # Bypass loopback (UI ↔ daemon IPC).
        iif lo accept;

        # Whitelist DNS/DHCP/NTP/loopback (already added by sub-project A's whitelist logic).
        # ...

        # Active interception: queue first packet of each new outbound flow.
        ct state new queue num 0 bypass;
    }
}
```

`bypass` est crucial : si le démon meurt ou la queue est saturée, le kernel laisse passer (fail-open). On préfère fail-open à fail-closed pour ne pas couper internet en cas de bug du démon. Documenté en commentaire.

## Configuration

Nouvelle section `[nfqueue]` dans `crates/daemon/src/config.rs` et `config/default.toml` :

```toml
[nfqueue]
enabled = true
queue_num = 0
max_queued = 1024
overflow_policy = "block"   # "block" or "accept"
```

`enabled = false` → mode dégradé observation-only (l'utilisateur peut tester SysWall sans NFQUEUE).

## Flux de données

```
+---------------------+         +------------------------+
|   Linux kernel      |         |  syswall-daemon        |
|                     |         |                        |
|  socket() →         |         |   NfqueueInterceptor   |
|  connect() →        |         |   (worker tokio)       |
|  SYN packet         |         |                        |
|  ↓                  |         |   read msg from queue  |
|  nft "ct state new  | ↘       |        ↓               |
|  queue num 0"       |  ↘ msg  |   parse 5-tuple        |
|  → packet held in   |    ↘    |        ↓               |
|  kernel queue       |     ↘   |   build Connection     |
|                     |      ↘  |        ↓               |
|                     |       → |   handler.decide()     |
|                     |         |        │               |
|                     |         |   .─────────.          |
|                     |         |  ( PolicyEngine )      |
|                     |         |   '─────────'          |
|                     |         |        │               |
|                     |         |  Allowed → Accept      |
|                     |         |  Blocked → Drop        |
|                     |         |  Pending → wait on     |
|                     |         |    user via broadcast  |
|                     |         |    (28s budget)        |
|                     |         |        │               |
|                     |   verdict       │               |
|  ←──────────────────┼───────  |   queue.verdict(msg)   |
|  drop / accept      |         |                        |
+---------------------+         +------------------------+
```

## Tests

### Domain (port + types)

`crates/domain/src/ports/interception.rs::tests` :
- `packet_verdict_equality` — `Accept != Drop`.
- Pas de mock du trait ici (testé via fakes en app).

### App (`LearningService` PacketDecisionHandler)

`crates/app/src/services/learning_service::tests` (ajouts) :
1. `decide_existing_allow_rule_returns_accept` — rule matches with effect Allow → verdict Accept, no PendingDecision created.
2. `decide_no_rule_default_block_returns_drop` — default Block → Drop, no PendingDecision.
3. `decide_pending_creates_decision_and_waits_for_user` — default Ask → PendingDecision created → user resolves AllowOnce → verdict Accept received.
4. `decide_pending_dedup_attaches_to_existing` — two flows same dedup_key → ONE PendingDecision, both await the same broadcast → both get the same verdict.
5. `decide_pending_timeout_returns_drop_with_audit` — user never responds → 28s timeout → Drop + audit Severity=Warning.
6. `block_once_resolves_to_drop` — user picks BlockOnce → verdict Drop.
7. `ignore_resolves_to_drop` — user picks Ignore → verdict Drop (kernel drops the held packet).

Use the existing `FakePendingDecisionRepository` and `FakeUserNotifier`. No fake `PacketInterceptor` needed for these — they test the handler directly.

### Infra (`NfqueueInterceptor`)

`crates/infra/src/nfqueue/interceptor.rs::tests` :
1. `parses_tcp_syn_into_connection` — synthetic IPv4 TCP SYN bytes → `Connection { protocol: Tcp, state: New, ... }`. No real NFQUEUE.
2. `parses_udp_into_connection` — synthetic IPv4 UDP packet.
3. `verdict_accept_translates_to_nfq_verdict_accept` — internal mapping test.
4. `overflow_policy_block_yields_drop_when_queue_full` — synthetic queue saturation → Drop verdicts.

Real NFQUEUE binding test is gated by `SYSWALL_TEST_NFQUEUE=1` (requires CAP_NET_ADMIN + a real kernel queue) and exists as a smoke test. Must be runnable manually:

```bash
SYSWALL_TEST_NFQUEUE=1 cargo test -p syswall-infra --test nfqueue_smoke -- --nocapture
```

### Daemon (bootstrap)

`crates/daemon/tests/nfqueue_bootstrap_test.rs` :
1. Bootstrap completes when `nfqueue.enabled = false` (degraded mode).
2. Bootstrap reports a clean error and stays in degraded mode when the queue cannot be opened (env-gated).

## Audit & observabilité

Nouveaux audit events :
- `Severity::Info, Category::System, "nfqueue: interception started, queue_num={}"` — au boot succès.
- `Severity::Error, Category::System, "nfqueue: interception failed, daemon in degraded mode: {reason}"` — au boot échec.
- `Severity::Warning, Category::Decision, "decision timeout: kernel will drop packet"` — par décision expirée.
- `Severity::Warning, Category::Decision, "queue overflow: dropping packet (overflow_policy=block)"` — par drop overflow.

Nouvelle métrique tracing : `tracing::info!(target: "nfqueue", "verdict {verdict} for {dedup_key} in {duration_ms}ms")` pour chaque verdict, niveau debug en production.

## Risques & mitigations

| Risque | Probabilité | Mitigation |
|---|---|---|
| Démon down → tout le trafic outbound bloqué | Critique sans mitigation | `bypass` flag dans la règle nft `queue num 0 bypass` → kernel laisse passer si le démon n'est pas connecté. Fail-open by design. |
| Queue saturée (1024 paquets en attente) | Possible sous bursts | `overflow_policy = "block"` par défaut + audit. L'utilisateur peut basculer en `"accept"` s'il préfère fail-open complet. |
| Latence de decision > 28s → kernel drop | Si l'utilisateur tarde | Documenté ; audit warning ; pas de retry kernel (le client se chargera de réessayer si TCP, et la nouvelle requête tombera sur la pending toujours active grâce au debounce). |
| Boucle infinie : daemon parle au DNS via NFQUEUE → DNS bloqué tant qu'il n'a pas decidé son DNS → deadlock | Réelle | Whitelist DNS du sous-projet A est en haut des règles, ses paquets `accept` avant la queue. Vérifier en test d'intégration. |
| Parsing manuel IP/TCP/UDP errors | Possible (octets malformés) | Bibliothèque `etherparse` (ou parsing manuel défensif avec `?`-propagation) ; tests sur paquets fuzzés. |
| Verdict mismatch (broadcast envoie sur le mauvais ID) | Bug potentiel | Tests rigoureux du dedup ; assertion `pd.id == expected_id` à chaque envoi. |
| `nfq` crate compatibilité kernel | Dépend des distros | Tester sur kernel ≥ 5.4 (déjà prérequis pour eBPF). Documenter en README. |

## Critères de succès

- [ ] Nouveau port `PacketInterceptor` + `PacketDecisionHandler` dans `domain`.
- [ ] Adapter `NfqueueInterceptor` dans `infra` avec parsing IPv4+IPv6 / TCP+UDP.
- [ ] `LearningService` implémente `PacketDecisionHandler`, dépend du port.
- [ ] Nouvelle config `[nfqueue]` dans TOML + `default.toml` mis à jour.
- [ ] Bootstrap lance le worker NFQUEUE, en mode dégradé si échec d'ouverture.
- [ ] Règle nft initiale avec `iif lo accept` + whitelist + `queue num 0 bypass`.
- [ ] Tests unitaires : 7 cas LearningService + 4 cas parser.
- [ ] Smoke test gated par `SYSWALL_TEST_NFQUEUE`.
- [ ] Debounce dedup_key vérifié par le test 4 ci-dessus.
- [ ] Documentation README+CHANGELOG en FR+EN.
- [ ] `cargo clippy --workspace --exclude ui --all-targets -- -D warnings` toujours vert.
- [ ] Tous les tests existants passent (au minimum 308).

## Plan d'exécution (commits ciblés)

| # | Étape | Type |
|---|---|---|
| 1 | Port `PacketInterceptor` + `PacketDecisionHandler` (domain) | feat |
| 2 | Fakes `FakePacketInterceptor` + helper handler (app fakes) | feat |
| 3 | Tests `LearningService::decide` (7 cas) — rouges → verts | test+refactor |
| 4 | Refactor `LearningService` : nouveau champ `verdict_broadcasts`, `pending_verdict_for` | refactor |
| 5 | Adapter `NfqueueInterceptor` skeleton + parsing IPv4 TCP | feat |
| 6 | Parsing IPv6 + UDP + tests | feat |
| 7 | Config `[nfqueue]` (struct + default.toml) | feat |
| 8 | Bootstrap câble l'interceptor + spawn worker + degraded mode | feat |
| 9 | Règle nft initiale (interception chain + bypass + queue) | feat |
| 10 | Audit events nouveaux (boot success/fail, decision timeout, queue overflow) | feat |
| 11 | Smoke test gated `SYSWALL_TEST_NFQUEUE` | test |
| 12 | Documentation README + CHANGELOG | docs |

12 commits estimés. Chaque commit doit compiler et passer les tests à lui seul.

## Dépendances Cargo

- `nfq = "0.4"` dans `crates/infra/Cargo.toml` (workspace dep).
- `etherparse = "0.16"` (ou parsing manuel) dans `crates/infra/Cargo.toml` (workspace dep).

Vérifier les CVE via `cargo audit` avant l'ajout (à venir en sous-projet ultérieur ou lancer manuellement).

## Hors-scope explicite

- Watchdog daemon, rotation journal, autres champs config dormants → sous-projet C.1 ultérieur.
- UI changes au-delà du minimum (popup affiche déjà les nouvelles décisions via le flux existant).
- IPv6 NFQUEUE : couvert par parsing, mais la règle nft cible `inet syswall` qui couvre IPv4 + IPv6 nativement.
- Performance benchmarks (ce n'est pas un release-blocker pour V0.2 ; tracking en V0.3).

---

*Spec rédigée le 2026-05-05. À approuver avant transition vers writing-plans.*
