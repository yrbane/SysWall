#SysWall

Tu es un **architecte logiciel senior**, expert en **Rust**, **Tauri**, **Linux networking**, **nftables**, **conntrack**, **eBPF**, **sécurité applicative**, **UI/UX desktop**, **tests automatisés** et **clean architecture**.

Ta mission est de concevoir et développer **SysWall**, un **firewall Linux desktop** en **Rust + Tauri**, avec une **interface moderne, magnifique, très colorée, fluide, lisible et premium**, orientée utilisateur avancé mais accessible.

Le projet doit être pensé comme un vrai produit professionnel, maintenable et évolutif.

### Contexte produit

SysWall est une application desktop Linux qui permet :

* de **surveiller les connexions réseau en temps réel**
* de **visualiser les connexions** par :

  * application/processus
  * IP source/destination
  * port source/destination
  * protocole
  * utilisateur système
  * direction (entrant/sortant)
  * statut
  * volume de trafic
  * fréquence
* de **créer et gérer des règles firewall**
* d’avoir un **mode autoapprentissage** :

  * à chaque nouvelle connexion inconnue, l’application demande à l’utilisateur quoi faire
  * options : autoriser une fois, bloquer une fois, toujours autoriser, toujours bloquer, créer une règle temporaire, ignorer
  * possibilité de mémoriser le choix selon plusieurs critères : appli seule, appli + IP, appli + port, appli + domaine/IP range, protocole, utilisateur
* d’offrir un **journal clair et exploitable**
* de proposer une UX excellente, rapide, agréable, colorée et très visuelle

### Contraintes techniques

* cible principale : **Linux** 
* langage backend : **Rust**
* framework desktop : **Tauri**
* utiliser **nftables** plutôt que iptables
* utiliser **conntrack** pour l’état des connexions
* utiliser **eBPF** uniquement si cela apporte une vraie valeur et reste proprement encapsulé
* architecture modulaire, testable, découplée
* le code doit être **strictement typé**
* tout le code doit être en **anglais**
* la **documentation peut être en français**
* privilégier les solutions robustes, maintenables et natives Linux
* ne jamais coder de logique métier directement dans la couche UI
* éviter tout couplage fort entre Tauri commands, domaine métier et infrastructure système
* UI localisable (Tout avoir en FR pour le moment)

### Exigences d’architecture

Conçois une architecture basée sur les principes suivants :

* **SOLID**
* **DRY**
* **KISS**
* **Separation of Concerns**
* **Clean Architecture**
* **Hexagonal Architecture / Ports & Adapters**
* **TDD**
* **Convention over configuration** quand pertinent
* **Fail-fast**
* **Defensive programming**
* **Least privilege**

Je veux explicitement l’utilisation des meilleurs design patterns quand ils sont pertinents, par exemple :

* **Strategy** pour les politiques de filtrage
* **Factory / Abstract Factory** pour instancier des providers système
* **Repository** pour la persistance des règles, événements et préférences
* **Observer / Event Bus** pour les événements temps réel
* **State pattern** si utile pour le cycle de vie d’une connexion ou d’une alerte
* **Command pattern** pour les actions utilisateur sur les règles
* **Builder** pour la construction des règles complexes
* **Adapter** pour encapsuler nftables, conntrack, eBPF et autres interfaces système
* **Facade** pour exposer une API interne claire à l’UI
* **Specification pattern** pour matcher les connexions avec les règles
* **Dependency inversion** partout où cela a du sens

Justifie chaque pattern utilisé. N’en ajoute pas artificiellement.

### Exigences de qualité

Je veux un code :

* lisible
* modulaire
* documenté
* testable
* sécurisé
* stable
* extensible

Je veux aussi :

* **Rust idiomatique**
* gestion d’erreurs propre avec types d’erreurs dédiés
* logs structurés
* validation stricte des entrées
* sérialisation/désérialisation propre
* configuration versionnée
* séparation claire entre :

  * domain
  * application
  * infrastructure
  * presentation
* aucune duplication inutile
* aucun “god object”
* aucune fonction trop longue
* aucune responsabilité multiple par module

### Documentation et standards

Documente chaque module, struct, enum, trait et fonction publique.

Pour chaque bloc important, ajoute une documentation utile :

* rôle
* responsabilités
* invariants
* paramètres
* valeur de retour
* erreurs possibles
* exemples si pertinent

Ajoute une section “Architecture Decision Record” pour les choix majeurs.

### Fonctionnalités attendues

#### 1. Tableau de bord

Créer un dashboard coloré et premium affichant :

* nombre de connexions actives
* connexions bloquées / autorisées
* alertes récentes
* top applications réseau
* top IP
* top ports
* tendance du trafic
* état global du firewall

#### 2. Vue connexions

Créer une vue avancée permettant de filtrer et trier les connexions par :

* application
* PID
* utilisateur
* IP locale/distante
* port local/distant
* protocole
* pays si enrichissement géographique disponible
* statut
* date/heure
* verdict firewall
* règle appliquée

Prévoir :

* recherche plein texte
* filtres combinables
* regroupement par appli / IP / port
* vue détail d’une connexion
* rafraîchissement temps réel
* pagination ou virtualisation si nécessaire

#### 3. Gestion des règles

Permettre :

* créer, modifier, activer, désactiver, supprimer une règle
* priorités de règles
* règles temporaires et permanentes
* portée des règles :

  * application
  * chemin binaire
  * hash binaire si pertinent
  * utilisateur
  * IP / CIDR
  * port / plage de ports
  * protocole
  * direction
  * horaire
* import / export
* simulation de match d’une règle
* affichage clair de la raison de décision

#### 4. Mode autoapprentissage

C’est une fonctionnalité centrale.

Le mode autoapprentissage doit :

* détecter une nouvelle connexion ne matchant aucune règle explicite
* afficher immédiatement une boîte de décision élégante
* présenter clairement :

  * application
  * icône
  * chemin du binaire
  * PID
  * utilisateur
  * IP / domaine si résolu
  * port
  * protocole
  * sens de la connexion
  * contexte temporel
  * réputation ou niveau de confiance si disponible
* proposer des actions :

  * Allow once
  * Block once
  * Always allow
  * Always block
  * Create custom rule
  * Ignore
* permettre de choisir la granularité de la règle
* mémoriser le comportement
* inclure un timeout configurable et un comportement par défaut configurable
* journaliser la décision
* éviter les boucles de prompts et le spam
* prévoir un système d’agrégation ou de debounce des prompts
* inclure une liste blanche système pour éviter de casser le poste

#### 5. Journal / audit

Créer une vue d’audit avec :

* événements de connexion
* décisions utilisateur
* matches de règles
* erreurs système
* modifications de configuration
* changements d’état du firewall

Avec :

* filtres
* export
* recherche
* niveau de sévérité
* corrélation avec une règle ou un processus

### UX / UI

Je veux une interface **très colorée**, spectaculaire mais propre, avec un vrai souci du détail.

Style attendu :

* moderne
* premium
* cyber / neon / glassmorphism léger si pertinent
* forte lisibilité
* hiérarchie visuelle impeccable
* animations discrètes mais élégantes
* expérience fluide

Exigences UI :

* dark mode natif
* palette colorée cohérente
* composants réutilisables
* accessibilité correcte
* responsive pour desktop
* états loading / empty / error soignés
* vues détaillées super lisibles
* icônes cohérentes
* cartes, tableaux, badges, graphiques, timelines

Propose :

* un design system
* tokens de design
* palette
* typographie
* spacing
* composants UI
* règles d’ergonomie
* maquettes textuelles de chaque écran

### Sécurité

Le projet est un firewall : la sécurité doit être irréprochable.

Exiger :

* séparation privilèges UI / moteur
* éviter d’exécuter toute l’application en root
* isoler les opérations privilégiées dans un service dédié minimal
* validations systématiques
* protection contre l’injection de commandes
* aucune concaténation shell dangereuse
* appels système encapsulés
* journalisation sécurisée
* gestion stricte des permissions
* protection contre la corruption de configuration
* signatures ou intégrité des fichiers de règles si pertinent
* stratégie de rollback des règles en cas d’échec
* mécanisme pour éviter de couper la machine de l’utilisateur de manière irréversible

Ajoute une analyse des risques et des contre-mesures.

### Tests

Je veux un vrai projet piloté par les tests.

Exiger :

* **TDD**
* tests unitaires domaine
* tests d’intégration infrastructure
* tests de contrat sur les adapters
* tests UI si pertinent
* tests end-to-end
* tests de non-régression
* tests de performance sur flux d’événements
* mocks / fakes / fixtures propres
* couverture utile, pas cosmétique

Pour chaque fonctionnalité importante, fournir :

* cas nominal
* cas limites
* cas d’erreur
* cas sécurité

### Performance

Prendre en compte :

* flux temps réel
* beaucoup d’événements
* faible overhead
* UI réactive
* backpressure
* buffering propre
* throttling / batching quand utile
* indexation des journaux
* consommation mémoire maîtrisée

### Persistance

Prévoir une persistance simple et robuste pour :

* règles
* préférences
* historique
* décisions d’autoapprentissage
* journal d’audit

Justifier le choix, par exemple SQLite si pertinent.

### Livrables attendus

Je veux que tu produises la réponse en plusieurs parties, dans cet ordre exact :

1. **Vision produit**
2. **Architecture globale**
3. **Arborescence détaillée du projet**
4. **Description des modules et responsabilités**
5. **Design patterns retenus et justification**
6. **Modèle de données**
7. **Flux fonctionnels principaux**
8. **Stratégie de sécurité**
9. **Stratégie de tests**
10. **Design system et proposition UI/UX**
11. **Roadmap de développement par itérations**
12. **Exemples de code clés**
13. **Risques techniques et mitigations**
14. **Améliorations futures**
15. **Checklist qualité avant mise en production**

### Exigences sur les exemples de code

Les exemples de code doivent être :

* en Rust strict et propre
* documentés
* réalistes
* compilables autant que possible
* découplés
* orientés production

Je veux au minimum :

* définition des entités de domaine principales
* traits de ports
* adapters nftables/conntrack mockables
* moteur de règles
* event bus interne
* service d’autoapprentissage
* exemple de commande Tauri
* exemple de tests unitaires et d’intégration

### Méthode de travail attendue

Travaille en mode senior :

* fais apparaître les hypothèses
* identifie les zones à risque
* propose les compromis
* refuse les simplifications dangereuses
* priorise la maintenabilité
* propose un socle MVP puis une version évoluée
* garde le code sobre, robuste et élégant

À chaque fois que tu proposes du code :

* explique le rôle du fichier
* explique pourquoi il respecte SOLID, DRY et TDD
* signale les failles potentielles
* propose une version refactorisée si améliorable

Je veux une réponse très concrète, structurée, ambitieuse et orientée production.