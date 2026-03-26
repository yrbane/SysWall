# SysWall

**Firewall Linux desktop moderne** | **Modern Linux desktop firewall**

SysWall est un pare-feu applicatif pour Linux avec une interface graphique premium, un mode d'autoapprentissage intelligent et un controle granulaire du trafic reseau.

SysWall is an application-level firewall for Linux with a premium GUI, intelligent auto-learning mode, and granular network traffic control.

---

## Fonctionnalites / Features

### Surveillance temps reel / Real-time monitoring
- Suivi des connexions reseau via **conntrack** (netfilter)
- Resolution des processus via **/proc** (PID, nom, chemin, ligne de commande)
- Flux d'evenements en streaming vers l'UI via **gRPC**

### Moteur de regles / Rule engine
- Gestion des regles firewall via **nftables**
- Criteres combinables : application, IP/CIDR, port/plage, protocole, direction, utilisateur, horaire
- Priorites, regles temporaires, import/export
- Protection des regles systeme (DNS, DHCP, loopback, NTP)
- Rollback automatique en cas d'echec (securite anti-lockout)

### Autoapprentissage / Auto-learning
- Detection des connexions inconnues (aucune regle ne correspond)
- Notification non-bloquante avec compte a rebours
- 6 actions : autoriser/bloquer une fois, toujours autoriser/bloquer, creer une regle, ignorer
- Deduplication (debounce) pour eviter le spam
- Expiration automatique des decisions en attente
- Persistance des decisions en base (survit aux redemarrages)

### Journal d'audit / Audit log
- Enregistrement de tous les evenements systeme
- Ecriture par lots (batch) pour la performance
- Rotation automatique (retention configurable)
- Filtres : severite, categorie, plage de dates, recherche texte
- Statistiques pour le tableau de bord
- Export JSON

### Interface premium / Premium UI
- Theme sombre cyber/neon avec glassmorphism subtil
- 6 vues : Tableau de bord, Connexions, Regles, Apprentissage, Journal, Parametres
- Composants reactifs en temps reel (Svelte stores + streaming gRPC)
- Interface entierement en francais
- Design system avec 11 composants reutilisables

---

## Architecture

```
+------------------+          gRPC / Unix socket          +------------------+
|   UI (Tauri)     | <----------------------------------> |  Daemon (root)   |
|  Svelte + TS     |    SysWallControl (req/res)          |  Rust            |
|  Non-privilegie  |    SysWallEvents (streaming)         |  Privilegies     |
+------------------+                                      +------------------+
                                                                  |
                                          +-----------+-----------+-----------+
                                          |           |           |           |
                                      nftables    conntrack    /proc     SQLite
                                      (regles)   (connexions) (process) (persistance)
```

### Crates Rust

| Crate | Role |
|---|---|
| `syswall-domain` | Entites, value objects, ports (traits), PolicyEngine, evenements, erreurs |
| `syswall-app` | Services applicatifs (RuleService, LearningService, ConnectionService, AuditService), fakes pour les tests |
| `syswall-infra` | Adapters : nftables, conntrack, procfs, SQLite (4 repos), EventBus tokio broadcast |
| `syswall-proto` | Definitions gRPC (protobuf) + code genere tonic |
| `syswall-daemon` | Point d'entree, bootstrap DI, superviseur, gestion signaux, serveur gRPC, configuration |
| `syswall-ui` | Application Tauri + SvelteKit + TypeScript |

### Principes

- **Architecture hexagonale** (ports & adapters) — le domain ne connait pas l'infrastructure
- **Tous les ports sont async** (`#[async_trait] + Send + Sync`)
- **Separation des privileges** — daemon root, UI non-privilegiee
- **Event-driven** — EventBus interne tokio broadcast
- **Autoapprentissage non-bloquant** — PendingDecision persistees, pas de prompt synchrone
- **Fail-safe** — les regles nftables restent en place si le daemon s'arrete

---

## Prerequis / Prerequisites

- **Linux** (nftables, conntrack, /proc)
- **Rust** >= 1.82 (edition 2024)
- **Node.js** >= 18
- **nft** (nftables) installe
- **conntrack-tools** (pour le monitoring temps reel)

### Installation des dependances (Arch Linux)

```bash
sudo pacman -S nftables conntrack-tools
```

### Installation des dependances (Debian/Ubuntu)

```bash
sudo apt install nftables conntrack
```

---

## Build

### Daemon

```bash
cargo build --release -p syswall-daemon
```

Le binaire sera dans `target/release/syswall-daemon`.

### UI

```bash
cd crates/ui
npm install
npm run tauri build
```

Le binaire Tauri sera dans `crates/ui/src-tauri/target/release/`.

---

## Utilisation / Usage

### 1. Demarrer le daemon

Le daemon necessite les privileges root pour acceder a nftables et conntrack :

```bash
sudo ./target/release/syswall-daemon
```

Ou avec un fichier de configuration personnalise :

```bash
sudo SYSWALL_CONFIG=/etc/syswall/config.toml ./target/release/syswall-daemon
```

### 2. Demarrer l'UI

L'UI se connecte au daemon via le socket Unix :

```bash
cd crates/ui && npm run tauri dev
```

### 3. Avec systemd (production)

Copier le binaire et creer le service :

```bash
sudo cp target/release/syswall-daemon /usr/bin/
sudo mkdir -p /etc/syswall /var/lib/syswall /var/log/syswall /var/run/syswall
sudo cp config/default.toml /etc/syswall/config.toml
```

Creer `/etc/systemd/system/syswall.service` :

```ini
[Unit]
Description=SysWall Firewall Daemon
After=network.target

[Service]
Type=notify
ExecStart=/usr/bin/syswall-daemon
Environment=SYSWALL_CONFIG=/etc/syswall/config.toml
User=root
Restart=on-failure
RestartSec=5s
WatchdogSec=30s
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/syswall /var/log/syswall /var/run/syswall

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable --now syswall
```

---

## Configuration

Fichier : `config/default.toml`

```toml
config_version = 1

[daemon]
socket_path = "/var/run/syswall/syswall.sock"
log_level = "info"
log_dir = "/var/log/syswall"

[database]
path = "/var/lib/syswall/syswall.db"
journal_retention_days = 30

[firewall]
default_policy = "ask"          # ask | allow | block
rollback_timeout_secs = 30
nftables_table_name = "syswall"

[learning]
enabled = true
prompt_timeout_secs = 60
max_pending_decisions = 50
default_timeout_action = "block"

[ui]
locale = "fr"
theme = "dark"
```

---

## Tests

```bash
# Tests unitaires + integration SQLite (241 tests)
cargo test --workspace

# Tests d'integration nftables/conntrack (necessite root + capabilities)
cargo test --workspace --features integration

# Tests frontend
cd crates/ui && npm test

# Linting
cargo clippy --workspace
```

---

## Structure du projet / Project structure

```
syswall/
├── Cargo.toml                  # Workspace root
├── config/default.toml         # Configuration par defaut
├── proto/syswall.proto         # Definitions gRPC
├── crates/
│   ├── domain/src/
│   │   ├── entities/           # Connection, Rule, Decision, AuditEvent...
│   │   ├── value_objects/      # Port, Protocol, Direction, RulePriority...
│   │   ├── services/           # PolicyEngine (matching pur)
│   │   ├── ports/              # Traits async (repositories, system, messaging)
│   │   ├── events/             # DomainEvent, FirewallStatus, Pagination...
│   │   └── errors/             # DomainError
│   ├── app/src/
│   │   ├── services/           # RuleService, LearningService, ConnectionService, AuditService
│   │   ├── commands/           # CreateRuleCommand, RespondToDecisionCommand...
│   │   └── fakes/              # 9 fake adapters pour les tests
│   ├── infra/src/
│   │   ├── nftables/           # NftablesFirewallAdapter (command builder, translator, parser)
│   │   ├── conntrack/          # ConntrackMonitorAdapter (event parser, transformer)
│   │   ├── process/            # ProcfsProcessResolver (cache LRU, /proc parsers)
│   │   ├── persistence/        # SQLite repos (rules, decisions, audit) + Database
│   │   └── event_bus/          # TokioBroadcastEventBus
│   ├── proto/                  # Code genere tonic/prost
│   ├── daemon/src/
│   │   ├── grpc/               # Serveur gRPC (control, events, converters)
│   │   ├── bootstrap.rs        # Cablage DI
│   │   ├── supervisor.rs       # Gestion des taches async
│   │   ├── config.rs           # Configuration TOML typee
│   │   └── signals.rs          # Gestion SIGTERM/SIGINT
│   └── ui/
│       ├── src-tauri/src/      # Client gRPC Tauri (Rust)
│       └── src/
│           ├── lib/components/ # Design system (11 composants)
│           ├── lib/stores/     # Svelte stores reactifs (6 stores)
│           ├── lib/api/        # Client API type
│           ├── lib/i18n/       # Localisation FR
│           └── routes/         # 6 vues (dashboard, connexions, regles, apprentissage, journal, parametres)
└── docs/superpowers/
    ├── specs/                  # Specifications de design
    └── plans/                  # Plans d'implementation
```

---

## Securite / Security

- **Separation des privileges** : le daemon tourne en root avec capabilities restreintes (`CAP_NET_ADMIN`, `CAP_NET_RAW`), l'UI tourne en userspace
- **Socket Unix securise** : permissions `0660`, groupe `syswall`, verification `SO_PEERCRED`
- **Pas de concatenation shell** : toutes les commandes nft/conntrack via `std::process::Command` avec arguments types
- **Rollback nftables** : sauvegarde de l'etat avant modification, restauration automatique en cas d'echec
- **Whitelist systeme** : regles DNS/DHCP/loopback/NTP creees au premier demarrage, non supprimables
- **Regles persistantes** : les regles nftables restent en place si le daemon s'arrete
- **Validation fail-fast** : configuration typee, rejet au demarrage si invalide
- **Protection anti-lockout** : timer de securite 30s sur les modifications de regles

---

## Statistiques / Stats

| Metrique | Valeur |
|---|---|
| Tests | 241 (unitaires + integration SQLite) |
| Fichiers Rust | 78 |
| Lignes Rust | ~11 400 |
| Fichiers Svelte/TS | 31 |
| Lignes Svelte/TS | ~4 600 |
| Crates Rust | 6 |
| Commits | 46 |

---

## Licence / License

MIT
