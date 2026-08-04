# Directives du projet SysWall

> Document vivant : à chaque passage de revue, 1 à 3 règles s'ajoutent ici — les plus
> manquantes à ce moment pour **ce** dépôt, jamais un déversement générique. Chaque règle
> cite une commande exacte ou un seuil chiffré, pas un vœu pieux, et doit être vraie pour
> SysWall aujourd'hui (ou accompagnée du changement qui la rend vraie).

## 1. Zéro warning clippy sur le workspace applicatif

`cargo fmt --all -- --check` et `cargo clippy --workspace --exclude ui --all-targets -- -D warnings`
doivent passer sans un seul warning avant tout commit — c'est déjà le gate bloquant du job
`lint` de la CI (`.github/workflows/ci.yml`), ce document ne fait que le rendre explicite
pour qui lit le dépôt hors CI. Le crate `ui` (Tauri) est volontairement exclu de clippy :
il dépend des conventions de lint propres à Tauri/GTK/WebKit, hors du périmètre de ce
gate ; côté JS/TS, `npm run check` (svelte-check) reste son garde-fou équivalent.

## 2. Audit de dépendances multicouche avant toute release

Trois outils, trois angles, systématiquement avant de taguer une version :
`cargo audit --ignore <ID> ...` (vulnérabilités RustSec), `cargo deny --config deny.toml check`
(licences, bans de doublons, source des crates), et `cd crates/ui && npm audit --audit-level=high`
côté frontend. Un advisory ignoré n'est jamais silencieux : il vit dans `deny.toml` (section
`[advisories].ignore`) ou dans le job `audit` du workflow CI, toujours commenté avec sa raison
et sa date de réévaluation (ex. « transitif Tauri/GTK, à retirer au prochain bump majeur »).
Un advisory qui **peut** être corrigé sans rupture (comme quick-xml/plist en 0.3.7, RUSTSEC-2026-0194/0195)
est corrigé immédiatement, pas seulement ignoré.

## 3. Le compteur de tests du README et le CHANGELOG bilingue ne dérivent jamais en silence

Constat concret : le nombre de tests affiché dans `README.md` (badge + 3 mentions textuelles)
a dérivé trois fois de suite (250 → 356 en 0.3.5/0.3.6, puis 356 → 358 en 0.3.7) faute de
vérification systématique au moment du commit. Règle : toute passe de documentation qui touche
au nombre de tests recompte avec la commande exacte de la CI —
`cargo test --workspace --exclude ui 2>&1 | grep -oE 'ok\. [0-9]+ passed' | grep -oE '[0-9]+' | awk '{s+=$1} END{print s}'`
— et aligne les 4 mentions du README (badge, principe TDD, bloc `cargo test`, tableau
Statistiques) dans le **même commit** que l'entrée `CHANGELOG.md` (FR) et son miroir exact
`CHANGELOG.en.md` (EN). Cette règle est en cours d'outillage : `system/tests/check-readme-test-count.sh`
(proposé via la branche `lutin/ameliorations`) fera échouer la CI si une de ces mentions
diverge du compte réel, plutôt que de compter sur la vigilance humaine à chaque passage.
