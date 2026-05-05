## Langue

- **Code** : anglais (variables, fonctions, classes, noms de fichiers)
- **Commentaires** : francais
- **Commits** : francais
- **Communication** : francais

## Principes de developpement

- **TDD** : Test Driven Development — ecrire les tests avant l'implementation
- **SOLID** : Single Responsibility, Open/Closed, Liskov Substitution, Interface Segregation, Dependency Inversion
- **DRY** : Don't Repeat Yourself
- **KISS** : Keep It Stupid Simple
- **YAGNI** : You Aren't Gonna Need It — pas de feature speculative

## Verification CSP (manuelle)

Apres chaque modification du chargement d'assets externes :

1. `cargo tauri dev`
2. Ouvrir DevTools (Ctrl+Shift+I sous Linux).
3. Onglet Console : aucune ligne « Refused to ... because it violates the following Content Security Policy directive ».
4. Onglet Network : toutes les requetes sont sur `tauri://localhost` ou `ipc://`.