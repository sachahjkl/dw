# Workflow BUSINESS pour OpenCode

Ce fichier complète `CONTEXT.md` avec un routage haut niveau.

## Réflexe de base

1. Identifier le contexte : `ADO`, `HA front`, `HA back`, `HE front`, `HE back`, ou combinaison.
2. Charger le bon skill avant d'agir.
3. Charger les références détaillées seulement au moment utile.

## Routage minimal

- Sujet ADO / Git / PR / worktree / hotfix -> charger `business-workflow`, puis `ado-workitem`.
- Projet HE -> charger `he-front` ou `he-back` selon le repo.
- Projet HA -> charger `ha-front` ou `ha-back` selon le repo.
- Front Angular -> utiliser pnpm, jamais npm.

## Phases

- `BOOTSTRAP` : recuperer le contexte ADO minimal, qualifier le sujet, creer le worktree si possible, puis relayer.
- `PLAN` : analyser techniquement le probleme dans le code du worktree du sujet (lecture seule), qualifier, poser les questions manquantes, ne rien modifier.
- `EXECUTE` : agir uniquement après validation explicite.

## Commandes cibles

- `/ado-bootstrap`
- `/ado-plan`
- `/ado-exec`
- `/ado-pr-plan`
- `/ado-pr-open`
- `/worktree-new`
- `/verify-conventions`
- `/commit-msg`
- `/pr-text`

## Enchainements recommandes

- Pipeline ADO par defaut :
  1. `/ado-bootstrap <workItemId> [worktree|plan|exec|pr-plan|pr-open]`
  2. `/ado-plan <workItemId>`
  3. `/ado-exec`
  4. `/ado-pr-plan`
  5. `/ado-pr-open`
- Relais explicites :
  - apres `/ado-bootstrap` -> `/ado-plan <workItemId>` ou continuation automatique jusqu'au jalon demande
  - apres `/ado-plan` -> `/ado-exec`
  - apres `/ado-pr-plan` -> `/ado-pr-open`
  - apres `/worktree-new` -> `/ado-plan <workItemId>` ou `/ado-exec` selon le niveau de preparation
  - apres `/verify-conventions` -> `/commit-msg` ou `/pr-text` si ajustements texte, sinon `/ado-pr-plan`

Regle de chainage :

- sans jalon explicite, chaque etape demande Oui/Non avant de continuer
- avec un jalon explicite (`worktree`, `plan`, `exec`, `pr-plan`, `pr-open`), l'agent enchaine sans redemander jusqu'a ce jalon
