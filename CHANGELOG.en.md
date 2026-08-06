# Changelog (English)

All notable changes documented here. Canonical/reference version: `CHANGELOG.md` (French).
Toutes les modifications notables sont documentees ici. Version canonique/de reference : `CHANGELOG.md` (francais).

## [0.3.9] - 2026-08-06 · "The prompt finally says what it is"

### Added

- **Destination identity in the new-connection prompt**: the window now shows the destination hostname (`github.com`) instead of a bare IP, with an "Application → host:port" summary line. The name comes first from **DNS snooping** (the domain the application actually requested, captured by observing DNS responses through a dedicated `input` nftables chain in fail-open `queue … bypass`), falling back to reverse-DNS then to the raw IP. Wiring: the resolver consults the snoop cache before the reverse lookup; an NFQUEUE consumer feeds that cache; a background task evicts expired entries. No new capability (reuses `CAP_NET_ADMIN`).

## [0.3.8] - 2026-08-05 · "Reinstall without ETXTBSY"

### Fixed

- **`install.sh` stops the daemon before replacing its binary**: on a reinstall, `sudo cp target/release/syswall-daemon /usr/bin/` failed with `Text file busy` (ETXTBSY) because the daemon was still running — the kernel refuses to overwrite the image of a running executable. The final `systemctl restart` was never reached. Step `[3/9]` now stops the service (`systemctl is-active --quiet syswall`) before copying; it is restarted at the end of the script as before. No effect on a first-time install (nothing to stop).

## [0.3.7] - 2026-08-04 · "Catching up three merged fixes"

### Fixed

- **quick-xml/plist vulnerabilities fixed (RUSTSEC-2026-0194/0195)**: a Tauri bump had made the historical workaround for these two high-severity advisories (quadratic DoS / unbounded allocation) obsolete. `cargo update -p plist` now resolves plist 1.10.0 (quick-xml >= 0.41 dependency), which fixes them. On the UI side, `npm audit fix` fixed postcss and `cookie` is pinned to `^0.7.2` via an npm `overrides` (SvelteKit still depends on `cookie ^0.6.0`). The CI `audit` job's `--ignore RUSTSEC-2026-0194/0195` flags are now harmless no-ops.
- **Expired temporary rules re-applied on startup**: `RuleRepository::list_enabled_ordered()` only filtered on `enabled = 1` in SQL, without excluding already-expired `RuleScope::Temporary { expires_at }` rules. This list feeds the nftables resync on daemon startup: a temporary rule ("block Firefox for 1h") kept being re-applied on every restart, even long after expiring. `RuleService::create_rule` now also rejects creating a temporary rule whose expiration is already in the past.

### Changed

- **Removed `firewall.rollback_timeout_secs`**: this config field was never read (flagged by the 2026-05-04 audit); the antilockout guard actually uses `AntilockoutConfig::timeout_secs` (`[antilockout]` section). Backward-compatible: serde silently ignores unknown TOML keys, so an existing config keeping the line still loads normally.

### Documentation

- **README test counter fixed (356 → 358)**: new drift observed (third occurrence after 250 → 356 in 0.3.5/0.3.6) across the 4 mentions (badge, TDD principle, `cargo test` block, Statistics table).

## [0.3.6] - 2026-08-04 · "Test badge and counters up to date"

### Documentation

- **README test counter fixed**: the badge was fixed in 0.3.5 (250 → 356, drifted over several versions), but three other textual mentions ("TDD — 250 tests", "All tests (250 tests)", Statistics table) were missed in that same pass. Aligned with the real count (`cargo test --workspace --exclude ui`): 356.

## [0.3.5] - 2026-08-04 · "English changelog catch-up"

### Documentation

- **`CHANGELOG.en.md` created**: this file was missing; it now backfills the full history (0.2.0 → 0.3.4) so the English changelog stays in sync with the French one going forward, entry for entry, same commit.

## [0.3.4] - 2026-07-16 · "IPv6 hardening"

### Fixed

- **Family-aware NFQUEUE truncation pre-check**: the guard at the top of `parse_packet` always required 20 bytes (minimal IPv4 header) before etherparse decoding, whereas the IPv6 header is 40. The guard now reads the version nibble of the first byte and requires ≥ 20 bytes for IPv4, ≥ 40 for IPv6, else `Malformed`. Purely defensive (etherparse re-validates), but consistent.
- **conntrack `-L` parsing fixed (IPv6 included)**: `parse_conntrack_line` only accepted the `conntrack -E` format (`[epoch]` timestamp prefix + `[NEW]`/… event type). `conntrack -L -o extended` lines — no timestamp nor type, but prefixed with an L3 label (`ipv6 10`) — were all rejected, so `GetActiveConnections` (active-connection snapshot, added in 0.3.3) never returned anything. The parser now accepts both formats: optional timestamp and event type (default `New`), L4 protocol located by name to skip the L3 label.

### Tests

- **IPv6 regression tests** added on previously uncovered v6 paths: truncated IPv6 packet (20-39 bytes) → clean `Malformed` without panic; IPv6 `conntrack -L -o extended` line (TCP `[ASSURED]` + UDP without brackets) → correct v6 addresses/protocol/ports; TCP listener on `[::1]:0` reachable by the antilockout probe; `/proc/net/udp6` line and `parse_hex_ipv6` (::1, port) → correct tuple.

### Notes

- **Observed IPv6 parity**: the `inet` nftables table, generic `IpAddr` parsers, the eBPF filter (`family == 10`), `/proc/net/*6` reading and the v6 antilockout probe already cover IPv6. ICMPv6/NDP is **intentionally not** intercepted at the NFQUEUE packet level (strict parity with ICMPv4, out of scope for a per-connection decision); no `Protocol::Icmpv6` variant is introduced.

## [0.3.3] - 2026-07-16 · "Active connections snapshot"

### Added

- **Seeding of the Connections tab on open**: the connection list was previously driven exclusively by the gRPC event stream, so connections **already active** when the UI started stayed invisible until a new event touched them. A new `GetActiveConnections(Empty) → ActiveConnectionsResponse` RPC now exposes a snapshot of active connections (via `conntrack -L`), enriched (process + reverse DNS) and evaluated against the rules, without publishing any event. Each entry is encoded as a `connection_detected` `DomainEventMessage` so the frontend reuses its existing rendering logic. On mount, the Connections tab seeds its store from this snapshot (best-effort), then the event stream takes over.

## [0.3.2] - 2026-07-15 · "Visible connections: supplementary-group auth"

### Fixed

- **The normally-launched UI was denied by the daemon** (empty Connections tab, no data arriving): SO_PEERCRED gRPC auth only compared the client's *primary* gid against the `syswall` group. A normally-launched `syswall-ui` (desktop icon, direct command) runs with the user's personal group as primary gid — `syswall` being only a *supplementary* group — hence a repeated denial (`gRPC denied: … gid=…`) every 2 s, leaving only `sg syswall -c syswall-ui` working. `PeerAuthPolicy::permits` now also consults the caller's **supplementary** groups (`getpwuid` + `getgrouplist`, resolver injected for tests). The UI now connects from the desktop icon, without `sg`.

## [0.3.1] - 2026-07-15 · "One-command deployment"

### Fixed

- **The daemon failed to start under systemd**: the unit did not reference the config, so the daemon looked for `config/default.toml` relative to cwd (`/` under systemd) and crashed. Added `Environment=SYSWALL_CONFIG=/etc/syswall/config.toml`.
- **`User=syswall` without a user**: the unit runs as `User=syswall` but the installers only created the group (group alone makes `systemctl start` fail). `install.sh` and `system/arch/syswall.install` now create the dedicated system user (`useradd --system`).
- **`DecisionAction` type out of sync with the protocol**: the `defer:N` action (snooze, accepted by gRPC since V0.3.0) was missing from the TS type, failing `npm run check` (CI job `ui`). Added the `` `defer:${number}` `` template-literal member to the union.
- **`install.sh` broken by the UI build**: `cd crates/ui && … && cd ../..` left the cwd inside `crates/ui` when the UI build failed (`set -e` exception on an `&&` list), crashing the daemon copy step. Root resolved absolutely, UI build isolated in a non-fatal subshell, `tauri build --no-bundle` called directly via the CLI (the `--` of `npm run` was swallowing the flag, so AppImage bundling — which needs network access via linuxdeploy — ran anyway), UI binary path fixed (`target/release/ui`), and sudo authentication requested upfront (fail fast instead of after several minutes of build).

### Added

- **One-command install**: `system/install.sh` detects the distribution (`/etc/os-release`), cleanly refuses systems without systemd, points to the native package on the Arch family, then **installs AND starts** the service (`systemctl restart` + status).
- **CI guard** `system/tests/check-service-config.sh`: checks unit/installer consistency (`SYSWALL_CONFIG`, `syswall` user, distro detection, service start). Wired into the `hardening-check` job.

### Changed

- **Workspace-wide `cargo fmt --all`** (rustfmt edition 2024): formatting had drifted and was failing the CI `lint` job. No logic changed.

### Security

- **`npm audit fix`** on the UI: fixes 3 high and 2 moderate front-end toolchain advisories (@sveltejs/kit, vite, svelte, postcss, cookie, devalue), transitive and non-breaking (semver). Unblocks the CI `audit` job.
- **`crossbeam-epoch` 0.9.18 → 0.9.20** (RUSTSEC-2026-0204, invalid pointer dereference): transitive dev dependency (criterion → rayon), fixed via `cargo update`. Unblocks the `deny` and `audit` jobs.
- **`lru` 0.12 → 0.16** (RUSTSEC-2026-0002, `IterMut` soundness): direct dependency of `infra` (process/DNS cache), API-compatible (`new`/`get`/`put`/`pop`). SysWall did not use `iter_mut`, but the bump removes the advisory at the source.

### CI

- **CI repaired** (red for several months due to infra issues): `protoc` installed in the jobs that compile `syswall-proto` (lint, test, gRPC, nfqueue, fuzz), `ui` excluded from `cargo test --workspace` (GTK/WebKit libs absent from the runners), and `cargo audit --ignore` on the `quick-xml` advisories (RUSTSEC-2026-0194/0195 — transitively macOS-only via plist/tauri, unreachable on Linux at the time). *(Superseded in 0.3.5: the ignores were dropped once a `plist` bump made the fix reachable.)*

## [0.3.0] - 2026-05-05

Post-V0.2 stabilization release: technical polish, CI hardening.

### Added

- **Real `Defer` action on the decision popup**: new `DecisionAction::Defer { duration_secs: u64 }` variant that snoozes the `dedup_key` in memory for N seconds (1..=86400). New matching flows fall back to Drop without a popup, then the decision re-triggers after expiry. UI shortcut `Esc` → `defer:300` (5 min). gRPC parser for `defer:N` with bound validation.
- **Typed `VerdictWaitError`**: replaces the silent `Ok(Drop)` fallback in `wait_for_verdict` with 3 typed variants (`Timeout`, `ChannelClosed`, `ChannelLagged { missed }`). Dedicated audit per variant with adapted severity (Warning for timeout, Error for channel) and machine-readable `wait_error` metadata.
- **Criterion benchmarks**: `crates/infra/benches/nfq_parser_bench.rs` (IPv4/IPv6 TCP/UDP parsing: ~160-465 ns/packet) and `crates/app/benches/dedup_key_bench.rs` (~360 ns/call). The `policy_bench.rs` bench pre-existed.
- **Real gRPC test harness**: `crates/daemon/tests/grpc_limits_test.rs` is no longer a skeleton. `message_over_1mib_is_rejected` sends 2 MiB and checks `OutOfRange`; `small_message_is_accepted` validates the happy path. `concurrency_limit_is_enforced` remains TODO for V0.4.
- **`cargo deny` in CI**: new `deny.toml` (license whitelist MIT/Apache-2/BSD/ISC/Zlib/MPL-2/Unicode-3.0/CC0/0BSD), 17 RUSTSEC advisories ignored line-by-line for unmaintained transitive Tauri/GTK/wry crates. `deny` job in CI via `EmbarkStudios/cargo-deny-action@v2`.
- **Dedicated CI jobs**: `grpc-integration` (env `SYSWALL_TEST_GRPC=1`, unprivileged) and `nfqueue-smoke` (sudo + `modprobe nfnetlink_queue`, `continue-on-error: true` since the runner may lack `CAP_NET_ADMIN`).
- **`PolicyEngine` property tests**: 6 invariants checked via proptest (default policy, no-panic, matched-rule consistency, first-match-wins, IP-family isolation, inclusive port bounds) in `crates/domain/tests/policy_engine_proptest.rs`.
- **`cargo-fuzz` fuzzing**: 3 libFuzzer targets on untrusted input surfaces — JSON criteria/scope/rule (`crates/domain/fuzz`), TOML config and the `CreateRuleRequest` gRPC converter with biased Arbitrary inputs (`crates/daemon/fuzz`). CI job `fuzz-smoke` (60 s/target, nightly).
- **Daemon lib**: `crates/daemon/src/lib.rs` exposes `config` and `grpc` for integration tests and fuzzing (the binary itself is unchanged).

### Changed

- **Workspace version** bumped `0.2.0 → 0.3.0`. The new `DecisionAction::Defer { .. }` variant is technically breaking for gRPC clients doing an exhaustive match, justifying the minor bump.
- **Tests moved out of infra god-files**: `crates/infra/src/nftables/translator/mod.rs` 511 → 113 LOC (production-only); `crates/infra/src/nftables/adapter/mod.rs` 717 → 606 LOC. Tests moved into sibling `tests.rs` files via `#[cfg(test)] mod tests;` (preserves `pub(super)` visibility).
- **License declared explicitly** on every workspace crate (via `license.workspace = true`), plus `crates/ebpf-prog` and `crates/ui/src-tauri` (`license = "MIT"` directly, being outside the workspace or under a different config).

### Documentation

- **`docs/roadmap-2026-2027.md`**: V0.3 → V1.0 roadmap over 6-9 months (Stabilization, UX+i18n, Packaging, Ecosystem).
- **GitHub Release V0.2.0** published with a structured body (highlights, prerequisites, full changelog).

### Notes

V0.3.6 (final professionally-designed logo) remains external: the SVG logo shipped in V0.2.0 is a clean placeholder (shield + interception net); finalization by a graphic designer is still to be scheduled.

---

## [0.2.0] - 2026-05-05

### Added

- **30 s antilockout**: automatic rollback of rule changes if external connectivity is lost within 30 s of applying (`AntilockoutGuard` + `TcpProbe`). Configurable endpoints under `[antilockout] endpoints = [...]`.
- **SO_PEERCRED peer authentication** on the gRPC socket: only `root` or members of the `syswall` system group can open a session. Denials are audited.
- **Audit categories**: `EventCategory::Antilockout`, `EventCategory::Authentication`.
- **Domain error**: `DomainError::AntilockoutTriggered { rolled_back_count }`.
- **Strict Tauri CSP** in the UI window (no `unsafe-eval`).
- **gRPC limits**: 1 MiB max decoding, 4 MiB max encoding, 64 concurrent streams per connection, 30 s timeout.
- **Critical UI toast** on `AntilockoutTriggered` events.
- **Unified install scripts** in `system/install/postinst.sh` (create the `syswall` user/group).

### Changed

- **Hardened systemd service**: `User=syswall` (dedicated user), `AmbientCapabilities` (no more root), `ProtectSystem=strict`, `RestrictAddressFamilies`, `SystemCallFilter`, `LockPersonality`, `NoNewPrivileges`, etc.
- **Daemon startup**: `panic!` replaced with `Result<(), StartupError>` + `exit(78)` (EX_CONFIG from sysexits.h) for boot failures.

### Fixed

- The `firewall.rollback_timeout_secs` field was declared but never read (compiler warning). It is now used by the antilockout guard.

### Security

- **Pre-V0.2**: any executable run by a user in the `syswall` group could disable the firewall via the gRPC socket without authentication. Resolved by `SO_PEERCRED`.
- **Pre-V0.2**: the daemon ran as `User=root` with no restriction (any memory-safety exploit meant full root). Resolved by a dedicated user + capability bounding + sandboxing.

### Documentation

- README: Security section strengthened in FR+EN.
- `crates/ui/CLAUDE.md`: manual CSP verification procedure.
- `docs/superpowers/specs/2026-05-05-security-hardening-design.md`: design spec.
- `docs/superpowers/plans/2026-05-05-security-hardening-plan.md`: TDD implementation plan.
- `docs/audit-2026-05-04.md`: full audit behind this release.

### Code Hygiene

- **Production `unwrap()` eradicated**: ~63 production occurrences replaced with `?` (propagation) or documented `expect("invariant in French")`. The `domain`, `app`, `daemon`, `infra`, `ebpf` crates now enable `#![cfg_attr(not(test), deny(clippy::unwrap_used))]`.
- **God-modules split**: `policy_engine` (matcher + evaluator), `audit_service` (command + query, CQRS-light), `converters` (rule + decision + audit + connection + error + parsers + event), `audit_repository` (queries + writes + migration), `translator` (criteria + verdict + system_rules), `adapter` (apply + rollback + whitelist). No production module > 500 LOC except integration tests.
- **Workspace version**: aligned to `0.2.0` (consistent with the system packages).
- **Cargo dependency `infra -> app`**: removed (was unused, a hexagonal-architecture violation at the Cargo level).
- **CI**: `cargo clippy --workspace --exclude ui --all-targets -- -D warnings` is now a mandatory gate.
- **24 `infra` clippy warnings**: all fixed.
- **21 pre-existing `daemon` clippy warnings**: all fixed.

### Active Blocking (NFQUEUE)

- **New `PacketInterceptor` port** + `NfqueueInterceptor` adapter (`crates/infra/src/nfqueue/`): intercepts the first packet of every new outbound flow via `nfnetlink_queue` and synchronizes the verdict with the user's decision.
- **`LearningService` implements `PacketDecisionHandler`**: evaluates via `PolicyEngine`, manages `PendingDecision` creation, and waits (≤ 28 s) for the verdict via `VerdictBroadcasts`.
- **Built-in deduplication**: a single popup per `(app, remote_ip, remote_port, protocol)` even under burst.
- **`interception` nft rule**: `iif lo accept` then `ct state new queue num 0 bypass` added at boot.
- **Degraded mode**: the daemon still starts if NFQUEUE fails.
- **`[nfqueue]` config**: `enabled`, `queue_num`, `max_queued`, `overflow_policy`.
- **Documented limit**: 28 s timeout per decision (the kernel drops at 30 s); `Severity::Warning, Category::Decision` audit on expiry.
- **Smoke test** gated by `SYSWALL_TEST_NFQUEUE` in `crates/daemon/tests/nfqueue_smoke_test.rs`.

Cargo dependencies added: `nfq = "0.2.5"`, `etherparse = "0.20"` (workspace, in `crates/infra/Cargo.toml`).

### UX & Accessibility

- **Immediate killswitch + 5 s undo toast**: no more paradoxical confirmation modal on an emergency action; a persistent 5 s undo covers mobile mistaps.
- **Decision popup keyboard shortcuts**: `a`/`Enter` (allow once), `b` (block once), `Shift+A` (always allow), `Shift+B` (always block), `i` (ignore), `Esc` (ignore). Keys shown via `<kbd>` tags.
- **Virtualized Audit page**: pagination replaced by `Table.svelte` virtual scroll. Smooth scrolling on >= 5000 events.
- **Modal focus trap** + focus restoration on close (Svelte `focusTrap` action). WCAG 2.4.3 compliance.
- **WCAG AA contrast**: `--text-tertiary` raised from `#636366` (3.4:1) to `#8e8e93` (4.6:1). New `--text-disabled` token for decorative uses only. `--text-secondary` moved to `#c7c7cc` to preserve the hierarchy.
- **Filter debounce**: Connections and Audit search debounced by 250 ms.
- **Rule toggles**: `role="switch"` + `aria-checked` for screen readers.
- **Mobile sidebar**: tap targets >= 44x44 px (WCAG 2.5.5).
- **Extensible toast**: new `action?: { label, handler }` field + visual progress bar.

New utilities: `crates/ui/src/lib/utils/debounce.ts`, `crates/ui/src/lib/actions/focus_trap.ts`.

### Design Polish

- **Sharper art direction**: macOS Dark kept, SysWall identity accent `#2cd4d4` (cyan turquoise) reserved for key moments (logo, active killswitch, interception net).
- **Self-hosted web fonts**: Inter Variable + JetBrains Mono in `crates/ui/static/fonts/`. `font-display: swap`.
- **SysWall SVG logo**: `SyswallLogo.svelte` component (mark + wordmark) + `favicon.svg`.
- **Lucide icons**: sidebar emoji replaced (LayoutDashboard, Network, Shield, BrainCircuit, Ban, ClipboardList, Settings).
- **Dense table polish**: zebra-striping (`--bg-row-stripe`), sticky header shadow on scroll, global `font-variant-numeric: tabular-nums`, stronger row hover (`--bg-row-hover`).
- **`StatCard:hover`**: 1 px lift + shadow.
- **`Input`**: `error` (red border + helper text) and `disabled` (0.5 opacity) states.
- **Card**: `glow` prop removed (YAGNI).
- **Killswitch pulse**: 2 s ease-in-out cyan when the network is active. Disabled under `prefers-reduced-motion: reduce`.
- **Tokens**: `--accent-syswall`, `--accent-syswall-dim`, `--accent-syswall-glow`, `--bg-row-hover`, `--bg-row-stripe`, `--shadow-sticky-header`.

### Config wiring (sub-project C.1)

Five TOML config fields previously flagged `never read` by the compiler are now used:

- **`daemon.watchdog_interval_secs`**: periodic `sd_notify(WATCHDOG=1)` sent to systemd. If `NOTIFY_SOCKET` is absent (launched outside systemd), silent no-op. Send frequency = `interval_secs / 2` (systemd recommendation).
- **`database.journal_retention_days`**: daily tokio rotation task that deletes `audit_events` older than `Utc::now() - retention_days`. `0` disables rotation. New `AuditRepository::delete_before(cutoff)` method.
- **`learning.enabled`**: `false` fully disables `PendingDecision` creation; flows without a matching rule fall back to `default_policy`.
- **`learning.default_timeout_action`**: action applied after an NFQUEUE verdict expires (`"allow"` or `"block"`, default `"block"`).
- **`learning.overflow_action`**: action applied when `max_pending_decisions` is reached (`"allow"` or `"block"`, default `"block"`). `Severity::Warning, Category::Decision` audit on saturation.

Three fields removed (YAGNI — no added value):

- `daemon.log_dir` (handled by systemd `LogsDirectory=syswall` + journald).
- `ui.theme` (SysWall is dark-only by design).
- `ui.refresh_interval_ms` (the UI uses gRPC streams, not polling).

New Cargo dependency: `sd-notify = "0.4"` (raw UnixDatagram, no `-lsystemd` required).
