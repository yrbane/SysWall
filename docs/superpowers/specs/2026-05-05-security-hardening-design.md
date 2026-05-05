# Spec — Sous-projet A : Renforcement sécurité critique SysWall

> Date : 2026-05-05
> Branche cible : `main`
> Cycle : spec → plan d'implémentation (writing-plans) → exécution TDD → commits incrémentaux
> Sous-projets suivants (non couverts ici) : B (hygiène code), C (fonctionnel manquant), D (UX bloquants), E (design polish)

## Contexte

L'audit du 2026-05-04 (`docs/audit-2026-05-04.md`) a identifié 5 vulnérabilités majeures :

1. **Anti-lockout 30s revendiqué dans le README mais inexistant** : le champ `rollback_timeout_secs` est dans `config.rs:47` mais signalé `never read` par le compilateur. Aucun timer ni probe de connectivité.
2. **gRPC sans authentification de peer** : socket Unix `0o660` + `chown` best-effort, mais aucun `SO_PEERCRED`. Tout binaire dans le groupe `syswall` peut désactiver le pare-feu.
3. **`system/syswall.service` non durci** : `User=root` sans capability bounding, sandbox, ni filtres syscalls.
4. **CSP Tauri désactivée** : `"csp": null` dans `tauri.conf.json`.
5. **Limites gRPC absentes** : pas de `max_decoding_message_size` ni `concurrency_limit` → DoS local trivial.

## Objectifs

- Rendre la promesse anti-lockout **réelle et testée**.
- Authentifier les appelants gRPC avec `SO_PEERCRED`.
- Faire tourner le daemon en utilisateur dédié `syswall` avec capabilities ambient.
- Activer une CSP stricte dans Tauri.
- Borner gRPC contre le DoS local.

Hors-scope : tout ce qui touche les sous-projets B/C/D/E.

## Décisions de conception (validées avec l'utilisateur)

- **Probe de connectivité** : TCP configurable, défauts `1.1.1.1:53` + `[2606:4700:4700::1111]:53`, succès si **au moins un endpoint** répond. **6 tentatives** aux instants T=0, 5, 10, 15, 20, 25 s ; rollback déclenché à T=30 s si toutes ont échoué. Per-endpoint timeout 2 s (deux endpoints sondés en parallèle par tick).
- **Privilèges daemon** : utilisateur système `syswall` avec `AmbientCapabilities` (option 2 de la discussion), pas `User=root`.
- **Branche** : développement direct sur `main`, commits incrémentaux atomiques.

## Architecture

### A.1 — Anti-lockout 30s

**Nouveau port** (`crates/domain/src/ports/connectivity.rs`) :

```rust
#[async_trait]
pub trait ConnectivityProbe: Send + Sync {
    async fn probe(&self) -> Result<ProbeOutcome, ProbeError>;
}

pub enum ProbeOutcome {
    Reachable,        // Au moins un endpoint a répondu (ConnRefused inclus)
    Unreachable,      // Tous les endpoints ont timeout
}

pub enum ProbeError {
    Timeout,
    Configuration(String),  // Endpoints invalides
}
```

**Service applicatif** (`crates/app/src/services/antilockout_guard.rs`) :

```rust
pub struct AntilockoutGuard {
    probe: Arc<dyn ConnectivityProbe>,
    firewall: Arc<dyn FirewallEngine>,
    audit: Arc<dyn AuditRepository>,
    timeout: Duration,                  // From config.rollback_timeout_secs
    armed: Mutex<Option<ArmedState>>,
}

struct ArmedState {
    rollback_handle: RollbackHandle,
    cancel_tx: oneshot::Sender<()>,     // Pour confirm()
    armed_at: Instant,
}

impl AntilockoutGuard {
    pub async fn arm(&self, rollback: RollbackHandle) -> Result<(), GuardError>;
    pub async fn confirm(&self) -> Result<(), GuardError>;
    pub fn is_armed(&self) -> bool;
}
```

**Adapter** (`crates/infra/src/connectivity/tcp_probe.rs`) :

```rust
pub struct TcpProbe {
    endpoints: Vec<SocketAddr>,
    per_endpoint_timeout: Duration,    // 2s par défaut
}

#[async_trait]
impl ConnectivityProbe for TcpProbe {
    async fn probe(&self) -> Result<ProbeOutcome, ProbeError> {
        // tokio::join!(connect endpoints) avec timeout, succès si ≥1 OK
    }
}
```

**Câblage** dans `crates/infra/src/nftables/adapter.rs::apply_ruleset` :

1. Sauvegarder l'état (rollback handle).
2. Si la transaction touche **uniquement** des règles whitelist (DNS/DHCP/NTP/loopback) → bypass guard.
3. Sinon `guard.arm(handle)`.
4. `tokio::spawn` une tâche qui sonde toutes les 5s pendant 30s ; si toutes les sondes échouent → `firewall.rollback(handle)` + audit event critique.

**Erreur domain** : nouveau variant `DomainError::AntilockoutTriggered { rolled_back_rule_count: usize }`.

### A.2 — Authentification peer gRPC

**Interceptor** (`crates/daemon/src/grpc/interceptors/peer_auth.rs`) :

```rust
pub struct PeerAuthInterceptor {
    allowed_uids: HashSet<u32>,        // {0}
    allowed_gids: HashSet<u32>,        // {syswall_gid}
    audit_tx: mpsc::Sender<AuditEvent>,
}

impl Interceptor for PeerAuthInterceptor {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        let creds = req.extensions()
            .get::<UnixPeerCredentials>()
            .ok_or_else(|| Status::internal("peer credentials unavailable"))?;
        if self.allowed_uids.contains(&creds.uid)
            || self.allowed_gids.iter().any(|g| creds.gids.contains(g)) {
            Ok(req)
        } else {
            self.audit_tx.try_send(AuditEvent::auth_denied(creds, &req)).ok();
            Err(Status::permission_denied(
                "syswall: caller must be root or in group 'syswall'"
            ))
        }
    }
}
```

**Capture des credentials** : middleware `tower::Service` qui appelle `getsockopt::<PeerCredentials>` sur chaque `UnixStream` accepté et insère le résultat dans les extensions de la requête.

**Boot strict** : `Group::from_name("syswall")?` ; échec → `panic!` avec message explicite. `chown` du socket `/run/syswall.sock` échoue → `panic!`.

### A.3 — Durcissement `syswall.service`

Fichier `system/syswall.service` complet :

```ini
[Unit]
Description=SysWall application-level firewall daemon
After=network-pre.target
Wants=network-pre.target
Documentation=https://github.com/yrbane/SysWall

[Service]
Type=notify
ExecStart=/usr/bin/syswall-daemon
Restart=on-failure
RestartSec=5

# Identité
User=syswall
Group=syswall

# Capabilities (eBPF + nftables + procfs + chown socket)
AmbientCapabilities=CAP_NET_ADMIN CAP_NET_RAW CAP_BPF CAP_PERFMON CAP_SYS_PTRACE CAP_DAC_READ_SEARCH CAP_CHOWN
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW CAP_BPF CAP_PERFMON CAP_SYS_PTRACE CAP_DAC_READ_SEARCH CAP_CHOWN

# Sandbox
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictNamespaces=true
LockPersonality=true
RestrictRealtime=true
RestrictAddressFamilies=AF_UNIX AF_NETLINK AF_INET AF_INET6
SystemCallFilter=@system-service @network-io @file-system ~@privileged ~@resources ~@obsolete
SystemCallArchitectures=native

# Pas de MDWX : eBPF JIT requis
MemoryDenyWriteExecute=false

# Répertoires gérés par systemd (mode 0750, owned syswall:syswall)
ConfigurationDirectory=syswall
LogsDirectory=syswall
StateDirectory=syswall
RuntimeDirectory=syswall

[Install]
WantedBy=multi-user.target
```

**Création utilisateur/groupe** dans les scripts paquet :

```sh
getent group syswall >/dev/null || groupadd --system syswall
getent passwd syswall >/dev/null || useradd --system --gid syswall \
    --home-dir /var/lib/syswall --shell /usr/sbin/nologin syswall
chown -R syswall:syswall /var/lib/syswall /var/log/syswall
```

À propager dans `system/{aur,deb,rpm}` (Arch déjà OK).

### A.4 — CSP Tauri

`crates/ui/src-tauri/tauri.conf.json` :

```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self' ipc: tauri:; img-src 'self' data: asset:; font-src 'self' data:"
```

`'unsafe-inline'` styles requis par Svelte 5 (style scoping). Pas d'`unsafe-eval`. À tester via DevTools de la fenêtre Tauri.

### A.5 — Limites gRPC

`crates/daemon/src/grpc/server.rs` :

```rust
Server::builder()
    .max_decoding_message_size(1 << 20)        // 1 MiB
    .max_encoding_message_size(4 << 20)        // 4 MiB (events streaming)
    .timeout(Duration::from_secs(30))
    .concurrency_limit_per_connection(64)
    .layer(InterceptorLayer::new(peer_auth_interceptor))
    ...
```

## Flux de données

### Anti-lockout

```
apply_ruleset(rules)
    ├── snapshot()      → handle
    ├── nft -f new      (atomique)
    ├── if !whitelist_only(rules):
    │       guard.arm(handle)
    │       spawn:
    │           for tick in 0..6:
    │               if tick > 0: sleep(5s)
    │               if probe.probe().await? == Reachable:
    │                   guard.confirm()
    │                   audit(Info, "anti-lockout: connectivity confirmed")
    │                   return
    │           firewall.rollback(handle)
    │           audit(Critical, "anti-lockout: connectivity lost, rolled back")
    │           emit_event(AntilockoutTriggered)
    └── return Ok
```

### Auth peer

```
UnixStream accept
    ├── getsockopt SO_PEERCRED → uid, gid, supplementary_gids
    ├── insert into request extensions
    ├── interceptor checks {uid in {0}} || {syswall_gid in gids}
    ├── if denied:
    │       audit(Warning, AuthenticationDenied)
    │       Status::permission_denied
    └── else: request proceeds
```

## Tests

### TDD strict — chaque chantier commence par les tests qui échouent

**A.1 (16+ cas)** :

`crates/domain/src/services/antilockout_guard_tests.rs` :
1. `arm` puis probe OK au premier tick (5s) → `confirm` auto, pas de rollback.
2. `arm` puis 5 probes KO → rollback déclenché à T=30s + audit critical.
3. `arm` puis probe KO 4 fois puis OK à 25s → confirm, pas de rollback.
4. `confirm` manuel à T=10s → annule timer, pas de rollback.
5. `arm` pendant un guard déjà armé → `Err(GuardError::AlreadyArmed)`.
6. Apply sur règles whitelist uniquement → bypass, pas d'arm.
7. `is_armed()` retourne true entre arm et confirm/timeout.
8. Audit event `Antilockout` créé avec contenu correct (rule_count, endpoints essayés).
9. `tokio::time::pause()` utilisé pour simuler 30s instantanément.

`crates/infra/src/connectivity/tcp_probe_tests.rs` :
10. Endpoint local joignable → `Reachable`.
11. Tous endpoints en timeout → `Unreachable`.
12. Un endpoint OK + un KO → `Reachable` (OR logique).
13. `ConnectionRefused` (port fermé) → `Reachable` (le réseau passe).
14. Endpoint mal formé → `ProbeError::Configuration`.
15. IPv4 OK + IPv6 KO → `Reachable`.
16. IPv6 OK + IPv4 KO → `Reachable`.

**A.2 (4+ cas)** :

`crates/daemon/src/grpc/interceptors/peer_auth_tests.rs` :
17. UID 0 → autorisé.
18. UID 1000 + GID `syswall` dans supplementary → autorisé.
19. UID 1000 + aucun GID syswall → refusé `PermissionDenied`.
20. Audit event créé sur refus avec uid/method.

Approche : `socketpair()` Unix dans le test, l'enfant `setuid/setgid` puis envoie une requête, le parent vérifie l'erreur.

**A.3** : script `system/tests/check-hardening.sh` (bash) qui parse la sortie de `systemctl show syswall.service` et vérifie 18 clés attendues. Lancé en CI dans un container `archlinux` avec systemd.

**A.4** : test manuel documenté + `crates/ui/CLAUDE.md` mis à jour : "Ouvrir DevTools de l'app, recharger, vérifier qu'aucune violation CSP n'apparaît dans la console."

**A.5** :

`crates/daemon/tests/grpc_limits_test.rs` :
21. Message > 1 MiB → `InvalidArgument`.
22. 65 streams concurrentes sur la même connexion → la 65e est mise en attente / rejetée.

**Couverture cible** : 270 → ~292 tests (270 actuels + 22 nouveaux).

## Plan de migration & compatibilité

- **Premier déploiement** : nouvelle install crée user `syswall` ; rien à migrer.
- **Upgrade depuis V0.1** : script `postupgrade` détecte `/var/lib/syswall` owned `root:root` → `chown -R syswall:syswall`. Service redémarré automatiquement.
- **Rollback** vers V0.1 : possible (l'utilisateur `syswall` reste, ne gêne pas un retour à `User=root`).
- **Désinstall** : utilisateur `syswall` conservé (convention Linux).
- **Compat IPC** : la CSP et les limites gRPC sont strictement plus restrictives. Aucun client externe (Tauri UI seul). Pas de breaking change protocol.

## Risques & mitigations

| Risque | Probabilité | Mitigation |
|---|---|---|
| `AmbientCapabilities` insuffisantes pour eBPF/nftables sur kernel < 5.10 | Moyenne | Test sur Arch courant ; documenter prérequis kernel ≥ 5.8 ; fallback `User=root` documenté en troubleshooting |
| `Group::from_name("syswall")` échoue au boot (install incomplète) | Faible | Panic explicite avec message actionnable |
| Probes Cloudflare bloquées par firewall corporate | Faible | Endpoints configurables ; documenter dans config |
| Whitelist bypass exploité pour s'auto-whitelister | Très faible | Whitelist hardcodée en domain (DNS/DHCP/NTP/loopback), non exposée à l'utilisateur |
| CSP `unsafe-inline` styles trop laxiste | Faible | Documenté ; à durcir post-V0.2 si Svelte 6 supprime le besoin |
| Tokio `time::pause()` ne fonctionne pas avec `tokio::spawn` cross-runtime | Faible | Utiliser `tokio::test(start_paused = true)` |

## Critères de succès

- [ ] `cargo test --workspace` : 0 échec, ≥ 290 tests.
- [ ] `cargo clippy -p syswall-domain -p syswall-app -p syswall-infra -p syswall-daemon -- -D warnings` : 0 warning.
- [ ] `system/tests/check-hardening.sh` passe en CI.
- [ ] Daemon démarre, applique une règle, annule via lockout artificiel (test manuel : `iptables -I INPUT -p tcp --dport 53 -j DROP` après apply, vérifier rollback à T+30s).
- [ ] UI Tauri : DevTools ne montre aucune violation CSP.
- [ ] Refus gRPC pour un peer non-syswall (test : `socat - UNIX-CONNECT:/run/syswall.sock` depuis user lambda → erreur).
- [ ] Documentation `README.md` (FR+EN) et `CHANGELOG.md` mises à jour.

## Plan d'exécution (13 commits, ordre strict)

1. `feat(domain): port ConnectivityProbe pour l'anti-lockout` (port + tests rouges).
2. `feat(app): AntilockoutGuard avec timer 30s et rollback automatique` (service + tests verts).
3. `feat(infra): TcpProbe pour la sonde de connectivite anti-lockout`.
4. `feat(infra): cablage du guard anti-lockout dans nftables apply`.
5. `feat(daemon): config anti-lockout (timeout, endpoints)`.
6. `feat(ui): notification UI sur rollback automatique anti-lockout`.
7. `feat(daemon): authentification peer SO_PEERCRED sur gRPC`.
8. `feat(daemon): journal des refus d'authentification gRPC`.
9. `feat(system): durcissement service systemd avec utilisateur syswall dedie`.
10. `test(system): verification automatique du durcissement systemd`.
11. `feat(ui): CSP stricte dans tauri.conf.json`.
12. `feat(daemon): limites de taille et concurrence gRPC`.
13. `docs: documentation des renforcements securite v0.2 (FR/EN)`.

Chaque commit doit compiler et passer les tests à lui seul (atomique).

## Hors-scope explicite

- Sous-projets B/C/D/E (traités après merge de A).
- Migration vers cargo-deny / cargo-audit en CI (souhaitable mais sous-projet B).
- Refactor des god-modules (sous-projet B).
- Lecture des champs config dormants autres que `rollback_timeout_secs` (sous-projet C).

---

*Spec rédigée le 2026-05-05. À approuver avant transition vers writing-plans.*
