# Mon verdict

Je valide largement :

* **workspace en crates séparées**
* **domain sans dépendances infra**
* **gRPC over Unix socket**
* **daemon root séparé de l’UI**
* **nftables + conntrack**
* **SQLite pour les fondations**
* **Tauri thin client**
* **EventBus interne**
* **testing pyramid propre**
* **config typée et fail-fast**

C’est un bon socle.

En revanche, il y a quelques points où je te recommande de **resserrer le design maintenant**, sinon tu vas te retrouver avec :

* des incohérences async/sync
* un domaine un peu trop “anémique”
* des ports trop génériques
* des problèmes de concurrence et de lifecycle
* quelques angles morts sécurité
* une complexité proto/UI qui peut gonfler trop vite

---

# Ce que je garderais tel quel

## 1. La séparation des responsabilités

Ton découpage `domain / app / infra / proto / daemon / ui` est bon.

Le point fort ici :

* le **domain** exprime le métier
* le **app** orchestre
* le **infra** adapte
* le **daemon** compose
* le **ui** consomme uniquement un contrat

C’est exactement ce qu’il faut pour éviter un monolithe Tauri où tout finit entremêlé.

## 2. Le daemon privilégié séparé

Très bon choix.
Un firewall desktop qui exécute tout en root côté UI serait une erreur de conception.

Le couple :

* **daemon systemd root**
* **UI non privilégiée**
* **Unix socket restreint**

est le bon modèle.

## 3. Le contrat gRPC

Très bonne décision pour :

* le streaming
* les DTO explicites
* la séparation UI/métier
* la testabilité

Pour un produit temps réel avec events, décisions, stats et audit, c’est bien meilleur qu’un simple bridge Tauri custom.

## 4. Le focus sur des ports métier

Tes ports ont la bonne intention.
Tu pars d’un **hexagonal réel**, pas cosmétique.

## 5. Le rollback nftables

Excellent point.
Le firewall lockout est un vrai risque produit. Le fait d’y penser dès les foundations est un très bon signal.

---

# Les points à corriger avant d’implémenter

## 1. Il faut unifier complètement ta stratégie async

Aujourd’hui, ta spec dit :

* certains ports sync
* d’autres async
* certains sync mais appelés via `spawn_blocking`
* SQLite async “par adaptation”

Ça peut marcher, mais ça va vite devenir bancal.

### Ce que je recommande

Choisis une ligne claire :

* **tous les ports applicatifs = async**
* même si l’implémentation réelle est sync en dessous
* les adapters gèrent eux-mêmes :

  * `spawn_blocking`
  * timeouts
  * retries éventuels
  * protection de concurrence

### Pourquoi

Parce que sinon :

* tes services `app` vont mélanger sync et async
* tes tests vont être plus hétérogènes
* les services d’orchestration vont devenir plus lourds
* tu vas avoir des signatures instables quand une implémentation change

### Recommandation

Passe tout en :

```rust
#[async_trait]
pub trait RuleRepository { ... }

#[async_trait]
pub trait AuditRepository { ... }

#[async_trait]
pub trait DecisionRepository { ... }

#[async_trait]
pub trait FirewallEngine { ... }

#[async_trait]
pub trait ConnectionMonitor { ... }

#[async_trait]
pub trait ProcessResolver { ... }

#[async_trait]
pub trait EventBus { ... }

#[async_trait]
pub trait UserNotifier { ... }
```

Même `ProcessResolver`.
Le coût conceptuel est plus faible que de garder une exception sync.

---

## 2. `ConnectionMonitor::start()` est trop flou

Tu as :

```rust
fn start(&self) -> Result<ConnectionStream, DomainError>;
```

Le problème, c’est le cycle de vie.

### Les questions implicites non tranchées

* Peut-on appeler `start()` plusieurs fois ?
* Est-ce idempotent ?
* Qui possède le stream ?
* Comment on arrête proprement ?
* Comment on redémarre après crash du sous-système ?
* Comment on expose les erreurs runtime du stream ?

### Ce que je recommande

Séparer clairement :

* **snapshot**
* **stream**
* **lifecycle**

Par exemple :

```rust
#[async_trait]
pub trait ConnectionMonitor: Send + Sync {
    async fn stream_events(&self) -> Result<ConnectionEventStream, DomainError>;
    async fn get_active_connections(&self) -> Result<Vec<Connection>, DomainError>;
}
```

Et côté daemon / composition root :

* un **supervisor** démarre la tâche de monitoring
* un **cancellation token** gère l’arrêt
* les erreurs de stream sont republiées dans l’`EventBus`

Le port ne doit pas porter toute la sémantique de lifecycle.

---

## 3. `RuleRepository::find_matching()` fuit un peu de logique métier infra

L’intention est bonne, mais ce port mélange deux niveaux :

* repository de persistance
* moteur de matching métier

Alors que ta spec dit aussi que le matching reste dans le domaine.

### Le problème

Si `find_matching(connection)` existe, l’infra peut être tentée de :

* filtrer en SQL
* sérialiser des critères
* embarquer une partie du matching hors domaine

### Ce que je recommande

Le repository doit rester centré sur la persistance :

```rust
async fn list_enabled_ordered(&self) -> Result<Vec<Rule>, DomainError>;
```

Puis le matching est fait par un **service métier dédié** ou directement par une **Specification** du domaine.

### Variante encore meilleure

Créer un composant métier explicite :

* `RuleEvaluator`
* `RuleMatcher`
* `PolicyEngine`

Ça clarifie énormément les responsabilités.

---

## 4. `RuleService::evaluate()` ne devrait pas être dans un service CRUD

Aujourd’hui `RuleService` fait trop de choses :

* CRUD
* import/export
* toggle
* apply nft
* évaluation métier

Ça commence à faire plusieurs responsabilités.

### Je te conseille de scinder

#### `RuleService`

* create
* update
* delete
* toggle
* list
* import/export

#### `PolicyEngine` ou `RuleEvaluator`

* evaluate(connection) -> verdict
* first_matching_rule(connection)
* explain_match(connection)

#### `FirewallSyncService`

* reconcile DB ↔ nftables
* apply/remove/sync

Tu restes beaucoup plus propre côté SRP.

---

## 5. `LearningService` ne doit pas bloquer sur l’UI

Ta spec suggère :

* connexion inconnue
* prompt
* décision
* application

Le piège, c’est de transformer le flux temps réel en flux bloquant.

### Risque

Si `UserNotifier.prompt_decision()` est conçu comme un appel direct qui attend une réponse, tu couples :

* monitoring
* bus d’événements
* disponibilité de l’UI
* vitesse de réaction utilisateur

C’est dangereux.

### Ce que je recommande

Le flow doit être **asynchrone par conception** :

1. `ConnectionService` détecte `Unknown`
2. publie `DecisionRequired`
3. `LearningService` crée un enregistrement `PendingDecision`
4. l’UI reçoit l’événement
5. l’utilisateur répond plus tard
6. `RespondToDecision` traite la réponse
7. si timeout, la policy par défaut s’applique

Donc :

* **pas de prompt bloquant**
* **pas de dépendance directe à l’UI**
* `UserNotifier` devrait être repensé

### À la place

Je remplacerais :

```rust
trait UserNotifier {
    fn prompt_decision(&self, connection: &Connection) -> Result<Decision, DomainError>;
}
```

par quelque chose comme :

```rust
#[async_trait]
pub trait DecisionRequestRepository {
    async fn create_pending(&self, request: &PendingDecision) -> Result<(), DomainError>;
    async fn list_pending(&self) -> Result<Vec<PendingDecision>, DomainError>;
    async fn resolve(&self, response: &DecisionResponse) -> Result<Decision, DomainError>;
}
```

et un notifier non bloquant :

```rust
#[async_trait]
pub trait UserNotifier {
    async fn notify_decision_required(&self, request: &PendingDecision) -> Result<(), DomainError>;
    async fn notify(&self, notification: &Notification) -> Result<(), DomainError>;
}
```

L’UI devient un consumer, pas un maillon synchrone du cœur métier.

---

## 6. Il manque un vrai type `PendingDecision`

Tu parles de pending decisions, mais il manque l’objet métier central.

Il te faut une entité dédiée, pas juste un concept implicite.

### Je recommande d’ajouter

```rust
struct PendingDecision {
    id: PendingDecisionId,
    connection_snapshot: ConnectionSnapshot,
    requested_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    deduplication_key: String,
    status: PendingDecisionStatus,
}
```

Avec :

```rust
enum PendingDecisionStatus {
    Pending,
    Resolved,
    Expired,
    Cancelled,
}
```

Ça te simplifie :

* debounce
* timeout
* queue UI
* redémarrage daemon
* audit
* reprise après crash

---

## 7. Le domaine mérite plus d’invariants explicites

Ton domain model est bon, mais encore un peu “data-centric”.
Il faut renforcer les invariants métier dans les constructeurs.

### Exemples

* `Port` ne doit pas accepter `0` si ton métier le refuse
* `RulePriority` devrait être un value object dédié
* `RuleName` pourrait imposer longueur / caractères
* `SocketAddress` devrait garantir cohérence IP/port
* `TemporaryRuleScope` doit refuser un `expires_at` passé
* `DecisionGranularity` doit être compatible avec la donnée disponible

### Recommandation

Évite les `String`, `u32`, `u64` nus quand ils portent une règle métier importante.

Crée des VO :

* `RulePriority`
* `AppName`
* `ExecutablePath`
* `Username`
* `BytesCount`
* `DebounceKey`

Le domaine sera plus robuste, plus testable et plus auto-documenté.

---

## 8. `Verdict` et `RuleAction` ne sont pas assez découplés

Aujourd’hui tu as :

* `RuleAction`: Allow, Block, Log, Ask
* `Verdict`: Pending, Allowed, Blocked, Unknown

Mais `Log` et `Ask` ne sont pas des verdicts finaux.

### Le risque

Confusion entre :

* résultat d’évaluation
* effet opérationnel
* décision finale
* état transitoire

### Je recommande

Séparer :

#### `RuleEffect`

```rust
enum RuleEffect {
    Allow,
    Block,
    Ask,
    Observe,
}
```

#### `ConnectionVerdict`

```rust
enum ConnectionVerdict {
    Unknown,
    PendingDecision,
    Allowed,
    Blocked,
    Ignored,
}
```

C’est plus clair.

---

## 9. Le `EventBus` broadcast seul n’est pas suffisant pour tous les usages

`tokio::broadcast` est très bien pour :

* fan-out
* streams temps réel
* events éphémères

Mais pas pour tout.

### Problème

Les événements critiques ne doivent pas dépendre d’un subscriber vivant au bon moment.

Exemples :

* décision requise
* erreur critique
* changement de config
* audit important

### Recommandation

Distingue deux familles :

#### Événements volatils

* broadcast
* temps réel
* UI live

#### Événements persistés / commandes durables

* base SQLite
* file pending decisions
* audit

Le bus ne remplace pas la persistance.

---

## 10. Le proto mérite une séparation “control plane / event plane”

Ta séparation RPC / streaming est bonne, mais je pousserais un peu plus loin.

### Recommandation

Deux services proto distincts :

* `SysWallQueryService`
* `SysWallCommandService`
* `SysWallEventService`

Pourquoi :

* responsabilités mieux séparées
* permissions futures plus faciles
* meilleur découpage client
* tests plus propres

Ce n’est pas obligatoire, mais c’est plus évolutif.

---

## 11. Il manque une vraie stratégie de concurrence SQLite

SQLite est pertinent, mais il faut cadrer le modèle d’accès.

### À décider explicitement

* `rusqlite` avec thread dédié ?
* pool de connexions ?
* un writer unique ?
* WAL activé ?
* niveau d’isolation ?
* stratégie en cas de `database is locked` ?

### Recommandation concrète

Pour ce projet :

* active **WAL**
* un **writer sérialisé**
* lectures concurrentes
* batch write pour audit
* retries bornés sur contention
* migrations atomiques au boot

Ajoute-le noir sur blanc dans la spec.

---

## 12. Le `ProcfsProcessResolver` doit être traité comme “best effort”

La résolution process/socket sur Linux est utile mais imparfaite.

### Cas réels

* PID déjà terminé
* socket réutilisée
* permissions / namespace
* race conditions entre event réseau et lecture `/proc`
* conteneurs / network namespaces

### Recommandation

Spécifie explicitement :

* la résolution process est **opportuniste**
* une connexion peut rester partiellement enrichie
* toute info process est versionnée avec un `confidence level` si besoin
* l’absence de process ne doit jamais bloquer la politique de sécurité

---

## 13. Le système de whitelist initiale doit être plus prudent

L’idée est bonne, mais la whitelist automatique “DNS, DHCP, NTP…” doit être très contrôlée.

### Risque

Tu peux créer des règles trop permissives sans le vouloir.

### Mieux

Définis des règles système **minimales et explicites**, par exemple :

* loopback always allow
* trafic local indispensable du daemon
* DNS seulement si résolveur système identifié ? sinon compliqué
* DHCP/NTP à discuter selon contexte desktop

Je te conseille de distinguer :

* **bootstrap safety rules**
* **system recommendations**
* **user-confirmed defaults**

Pas tout en “hard rules non supprimables”.

---

## 14. Il manque une notion claire de “policy fallback”

Tu as `default_policy = ask | allow | block`, c’est bien, mais il faut préciser :

### Quand s’applique la fallback ?

* aucune règle matchée
* UI indisponible
* pending decision expirée
* daemon redémarre
* queue pending saturée
* notifier HS
* erreur interne ?

### Je recommande de spécifier

Une matrice explicite :

* no rule match + learning enabled → create pending decision
* no rule match + learning disabled → apply default policy
* pending timeout → apply timeout action
* queue full → apply overflow action
* internal error → fail closed or fail safe depending on category

Ça évite des comportements implicites dangereux.

---

# Les ajouts que je ferais dans la spec

## 1. Ajouter un `PolicyEngine`

C’est la pièce qui manque le plus.

### Responsabilité

* évaluer une connexion contre les règles
* retourner verdict + matched_rule + explanation
* encapsuler la logique Specification

### Exemple

```rust
struct PolicyEvaluation {
    verdict: ConnectionVerdict,
    matched_rule_id: Option<RuleId>,
    reason: EvaluationReason,
}
```

Avec :

```rust
enum EvaluationReason {
    MatchedRule { rule_id: RuleId, effect: RuleEffect },
    NoMatchingRule,
    PendingUserDecision,
    DefaultPolicyApplied,
    TemporaryBypass,
}
```

Très utile pour audit, UI et debug.

---

## 2. Ajouter une `PendingDecisionRepository`

Indispensable à mon avis.

### Pourquoi

* éviter le couplage direct UI <-> learning
* survivre aux redémarrages
* traiter les timeouts proprement
* gérer les doublons et le debounce

---

## 3. Ajouter une couche `supervision` dans le daemon

Dans `daemon`, je créerais un module en plus :

* `bootstrap`
* `runtime`
* `supervisor`

Le supervisor gère :

* démarrage des tasks
* restart interne d’un stream si recoverable
* cancellation tokens
* propagation d’erreurs
* shutdown ordonné

---

## 4. Ajouter des IDs forts partout

Je garderais tes UUID, mais via newtypes stricts :

* `ConnectionId`
* `RuleId`
* `DecisionId`
* `PendingDecisionId`
* `AuditEventId`

Très bon pour la lisibilité et la sûreté de typage.

---

## 5. Ajouter des DTO explicitement distincts des entités domaine

Le proto ne doit pas être “quasi isomorphe” au domaine sans contrôle.

Je te conseille :

* domaine = entités riches
* app = cas d’usage
* proto = DTO
* converters = module dédié

Exemple :

* `CreateRuleRequestDto`
* `RuleDetailsDto`
* `ConnectionListItemDto`

Pas juste des conversions implicites partout.

---

# Ce que je modifierais dans l’arborescence

Je la garderais proche, avec quelques ajustements.

```text
syswall/
├── Cargo.toml
├── proto/
│   └── syswall.proto
├── crates/
│   ├── domain/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── entities/
│   │       ├── value_objects/
│   │       ├── services/          # PolicyEngine, RuleMatcher
│   │       ├── ports/
│   │       ├── events/
│   │       └── errors/
│   ├── app/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── use_cases/         # create_rule, delete_rule, respond_decision...
│   │       ├── services/          # orchestration services
│   │       ├── dto/
│   │       └── errors/
│   ├── infra/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── nftables/
│   │       ├── conntrack/
│   │       ├── persistence/
│   │       ├── process/
│   │       ├── event_bus/
│   │       ├── config/
│   │       └── mappers/
│   ├── proto/
│   ├── daemon/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── bootstrap.rs
│   │       ├── supervisor.rs
│   │       ├── grpc/
│   │       ├── config.rs
│   │       └── signals.rs
│   └── ui/
```

La grosse différence :

* `domain/services` pour le vrai métier pur
* `app/use_cases` pour rendre les cas d’usage explicites
* `daemon/supervisor` pour le runtime

---

# Mes recommandations sécurité

## À renforcer explicitement

### 1. Validation du binaire connecté au socket

Tu as déjà :

* vérif que le socket est root-owned

C’est bien, mais côté daemon, je te recommande aussi :

* vérifier les **peer credentials** Unix (`SO_PEERCRED`)
* extraire `uid`, `gid`, `pid`
* refuser les clients non autorisés même si le FS est mal configuré

Ça ajoute une vraie défense en profondeur.

### 2. Encapsulation stricte des commandes système

Je formaliserais dans la spec :

* aucun `sh -c`
* uniquement `std::process::Command`
* arguments typés
* sanitation stricte
* timeout systématique
* capture stdout/stderr bornée
* codes de retour mappés en erreurs métier/infrastructure

### 3. Protection contre les règles trop larges

Ajoute une validation métier pour refuser par défaut certaines règles dangereuses :

* allow any any permanent
* suppressions de protections système
* priorité système écrasée
* règles contradictoires importées sans confirmation

### 4. Distinction des erreurs recoverable / unrecoverable

Pour le daemon, c’est important :

* erreur proto isolée → recoverable
* DB corrompue → fatal
* sync nft impossible → degraded mode
* conntrack stream down → retryable

Ça mérite des types d’erreurs dédiés.

---

# Mes recommandations test

Ta stratégie de test est bonne, mais je la rendrais encore plus précise.

## À ajouter

### 1. Property-based testing dans le domaine

Pour `RuleCriteria`, `IpMatcher`, `PortMatcher`, `Schedule`, c’est parfait.

Exemples :

* une IP exacte matche elle-même et pas une autre
* un range contient ses bornes
* toute règle vide match toute connexion
* la priorité minimale gagne toujours

### 2. Contract tests partagés pour tous les repositories

Crée une suite commune :

* save then find
* delete then missing
* list sorted
* invalid data rejected

Ensuite tu réutilises la même suite pour fake repo et SQLite repo.

### 3. Tests de résistance sur l’event bus

Cas à valider :

* subscriber lent
* burst de 10k events
* saturation
* perte contrôlée
* journalisation du lag

### 4. Tests de recovery daemon

Très importants :

* DB absente au boot
* socket déjà présent
* nft sync fail puis rollback
* shutdown pendant flush audit
* expiration de pending decisions après restart

---

# Le point le plus important à mes yeux

Le plus gros ajustement à faire, c’est celui-ci :

## Remplacer le modèle “prompt utilisateur synchrone” par un modèle “pending decision asynchrone persisté”

C’est vraiment le pivot architectural qui va rendre le produit propre.

Sinon tu risques :

* de coupler le domaine à l’interface
* de bloquer des flux
* de rendre les timeouts sales
* de compliquer les redémarrages
* de casser le TDD sur le learning mode

---

# Version resserrée de tes décisions d’architecture

Si je devais reformuler tes fondations en version plus nette, je dirais :

* **domain** contient les entités, VOs, événements, erreurs, policy engine et ports
* **app** contient les cas d’usage asynchrones
* **infra** implémente persistence, nftables, conntrack, procfs, event bus
* **daemon** compose le runtime, supervise les tâches et expose gRPC
* **ui** est un client strictement découplé, sans métier
* **learning mode** repose sur des `PendingDecision` persistées et non sur des prompts bloquants
* **rule matching** vit dans le domaine, jamais dans les repositories
* **tous les ports applicatifs sont async**
* **SQLite est en WAL avec writer maîtrisé**
* **nftables sync est rollback-safe**
* **Unix socket sécurise via permissions + peer credentials**

---

# Ce que je ferais ensuite, concrètement

L’ordre de travail que je te recommande :

1. **verrouiller la spec foundations v2**
2. **générer le workspace Cargo + crates**
3. **coder le domaine pur**
4. **coder les ports async**
5. **coder le PolicyEngine**
6. **coder les fakes de test**
7. **coder les services app**
8. **coder SQLite + EventBus**
9. **coder proto + mappings**
10. **coder daemon bootstrap**
11. **coder Tauri thin client**
12. **ensuite seulement** brancher nftables/conntrack réels

Ça t’évite de te noyer trop tôt dans l’OS integration.

---

# Ma recommandation finale

Ta spec est **bonne**, mais pour qu’elle soit vraiment prête à exécution, je ferais ces 5 changements avant de coder :

1. **tout passer en async côté ports**
2. **extraire un vrai `PolicyEngine`**
3. **introduire `PendingDecision` + repository dédié**
4. **retirer tout prompt bloquant du cœur métier**
5. **formaliser le runtime daemon avec supervision et lifecycle**

Avec ça, ton lot “Foundations” devient vraiment **propre, scalable et codable sans rework majeur**.
