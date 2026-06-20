# Nommage des tâches, activités et sujets techniques

## Objectif

Le titre d'un work item doit décrire une tranche de travail réelle, compréhensible par un humain sans lire tout le contexte du sprint.

Avant de nommer un élément, vérifier que le bon type de work item est utilisé.

Rappel rapide :

1. Une `User Story` ou une `Anomalie` doit être découpée en `Tâches` pour porter le travail concret.
2. Une `Activité` est réservée aux sujets techniques autonomes.
3. Pour la politique complète de choix entre `Tâche` et `Activité`, se référer à `SKILL.md`.

Un bon titre permet de comprendre rapidement :

1. le périmètre (`FRONT`, `BACK`, `CI`, `DEVTOOLS`, etc.)
2. la nature du travail
3. le résultat attendu ou le sujet traité

## Formats recommandés

### Tâche ou activité front

```text
[FRONT] Titre concret
```

Exemples :

```text
[FRONT] Rendre http-resource production ready
[FRONT] Migrer la recherche globale vers le nouveau contrat API
[FRONT] Ajouter la gestion de l'annulation sur les ressources HTTP
```

### Tâche ou activité back

```text
[BACK] Titre concret
```

Exemples :

```text
[BACK] Exposer l'endpoint de simulation PEGASE
[BACK] Corriger le mapping Dapper des événements RH
```

### Autres sujets techniques

```text
[CI] Titre concret
[DEVTOOLS] Titre concret
[TECH] Titre concret
```

Exemples :

```text
[CI] Stabiliser le pipeline de build Angular
[DEVTOOLS] Générer automatiquement les types OpenAPI
[TECH] Réduire le coût de calcul du cache des ressources HTTP
```

## Règles

1. Le titre doit être concret.
2. Le titre doit représenter un vrai travail réalisé.
3. Le titre ne doit pas être une phase abstraite.
4. Le titre doit être en français.
5. Le titre doit éviter les verbes vagues comme `gérer`, `faire`, `modifier` quand un verbe plus précis existe.
6. Le titre ne doit pas contenir de badge `[AI]` ou équivalent.
7. La traçabilité IA doit passer par un commentaire séparé sur le work item, pas par le titre.

## Titres interdits ou à éviter

Exemples à éviter :

```text
[FRONT] Vérification
[FRONT] Tests
[FRONT] Analyse
[FRONT] Modifs diverses
[FRONT] Ajustements UI
```

Pourquoi :

1. le travail n'est pas identifiable
2. le titre n'aide ni la PR ni le suivi de sprint
3. le lien entre code, commit et work item devient faible
