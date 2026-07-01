---
name: business-workflow
description: Routage et orchestration du workflow BUSINESS (ADO, Git, HA, HE, conventions de nommage, worktrees).
license: MIT
metadata:
  authors:
    - OpenCode
    - Sacha FROMENT
  version: '1.0'
---

# Workflow BUSINESS

Ce skill centralise les règles de routage entre les différents contextes métier et techniques de BUSINESS :

- Azure DevOps (work items, états, branches, commits, PR)
- Projets HA (front Angular / back .NET)
- Projets HE (front Angular / back .NET)
- Outils transverses (pnpm, worktrees, conventions)

## Quand utiliser ce skill

Charge ce skill dès qu'une demande implique :

1. Un work item Azure DevOps (US, Anomalie, Bug, Tâche, Activité).
2. Une action Git (branche, commit, PR, worktree, hotfix).
3. Un changement dans un projet HA ou HE (front ou back).
4. La création d'un nouveau worktree ou sujet de développement.

## Règles de routage

### 1. Dès qu'une action ADO/Git est détectée

- Charger le skill `ado-workitem`.
- Lire obligatoirement `ado-workitem/references/states.md`.
- Lire obligatoirement `ado-workitem/references/pr-rules.md`.

### 2. Détection du contexte technique

Identifier la famille de projet à partir du chemin du worktree courant ou des fichiers présents :

| Indices | Famille | Skills à charger |
|---|---|---|
| Chemin contient `ws/he`, `hommage-exploitation`, `Ogf.Exploitation.CentreServeur` | HE back | `he-back`, `dotnet`, `sqlserver` |
| Chemin contient `ws/he`, `front-hommage-exploitation`, `@ogf/exploitation-core-ui` | HE front | `he-front`, `angular`, `angular-developer`, `tooling-pnpm` |
| Chemin contient `ws/ha`, `Ogf.Gesco` | HA back | `ha-back`, `dotnet` |
| Chemin contient `ws/ha`, `src/app/gesco` | HA front | `ha-front`, `angular`, `angular-developer`, `tooling-pnpm` |
| Fichiers `.csproj`, `.sln` | .NET générique | `dotnet` |
| Fichiers `angular.json`, `package.json` avec `@angular/core` | Angular générique | `angular`, `angular-developer` |

### 3. Conventions non négociables

- Toujours utiliser **pnpm** pour les projets front Angular. Ne jamais utiliser `npm install`.
- Dans un nouveau worktree front, appliquer avant tout build/test :
  1. `pnpm install`
  2. `pnpm approve-builds --all`
- Écrire en **français** pour les titres ADO, les messages de commit et les titres de PR.
- Ne jamais créer de faux work items de confort (`analyse`, `tests`, `vérification`, `lecture`).
- Ne pas inventer de nom de branche, de commit ou de PR : ils dérivent du work item parent.

### 4. Frontière avec `ado-workitem`

`business-workflow` n'est pas la source de vérité des règles ADO détaillées.

Le skill `ado-workitem` porte les détails sur :

- structuration en tâches
- assignation parent + enfants
- Story Points
- nommage branche / commit / PR / worktree
- hotfix
- PR et reviewers
- traçabilité IA

Le rôle de `business-workflow` est de dire **quand** charger `ado-workitem` et **quand** charger les skills HA/HE.

### 5. Phases d'exécution

Commande d'entrée recommandée pour un nouveau sujet ADO :

1. `/ado-bootstrap <workItemId>`
2. `/ado-plan <workItemId>`
3. `/ado-exec`
4. `/ado-pr-plan`
5. `/ado-pr-open`

Rôle des phases :

- `BOOTSTRAP` : récupérer le contexte ADO minimal, qualifier le sujet, créer le worktree si possible, puis relayer
- `PLAN` : analyser techniquement le problème dans le code du worktree, sans modifier
- `EXECUTE` : créer/modifier/développer selon le plan validé
- `PR_PLAN` : préparer la PR sans l'ouvrir
- `PR_OPEN` : pousser, ouvrir la PR et mettre à jour ADO

Pour tout flux automatisé (`/ado-plan`, `/ado-exec`, `/ado-pr-plan`, `/ado-pr-open`) :

1. **Phase PLAN** : analyser, proposer, attendre validation explicite (`go`, `ok`, `c'est bon`, `valide`).
2. **Phase EXECUTE** : créer, modifier, pousser, ouvrir la PR.

Ne jamais passer directement à EXECUTE sans validation explicite de l'utilisateur, sauf commande `/ado-force-*` explicitement demandée.

Règle de chaînage :

1. Sans jalon explicite, chaque étape demande Oui/Non avant de continuer vers la suivante.
2. Avec un jalon explicite (`worktree`, `plan`, `exec`, `pr-plan`, `pr-open`), l'agent enchaîne sans redemander jusqu'à ce jalon.
3. Même avec un jalon, ne jamais sauter une vraie question bloquante.

### 6. Chargement progressif du contexte

Ordre recommandé :

1. Charger `business-workflow` seulement.
2. Si action ADO/Git, charger `ado-workitem`, puis lire seulement `states.md` et `pr-rules.md`.
3. Si besoin de nommage, charger `git-naming.md` et/ou `task-naming.md`.
4. Si besoin worktree, charger `worktree-scripts.md`.
5. Si besoin code, charger seulement le skill technique utile (`he-front`, `he-back`, `ha-front`, `ha-back`).

## Références

- `C:\Users\froment\.agents\CONTEXT.md`
- `ado-workitem/SKILL.md` et ses références `states.md`, `pr-rules.md`, `git-naming.md`, `task-naming.md`, `worktree-scripts.md`
- `he-front/SKILL.md`, `he-back/SKILL.md`, `ha-front/SKILL.md`, `ha-back/SKILL.md`
- `tooling-pnpm/SKILL.md`
