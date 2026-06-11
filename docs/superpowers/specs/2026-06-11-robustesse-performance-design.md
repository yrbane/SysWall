# Sous-projet ① — Robustesse & performance / Sub-project ① — Robustness & performance

**Date** : 2026-06-11
**Statut / Status** : Validé / Approved
**Contexte / Context** : Premier sous-projet du programme « Faire de SysWall le meilleur firewall et le plus complet pour Linux » (① Robustesse & perf → ② Capacités de filtrage → ③ Roadmap V0.4 i18n/UX → ④ Analyse concurrentielle).

---

## 1. Objectif / Goal

**FR** — Rendre le cœur de SysWall inattaquable et rapide avant d'ajouter des fonctionnalités : parité IPv6 complète, fuzzing et property testing du moteur, migration du blocage actif de NFQUEUE vers eBPF (cgroup hooks), et validation de performance sous charge.

**EN** — Make SysWall's core bulletproof and fast before adding features: full IPv6 parity, fuzzing and property testing of the engine, migration of active blocking from NFQUEUE to eBPF (cgroup hooks), and performance validation under load.

## 2. Décisions actées / Settled decisions

| Décision / Decision | Choix / Choice |
|---|---|
| Rôle eBPF vs NFQUEUE | **eBPF remplace NFQUEUE** (pas d'hybride) / eBPF **replaces** NFQUEUE (no hybrid) |
| Sémantique « ask » | **Deny + retry transparent** : EPERM immédiat, verdict poussé dans la map BPF dès la réponse utilisateur / immediate EPERM, verdict pushed to the BPF map once the user answers |
| Ordre des chantiers | **A. Filet d'abord** : fuzzing → IPv6 → migration eBPF → perf / safety net first: fuzzing → IPv6 → eBPF migration → perf |
| Prérequis système | Kernel ≥ 5.8 + cgroup v2 ; sinon erreur de démarrage explicite + mode dégradé (monitoring seul) / otherwise explicit startup error + degraded mode (monitoring only) |

## 3. Architecture cible / Target architecture

### 3.1 Hooks eBPF / eBPF hooks

**FR** — Programmes aya-ebpf attachés au cgroup racine :
- `cgroup/connect4` et `cgroup/connect6` — TCP et UDP connecté ;
- `cgroup/sendmsg4` et `cgroup/sendmsg6` — UDP non connecté.

Le verdict est rendu **avant l'émission du paquet**, dans le contexte du processus appelant : `bpf_get_current_pid_tgid()` fournit le PID/TGID de façon fiable à 100 %. Le `HybridProcessResolver` se simplifie (plus de corrélation socket→PID après coup pour le chemin de blocage).

**EN** — aya-ebpf programs attached to the root cgroup: `cgroup/connect4|6` (TCP and connected UDP) and `cgroup/sendmsg4|6` (unconnected UDP). The verdict is issued **before the packet is emitted**, in the calling process context: `bpf_get_current_pid_tgid()` yields a 100 % reliable PID/TGID. `HybridProcessResolver` gets simpler.

### 3.2 Maps et flux de décision / Maps and decision flow

**FR** —
- **Map de verdicts** (dual-stack dès la conception) : clé `(tgid, addr dst, port dst, proto)` → verdict `Allow | Deny`. Alimentée exclusivement par le daemon.
- **Flux inconnu** : le hook émet un événement vers le daemon via **ring buffer** et retourne **EPERM** immédiatement (deny + retry). Le daemon résout le chemin de l'exécutable, évalue le `PolicyEngine`, pousse le verdict dans la map et déclenche la popup en mode « ask ». Dès que l'utilisateur répond « autoriser », la tentative suivante de l'application réussit.
- **Invalidation** : changement de règle, expiration de règle temporaire ou fin de processus → entrées retirées de la map par le daemon.

**EN** — A dual-stack **verdict map** keyed by `(tgid, dst addr, dst port, proto)`, written only by the daemon. Unknown flow: the hook emits a **ring buffer** event and returns **EPERM** immediately (deny + retry). The daemon resolves the executable path, evaluates the `PolicyEngine`, pushes the verdict into the map and shows the popup in “ask” mode. Rule changes, temporary-rule expiry or process exit invalidate map entries.

### 3.3 Rôles respectifs / Layer responsibilities

**FR** — **nftables reste la couche de base** : default policy, règles système protégées (DNS, DHCP, loopback, NTP), fail-safe si le daemon s'arrête. eBPF remplace uniquement le rôle de NFQUEUE : le verdict interactif par application. NFQUEUE est déprécié derrière un feature flag (`[nfqueue] enabled = false` par défaut) puis **supprimé en fin de sous-projet** (code, config, smoke test, job CI).

**EN** — **nftables remains the base layer** (default policy, protected system rules, fail-safe if the daemon dies). eBPF only replaces NFQUEUE's role: the interactive per-application verdict. NFQUEUE is deprecated behind a feature flag (`enabled = false` by default) then **removed at the end of the sub-project** (code, config, smoke test, CI job).

## 4. Phases / Phases

### Phase 1 — Fuzzing + property tests

**FR** —
- Cibles cargo-fuzz sur le code qui survit à la migration : parsing config TOML, converters gRPC/protobuf (`crates/daemon/src/grpc/converters/`). Le parser NFQUEUE, condamné, n'est pas fuzzé.
- proptest sur les invariants du `PolicyEngine` : ordre des priorités, expiration des règles temporaires, cohérence du matching (7 critères), absence de panic sur entrées arbitraires.
- Job CI « fuzz-smoke » : 60 s par cible à chaque PR ; corpus commité.

**EN** — cargo-fuzz targets on surviving code only (TOML config parsing, gRPC/protobuf converters); proptest on `PolicyEngine` invariants (priority ordering, temporary-rule expiry, 7-criteria matching consistency, no panic on arbitrary input). CI “fuzz-smoke” job: 60 s per target per PR; corpus committed.

### Phase 2 — IPv6 complet / Full IPv6

**FR** —
- Audit de parité v4/v6 sur toute la chaîne : translator nftables (table `inet`), conntrack v6, `/proc/net/tcp6|udp6`, résolution DNS, affichage UI.
- Chaque écart identifié → test de parité (même scénario en v4 et v6) puis correction.
- Les nouvelles structures (maps BPF de la phase 3) naissent dual-stack.

**EN** — v4/v6 parity audit across the whole chain (nftables translator with `inet` table, conntrack v6, `/proc/net/tcp6|udp6`, DNS resolution, UI display). Every gap gets a parity test (same scenario in v4 and v6) then a fix. New structures (phase 3 BPF maps) are born dual-stack.

### Phase 3 — Migration eBPF / eBPF migration

**FR** —
- Implémentation des 4 programmes cgroup + maps + ring buffer dans `crates/ebpf-prog`, chargement/attache via aya dans `crates/ebpf`.
- Nouveau port domain (ex. `FlowGate`) remplaçant `PacketInterceptor` ; adapter eBPF dans `crates/ebpf` ; fake pour les tests app.
- Intégration `LearningService` : deny + retry, dédup des popups par `dedup_key` (mécanisme existant conservé).
- Détection des prérequis au boot (kernel ≥ 5.8, cgroup v2 monté) ; échec → erreur de démarrage explicite + mode dégradé monitoring seul, audité.
- Dépréciation puis suppression de NFQUEUE.

**EN** — Four cgroup programs + maps + ring buffer in `crates/ebpf-prog`, loaded/attached via aya in `crates/ebpf`. New domain port (e.g. `FlowGate`) replacing `PacketInterceptor`, eBPF adapter, fake for app tests. `LearningService` integration (deny + retry, existing `dedup_key` dedup kept). Boot-time prerequisite detection (kernel ≥ 5.8, cgroup v2); failure → explicit startup error + monitoring-only degraded mode, audited. NFQUEUE deprecated then removed.

### Phase 4 — Performance & charge / Performance & load

**FR** —
- Baseline **avant** migration : benchs du chemin NFQUEUE existant, documentés.
- Après migration : benchs Criterion du hot path (lookup map, évaluation policy, traitement ring buffer), test de charge ~10 000 connexions/s, mesure de latence des verdicts (cache hit kernel-side attendu < 1 µs).
- Backpressure du ring buffer : politique configurable en cas de saturation (deny/allow + audit), équivalent de l'actuel `overflow_policy`.

**EN** — Pre-migration baseline benches on the NFQUEUE path, documented. Post-migration: Criterion benches on the hot path (map lookup, policy evaluation, ring buffer handling), load test at ~10,000 connections/s, verdict latency measurement (kernel-side cache hit expected < 1 µs). Ring buffer backpressure: configurable saturation policy (deny/allow + audit), equivalent of today's `overflow_policy`.

## 5. Gestion d'erreurs / Error handling

**FR** —
- Anti-lockout et rollback nftables inchangés.
- Détachement eBPF inattendu → audit `Severity::Error` + réattachement par le superviseur (stratégie de retry existante).
- Ring buffer plein → politique configurable + audit (`Severity::Warning`).
- Verdict map pleine → éviction LRU côté daemon + audit.

**EN** — Anti-lockout and nftables rollback unchanged. Unexpected eBPF detach → `Severity::Error` audit + supervisor re-attach (existing retry strategy). Full ring buffer → configurable policy + `Warning` audit. Full verdict map → daemon-side LRU eviction + audit.

## 6. Tests / Testing

**FR** — TDD strict. Fakes du crate `app` réutilisés/étendus (fake `FlowGate`). Smoke test eBPF gated par `SYSWALL_TEST_EBPF` (même modèle que l'actuel smoke NFQUEUE), job CI dédié avec privilèges (`continue-on-error` si le runner ne permet pas l'attache cgroup). Tests de parité v4/v6 systématiques. Benchs Criterion en CI informatif (pas de gate).

**EN** — Strict TDD. `app` crate fakes reused/extended (fake `FlowGate`). eBPF smoke test gated by `SYSWALL_TEST_EBPF` (same model as the current NFQUEUE smoke), dedicated privileged CI job (`continue-on-error` when the runner cannot attach cgroup programs). Systematic v4/v6 parity tests. Criterion benches in CI as informative (no gate).

## 7. Hors périmètre / Out of scope

**FR** — Filtrage par domaine/DNS, blocklists, GeoIP, profils réseau (→ sous-projet ②) ; i18n et UX (→ ③) ; analyse concurrentielle (→ ④) ; filtrage entrant interactif par application (inchangé : nftables).

**EN** — Domain/DNS filtering, blocklists, GeoIP, network profiles (→ sub-project ②); i18n and UX (→ ③); competitive analysis (→ ④); interactive inbound per-app filtering (unchanged: nftables).
