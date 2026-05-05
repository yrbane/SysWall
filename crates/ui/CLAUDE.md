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

## Validation visuelle (sous-projet E)

Apres modification du design system, verifier :
1. Tous les emojis sidebar sont remplaces par des icones Lucide.
2. Police Inter Variable chargee (DevTools > Network > woff2). Pas de FOIT/FOUT visible.
3. Logo SysWall affiche en topbar et favicon visible dans l'onglet du navigateur.
4. Pilule killswitch pulse subtilement quand le reseau est actif.
5. Hover sur StatCard => leger lift + ombre.
6. Hover sur ligne de tableau => fond plus marque qu'avant.
7. Zebra-striping subtil mais perceptible sur tableaux denses (audit, connexions).
8. `prefers-reduced-motion: reduce` (DevTools > Rendering) => pulsation desactivee.
9. Input avec prop error => bordure rouge + helper text.