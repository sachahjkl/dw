# Nommage Git : branches, commits et titres de PR

## Principe général

Une branche, un commit, un titre de PR et un work item doivent raconter la même histoire.

Quand quelqu'un lit :

1. le nom de branche
2. le commit principal
3. le titre de PR
4. le work item lié

il doit comprendre immédiatement qu'il s'agit du même sujet.

## Format de commit

### Cas standard US + tâche

```text
feat(#USNUMBER #TASKNUMBER): description en français
```

Pour une `User Story`, utiliser `feat(...)`.

Exemple :

```text
feat(#25891 #50481): envoyer order.Number au lieu de order.Id
```

### Cas standard anomalie + tâche

```text
fix(#ANOMALIENUMBER #TASKNUMBER): description en français
```

Exemples :

```text
fix(#53115 #53312): corriger le calcul des créneaux sur le planning RH
```

Pour une `Anomalie`, utiliser `fix(...)`.

Cette forme avec tâche enfant reste la forme attendue par défaut pour une `User Story` ou une `Anomalie`.

### Cas bug sans tâche enfant

```text
bug(#BUGNUMBER): description en français
```

Exemple :

```text
bug(#53020): corriger l'ouverture d'un dossier depuis la recherche
```

### Cas activité technique

```text
chore(#ACTIVITENUMBER): description en français
feat(#ACTIVITENUMBER): description en français
fix(#ACTIVITENUMBER): description en français
```

Exemple :

```text
feat(#53443): rendre http-resource production ready
```

## Règles de commit

1. Le message doit toujours inclure `#<workItemId>` pour créer le lien ADO.
2. Le texte après `:` doit être en français.
3. Le texte après `:` doit décrire un résultat, une correction ou une intention utile, pas juste `modif` ou `ajustements`.
4. Si un seul work item porte réellement le changement, ne surcharge pas le message avec d'autres IDs.
5. Si le travail concerne une US et une tâche enfant, inclure les deux IDs est la forme privilégiée.
6. Une `User Story` utilise `feat(...)`.
7. Une `Anomalie` utilise `fix(...)`.
8. Pour une US ou une anomalie, ne retomber sur une forme sans tâche enfant que si l'exception est réellement justifiée.

## Format de branche

Formats autorisés et cohérents avec les usages observés :

```text
feat/USNUMBER
feat/USNUMBER-TASKNUMBER-description-courte-francais
fix/USNUMBER-TASKNUMBER-description-courte-francais
bug/BUGNUMBER-description-courte-francais
hotfix/YYYY-MM-DD
fix/USNUMBER-TASKNUMBER-description-courte-francais-on-hotfix-YYYY-MM-DD
```

Exemples :

```text
feat/53443
feat/25891-50481-validation-commande-servicebus
fix/53115-53312-correction-creneaux-planning-rh
bug/53020-ouverture-dossier-recherche
hotfix/2026-04-22
fix/53498-53521-53522-transmission-tld-on-hotfix-2026-04-22
```

## Règles de branche

1. Préfère la convention déjà visible dans le dépôt.
2. Si le dépôt utilise déjà une forme courte `feat/<id>`, elle est acceptable.
3. Si une description est ajoutée, elle doit être courte, en français simplifié sans accents, avec des tirets.
4. Si le sujet est porté par une activité seule, le numéro de l'activité peut suffire.
5. Si le sujet est un bug sans tâche enfant, utilise le numéro du bug.
6. Ne mélange pas plusieurs conventions sur la même branche.
7. Une branche `hotfix/YYYY-MM-DD` désigne une branche d'intégration partagée, pas forcément la branche source de la PR de l'agent.
8. Pour une contribution sur hotfix, préférer une branche source dédiée de type `fix/...-on-hotfix-YYYY-MM-DD`, puis ouvrir la PR vers `hotfix/YYYY-MM-DD`.

## Nommage des worktrees

Le dossier parent du sujet doit rester lisible au premier coup d'oeil.

Format recommandé :

```text
<type>-<workItemId>-<sujet-court>\front
<type>-<workItemId>-<sujet-court>\back
```

Exemples :

```text
fix-53635-reprendre-numero-he-tsc-pre-reservation\back
feat-53847-type-convoi-tooltip-planning-rh\front
bug-53279-documents-tableau-bord\back
```

Règles :

1. Le dossier parent doit être dérivé de la branche réellement checkoutée, pas d'un ancien nom de dossier obsolète.
2. Le parent suit le format `type-id-slug`, où `type` reflète la branche : `feat`, `fix`, `bug`, `chore`, `perf`, `release`, `hotfix`, `dev`, etc.
3. Ajoute systématiquement une chaîne sujet après l'identifiant principal pour éviter les worktrees ambigus.
4. Le sujet doit être court, compréhensible, en français simplifié sans accents, avec des tirets.
5. Les sous-dossiers repo sont fixes : `front` et `back`.
6. Si le sujet ne concerne qu'un dépôt, créer quand même le parent sujet et n'y mettre que le sous-dossier repo nécessaire.
7. Si le worktree sert à un report, tu peux suffixer proprement le contexte dans le slug parent, par exemple `-on-release-2-17` ou `-on-hotfix-2026-04-22`.
8. Ne renomme pas un linked worktree en déplaçant le dossier à la main ; utiliser `git worktree move`.

## Format de titre de PR

```text
feat(#USNUMBER #TASKNUMBER): résumé général pertinent de la PR
fix(#ANOMALIENUMBER #TASKNUMBER): résumé général pertinent de la PR
bug(#BUGNUMBER): résumé général pertinent de la PR
chore(#ACTIVITENUMBER): résumé général pertinent de la PR
```

Exemples :

```text
feat(#25891 #50481): fiabiliser l'envoi des commandes vers Service Bus
feat(#53443): rendre http-resource production ready
```

## Règles de titre de PR

1. Le titre de PR doit résumer le changement global, pas un détail de fichier.
2. Le titre doit rester cohérent avec le commit principal.
3. Le titre doit être en français.
4. Une PR issue d'une `User Story` utilise `feat(...)`.
5. Une PR issue d'une `Anomalie` utilise `fix(...)`.
