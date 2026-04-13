<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.82+-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/Tauri-2-blue?logo=tauri" alt="Tauri">
  <img src="https://img.shields.io/badge/Svelte-5-ff3e00?logo=svelte" alt="Svelte">
  <img src="https://img.shields.io/badge/License-MIT-green" alt="MIT">
  <img src="https://img.shields.io/badge/Tests-250-brightgreen" alt="Tests">
  <a href="https://github.com/yrbane/SysWall/actions/workflows/ci.yml">
    <img src="https://github.com/yrbane/SysWall/actions/workflows/ci.yml/badge.svg" alt="CI">
  </a>
</p>

# SysWall

**Pare-feu applicatif Linux de bureau** | **Linux desktop application-level firewall**

SysWall intercepte, analyse et controle le trafic reseau au niveau applicatif. Il identifie quel processus communique avec quel serveur, apprend vos habitudes et applique des regles granulaires via nftables — le tout avec une interface premium.

SysWall intercepts, analyzes and controls network traffic at the application level. It identifies which process communicates with which server, learns your habits, and enforces granular rules via nftables — all through a premium interface.

---

## Pourquoi SysWall ? / Why SysWall?

Les pare-feux Linux classiques (iptables, ufw) filtrent par IP/port. **SysWall filtre par application** : "Firefox peut acceder a github.com, mais pas de connexion sortante pour ce script inconnu." C'est l'equivalent Linux de Little Snitch (macOS) ou GlassWire (Windows).

Traditional Linux firewalls (iptables, ufw) filter by IP/port. **SysWall filters by application**: "Firefox can reach github.com, but no outbound for that unknown script." It's the Linux equivalent of Little Snitch (macOS) or GlassWire (Windows).

---

## Fonctionnalites / Features

### Surveillance temps reel / Real-time monitoring
- Detection des connexions via **conntrack** (netfilter kernel)
- Identification des processus via **eBPF** (tracepoint `inet_sock_set_state`) avec fallback `/proc`
- Resolution DNS inverse avec **cache LRU** (4096 entrees, TTL 5min)
- Icones des applications resolues depuis les fichiers `.desktop`
- Streaming en temps reel vers l'UI via **gRPC** sur socket Unix

### Moteur de regles / Rule engine
- Gestion via **nftables** (pas d'iptables legacy)
- 7 criteres combinables : application, IP/CIDR, port/plage, protocole, direction, utilisateur, horaire
- Priorites, regles temporaires avec expiration, import/export
- Protection des regles systeme (DNS, DHCP, loopback, NTP)
- Rollback automatique en cas d'echec (timer anti-lockout 30s)

### Autoapprentissage / Auto-learning
- Detection des connexions sans regle correspondante
- Notification non-bloquante avec compte a rebours
- 6 actions : autoriser/bloquer une fois, toujours autoriser/bloquer, creer une regle, ignorer
- Deduplication (debounce) pour eviter le spam de notifications
- Persistance en base SQLite (survit aux redemarrages)

### Journal d'audit / Audit log
- Enregistrement de tous les evenements systeme
- Ecriture par lots (batch) pour la performance
- Rotation automatique configurable
- Filtres : severite, categorie, plage de dates, recherche texte
- Export JSON

### Interface graphique / GUI
- **Tauri 2 + SvelteKit 5** avec Svelte runes
- Theme sombre cyber/neon avec glassmorphism subtil
- 6 vues : Tableau de bord, Connexions, Regles, Apprentissage, Journal, Parametres
- Layout responsive : desktop, tablette (sidebar iconique), mobile (barre de navigation)
- Toasts de notification, squelettes de chargement, etats vides
- Transitions de page animees
- Interface entierement en francais

---

## Architecture

```
+------------------+        gRPC / Unix socket         +-------------------+
|   UI (Tauri 2)   | <-------------------------------> |  Daemon (root)    |
|  Svelte 5 + TS   |   SysWallControl (req/res)        |  Rust async       |
|  Non-privilegie   |   SysWallEvents (streaming)       |  Privileges restr.|
+------------------+                                    +-------------------+
                                                               |
                                     +----------+---------+----+----+---------+
                                     |          |         |         |         |
                                   eBPF    nftables   conntrack   /proc    SQLite
                                  (PID)    (regles)  (connexions)(process)(persist.)
```

### Crates Rust (7 crates)

| Crate | Role |
|---|---|
| `syswall-domain` | Entites, value objects, ports (traits), PolicyEngine, evenements |
| `syswall-app` | Services applicatifs, commandes, 9 fakes pour les tests |
| `syswall-infra` | Adapters : nftables, conntrack, procfs, SQLite, EventBus, DNS |
| `syswall-ebpf` | Capture PID kernel via eBPF (aya) + HybridProcessResolver |
| `syswall-proto` | Definitions gRPC (protobuf) + code genere tonic/prost |
| `syswall-daemon` | Bootstrap DI, superviseur, gRPC, systemd, configuration |
| `syswall-ui` | Application Tauri + SvelteKit + TypeScript |

Le programme BPF (`crates/ebpf-prog`) est compile separement avec `aya-ebpf` (target `bpfel-unknown-none`).

### Principes

- **Architecture hexagonale** (ports & adapters) — le domain ne connait pas l'infrastructure
- **TDD** — 250 tests unitaires et d'integration
- **SOLID, DRY, KISS**
- **Separation des privileges** — daemon root avec capabilities restreintes, UI userspace
- **Event-driven** — EventBus interne tokio broadcast
- **Fail-safe** — les regles nftables restent en place si le daemon s'arrete

---

## Installation

### Arch Linux (recommande / recommended)

```bash
# Depuis les sources locales / From local sources
cd system/arch
makepkg -si

# Ou depuis l'AUR (quand publie) / Or from AUR (when published)
# yay -S syswall
```

### Debian / Ubuntu

```bash
# Installer les dependances / Install dependencies
sudo apt install nftables conntrack

# Installer avec le script / Install with the script
sudo ./system/install.sh
```

### Depuis les sources / From source

```bash
# Prerequis / Prerequisites
# Rust >= 1.82, Node.js >= 18, nftables, conntrack-tools

# Daemon
cargo build --release -p syswall-daemon

# UI (Tauri)
cd crates/ui && npm ci && npm run tauri build

# eBPF (optionnel, necessite nightly + bpf-linker)
cd crates/ebpf-prog
rustup run nightly cargo build --release
```

### Apres installation / Post-install

```bash
# Demarrer le daemon / Start the daemon
sudo systemctl enable --now syswall

# Ajouter votre utilisateur au groupe syswall / Add user to syswall group
sudo usermod -aG syswall $USER

# Lancer l'UI / Launch the UI
syswall-ui
```

---

## Configuration

Fichier / File : `/etc/syswall/config.toml`

```toml
config_version = 1

[daemon]
socket_path = "/var/run/syswall/syswall.sock"
log_level = "info"                    # trace | debug | info | warn | error
log_dir = "/var/log/syswall"
watchdog_interval_secs = 15

[database]
path = "/var/lib/syswall/syswall.db"
journal_retention_days = 30
audit_batch_size = 100
audit_flush_interval_secs = 2

[firewall]
default_policy = "ask"                # ask | allow | block
rollback_timeout_secs = 30
nftables_table_name = "syswall"

[monitoring]
conntrack_buffer_size = 4096
process_cache_ttl_secs = 5
event_bus_capacity = 4096

[ebpf]
enabled = true                        # false pour desactiver eBPF / disable eBPF

[learning]
enabled = true
prompt_timeout_secs = 60
max_pending_decisions = 50
default_timeout_action = "block"

[ui]
locale = "fr"
theme = "dark"
refresh_interval_ms = 1000
```

---

## Tests & Benchmarks

```bash
# Tous les tests (250 tests)
cargo test --workspace

# Tests d'integration nftables/conntrack (necessite root)
cargo test --workspace --features integration

# Benchmarks PolicyEngine (Criterion)
cargo bench -p syswall-app

# Linting
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings

# Audit de securite
cargo audit
```

### Resultats benchmarks / Benchmark results

| Scenario | Temps / Time |
|---|---|
| PolicyEngine, 10 regles | 73 ns |
| PolicyEngine, 100 regles | 969 ns |
| PolicyEngine, 500 regles | 15.5 us |
| PolicyEngine, 1000 regles | 9 us |
| Best case (match immediat, 1000 regles) | 55 ns |

---

## Securite / Security

Le service systemd est durci avec des restrictions strictes :

The systemd service is hardened with strict restrictions:

| Directive | Description |
|---|---|
| `CapabilityBoundingSet` | `CAP_NET_ADMIN` `CAP_NET_RAW` `CAP_SYS_PTRACE` `CAP_DAC_READ_SEARCH` `CAP_BPF` `CAP_PERFMON` |
| `SystemCallFilter` | `@system-service` `@network-io` `bpf` `perf_event_open` |
| `RestrictAddressFamilies` | `AF_UNIX` `AF_INET` `AF_INET6` `AF_NETLINK` |
| `ProtectKernelModules` | `true` |
| `ProtectKernelLogs` | `true` |
| `LockPersonality` | `true` |
| `RestrictRealtime` | `true` |

Autres mesures / Other measures:
- **Socket Unix securise** : permissions `0660`, groupe `syswall`
- **Pas de concatenation shell** : commandes nft/conntrack via `std::process::Command`
- **Rollback nftables** : restauration automatique en cas d'echec
- **Whitelist systeme** : regles DNS/DHCP/loopback/NTP non supprimables

---

## CI/CD

Le projet utilise **GitHub Actions** avec 3 workflows :

| Workflow | Declencheur / Trigger | Actions |
|---|---|---|
| `ci.yml` | Push / PR sur `main` | fmt, clippy, tests, build UI, `cargo audit` |
| `release.yml` | Tag `v*` | Build daemon + Tauri, packages, GitHub Release |
| `aur.yml` | Publication d'une release | Mise a jour automatique du PKGBUILD AUR |

---

## Packaging

| Format | Chemin | Commande |
|---|---|---|
| **Arch (local)** | `system/arch/PKGBUILD` | `makepkg -si` |
| **AUR** | `system/aur/PKGBUILD` | `yay -S syswall` |
| **Deb** | `system/deb/` | Scripts postinst/postrm pour Tauri bundle |
| **RPM** | `system/rpm/syswall.spec` | Spec avec scriptlets |

---

## Structure du projet / Project structure

```
syswall/
├── Cargo.toml                    # Workspace (7 crates)
├── config/default.toml           # Configuration par defaut
├── proto/syswall.proto           # Definitions gRPC
├── system/
│   ├── syswall.service           # Service systemd durci
│   ├── syswall.desktop           # Raccourci bureau
│   ├── install.sh                # Script d'installation
│   ├── arch/                     # PKGBUILD Arch Linux
│   ├── aur/                      # PKGBUILD AUR
│   ├── deb/                      # Scripts Debian
│   └── rpm/                      # Spec RPM
├── .github/workflows/            # CI/CD GitHub Actions
├── crates/
│   ├── domain/src/               # Coeur metier pur
│   │   ├── entities/             # Connection, Rule, Decision, AuditEvent
│   │   ├── value_objects/        # Port, Protocol, Direction, RulePriority
│   │   ├── services/             # PolicyEngine (matching)
│   │   ├── ports/                # 9 traits async
│   │   ├── events/               # DomainEvent, DefaultPolicy, Pagination
│   │   └── errors/               # DomainError
│   ├── app/src/                  # Couche application
│   │   ├── services/             # RuleService, LearningService, ConnectionService, AuditService
│   │   ├── commands/             # CQRS commands
│   │   ├── fakes/                # 9 fake adapters (tests)
│   │   └── benches/              # Benchmarks Criterion
│   ├── infra/src/                # Adapters infrastructure
│   │   ├── nftables/             # NftablesFirewallAdapter
│   │   ├── conntrack/            # ConntrackMonitorAdapter
│   │   ├── process/              # ProcfsProcessResolver, IconResolver
│   │   ├── dns/                  # Reverse DNS avec cache LRU
│   │   ├── persistence/          # SQLite (rules, decisions, audit, pending)
│   │   └── event_bus/            # TokioBroadcastEventBus
│   ├── ebpf/src/                 # Capture PID via eBPF
│   │   ├── lib.rs                # EbpfProcessResolver, HybridProcessResolver
│   │   └── events.rs             # SocketEvent (partage avec le programme BPF)
│   ├── ebpf-prog/src/            # Programme BPF kernel (aya-ebpf, no_std)
│   ├── proto/                    # Code genere tonic/prost
│   ├── daemon/src/               # Point d'entree daemon
│   │   ├── grpc/                 # Serveur gRPC (control, events)
│   │   ├── bootstrap.rs          # Injection de dependances
│   │   ├── supervisor.rs         # Orchestrateur de taches async
│   │   └── config.rs             # Configuration TOML typee
│   └── ui/                       # Frontend Tauri + SvelteKit
│       ├── src-tauri/            # Bridge Rust (client gRPC)
│       └── src/
│           ├── lib/components/   # Design system (13 composants)
│           ├── lib/stores/       # Svelte stores reactifs (7 stores)
│           ├── lib/api/          # Client API type
│           ├── lib/i18n/         # Localisation FR
│           └── routes/           # 7 vues
└── docs/superpowers/
    ├── specs/                    # Specifications de design
    └── plans/                    # Plans d'implementation
```

---

## Statistiques / Stats

| Metrique / Metric | Valeur / Value |
|---|---|
| Tests | **250** (unitaires + integration SQLite) |
| Crates Rust | 7 (+1 programme BPF) |
| Fichiers Rust | ~85 |
| Lignes Rust | ~13 000 |
| Fichiers Svelte/TS | ~35 |
| Lignes Svelte/TS | ~5 000 |
| Composants UI | 13 |
| Commits | ~80 |
| Workflows CI/CD | 3 |
| Formats de packaging | 4 (Arch, AUR, deb, rpm) |

---

## Feuille de route / Roadmap

- [x] Architecture hexagonale + PolicyEngine
- [x] Surveillance temps reel (conntrack + /proc)
- [x] Moteur de regles nftables
- [x] Autoapprentissage non-bloquant
- [x] Interface premium Tauri + SvelteKit
- [x] Integration systemd (sd_notify, watchdog)
- [x] Capture PID via eBPF (aya)
- [x] Packaging Arch / AUR / deb / rpm
- [x] CI/CD GitHub Actions + deploiement AUR
- [x] Durcissement systemd
- [x] Benchmarks performance (Criterion)
- [ ] Filtrage IPv6 complet
- [ ] Mode paranoiaque (bloquer tout sauf whitelist)
- [ ] Profils reseau (maison, bureau, public)
- [ ] Export/import de regles entre machines

---

## Licence / License

[MIT](LICENSE)
