# Identité de destination dans le prompt de nouvelle connexion

> Sous-projet #1 du chantier « le prompt doit dire ce que c'est ».
> Date : 2026-08-05 · Cible de version : **0.3.9** (mineure — ouvre l'axe ② DNS/domaines).

## Problème

Quand une fenêtre de nouvelle connexion s'affiche, l'utilisateur « ne sait pas ce que
c'est ». Le prompt (`DecisionPrompt.svelte`) est **centré IP** : il montre
`destination = 140.82.121.4:443` sans jamais dire que c'est `github.com`.

Or les briques existent déjà mais sont **du code mort, jamais câblé** :

- `crates/infra/src/dns/snooper.rs` : `DnsSnoopCache` (cache IP→domaine avec TTL :
  `insert`/`get`/`evict_expired`) et `parse_dns_response(&[u8]) -> Vec<(String, IpAddr, u32)>`
  — écrits et testés, **jamais alimentés ni appelés**.
- `crates/infra/src/dns/mod.rs` : `DnsResolver` (reverse-DNS + cache LRU), implémente le
  port `DnsResolver` — mais le snapshot est **codé en dur à `hostname: None`**.
- `ConnectionSnapshot` (domaine) possède déjà un champ `hostname` ; le type TS UI ne l'a pas.

## Objectif

Afficher l'**identité de destination** dans le prompt :

- le **domaine que l'appli a résolu** (ex. `github.com`) en primaire ;
- une **ligne résumé en clair** : « AppX → github.com:443 » en tête du prompt ;
- l'IP reléguée en sous-titre monospace ;
- **repli gracieux** : domaine (snoop) → reverse-DNS (PTR) → IP brute.

Hors périmètre de ce sous-projet (traités plus tard) : identité de l'appli renforcée (#2),
contexte de risque / GeoIP / réputation (#3), refonte lisibilité complète (#4).

## Décision d'architecture : observer les réponses DNS via NFQUEUE entrant

Le pipeline d'interception actuel est **output-only** (chaîne `inet syswall interception`,
hook `output` priorité 0, policy accept, bypass loopback, `queue` du 1er paquet des flux
avec `bypass` = fail-open ; whitelist DNS/DHCP/NTP acceptée sans queue).

Pour mapper IP→domaine il faut la **réponse** DNS (name→IP), donc du trafic **entrant**.
Trois mécanismes ont été comparés :

| Option | Réutilise | Risque DNS | Verdict |
|--------|-----------|------------|---------|
| **NFQUEUE entrant** (retenu) | CAP_NET_ADMIN, NFQUEUE, `parse_dns_response` | Faible (bypass fail-open) | ✅ |
| Sniffer passif (raw socket) | — | Nul (hors chemin) | Nouvelle CAP_NET_RAW, élargit le sandbox systemd |
| eBPF | ring buffer socket | Nul | Surdimensionné (parsing DNS/IPv6 en eBPF) |

**Retenu : NFQUEUE entrant.** Une nouvelle règle nft queue les réponses DNS
(`udp sport 53`) vers une **queue dédiée `bypass`** ; le daemon parse puis ACCEPT.
Aligné sur l'archi existante, **aucune nouvelle capability** (préserve le durcissement
systemd et son check CI), fail-open préservé.

## Flux de données

```
Réponse DNS (in, udp sport 53)
  └─ nft input: queue num <DNS_QUEUE> bypass
       └─ [daemon] consommateur NFQUEUE-DNS
            ├─ parse_dns_response(payload) -> [(host, ip, ttl)]
            ├─ DnsSnoopCache.insert(ip, host, ttl)   (Arc partagé)
            └─ verdict ACCEPT (toujours, même sur erreur)

Nouvelle connexion sortante interceptée
  └─ [app] construction du PendingDecision / ConnectionSnapshot
       └─ résolution du nom de destination :
            1. DnsSnoopCache.get(dest_ip)            (domaine demandé — précis)
            2. sinon DnsResolver.resolve(dest_ip)    (reverse PTR, timeout ~200 ms)
            3. sinon None
       └─ snapshot.hostname = Some(nom) | None
  └─ sérialisation snapshot_json (inclut hostname)
  └─ [UI] DecisionPrompt : ligne résumé + hostname primaire, IP secondaire
```

## Composants (unités testables)

1. **`DnsSnoopCache`** (`infra/dns/snooper.rs`) — *existant, inchangé*. Store IP→domaine.
   Devient un `Arc<DnsSnoopCache>` partagé, possédé par le daemon.

2. **Consommateur NFQUEUE-DNS** (*neuf*, `infra`/`daemon`) — unité focalisée :
   lire le paquet queué → `parse_dns_response` → `insert` dans le cache → ACCEPT.
   Réutilise la machinerie NFQUEUE existante ; nouvelle constante `DNS_QUEUE_NUM`.
   Boucle indépendante de la boucle d'interception principale.

3. **Règle nft DNS** (*neuf*, `infra/nftables/translator`) — l'interception existante
   étant sur le hook `output`, l'observation des réponses DNS (entrantes) exige une
   **nouvelle chaîne dédiée sur le hook `input`** (`type filter hook input priority 0 ;
   policy accept ;`), contenant `udp sport 53 queue num <DNS_QUEUE_NUM> bypass`. Elle est
   créée/détruite avec le reste du ruleset syswall.

4. **Étape de nommage de destination** (*neuf*, `app`) — `resolve_destination_name(ip)` :
   snoop-cache puis reverse-DNS borné par timeout. Injectée là où se crée le
   `PendingDecision`. Dépend de `Arc<DnsSnoopCache>` + `Arc<dyn DnsResolver>`.

5. **`DecisionPrompt.svelte`** (`ui`) — *affichage seul*. Ajout `hostname?: string` au type
   `ConnectionSnapshot`, parsing, ligne résumé « {app} → {host|ip}:{port} », host primaire.

## Gestion d'erreurs & invariants

- **Ne jamais casser le DNS** : queue `bypass` (fail-open si le daemon ne consomme plus) ;
  toute erreur du consommateur → ACCEPT quand même ; paquet DNS malformé →
  `parse_dns_response` renvoie une liste vide, aucune panic.
- **Prompt jamais retardé** : le reverse-DNS de secours est borné par un timeout
  (~200 ms) ; au-delà, `hostname = None` et on affiche l'IP.
- **TTL** : respecté via `insert(ip, host, Some(ttl))` ; `evict_expired` appelé
  périodiquement (tâche de fond du daemon).
- **Zéro régression** : si rien ne résout, le prompt affiche l'IP exactement comme
  aujourd'hui.

## Plan de tests (TDD)

- **Consommateur** : une réponse DNS captée (wire format connu) alimente correctement le
  cache (host, ip, ttl) ; un paquet tronqué/malformé → cache inchangé, pas de panic.
- **Nommage** : priorité snoop sur reverse ; repli reverse quand snoop vide ; `None`
  quand les deux échouent ; le timeout n'excède pas la borne (fake resolver lent).
- **Sérialisation** : `snapshot_json` contient `hostname` quand présent, l'omet/`null` sinon.
- **nftables** : la génération de règles inclut la règle DNS-queue (hook input, sport 53,
  `queue ... bypass`).
- **UI** : le parsing du snapshot lit `hostname` ; la ligne résumé rend le host quand
  présent, l'IP quand absent ; l'IP reste visible en secondaire.

## Versionnage & livraison

- Fonctionnalité → **mineure**. Cible **0.3.9** (bump Cargo + `Cargo.lock` + entrée
  `CHANGELOG.md` FR et `CHANGELOG.en.md`).
- Vérification E2E requise (comportement UI modifié) : voir le prompt afficher un domaine
  réel sur une connexion sortante après une résolution DNS.
