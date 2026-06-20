# CONTEXT

Ce fichier est un garde-fou global.

Il ne remplace pas les skills. Il rappelle uniquement les réflexes minimaux avant d'agir.

## Règle générale

Avant de proposer ou faire un changement :

1. Identifier le contexte : `ADO`, `HA front`, `HA back`, `HE front`, `HE back`, ou combinaison.
2. Charger le skill correspondant.
3. Lire les références détaillées seulement au moment utile.
4. Appliquer les conventions du skill chargé.

## Garde-fous globaux

1. Pour Angular, utiliser **pnpm**, jamais `npm`.
2. Dans ADO, les commits, les titres de PR et les noms métier : écrire en français sauf contrainte explicite contraire.
3. Si une demande touche ADO, Git, PR, worktree, hotfix, HA, HE ou le nommage, aller lire les skills avant d'agir.
4. En cas de doute, privilégier le skill comme source de vérité, pas le contexte global.

## PNPM dans un nouveau worktree front

Avant `pnpm typecheck`, `pnpm test`, `pnpm build` ou équivalent dans un nouveau worktree front Angular :

1. `pnpm install`
2. `pnpm approve-builds --all`

Symptôme si oublié :

```text
ERR_PNPM_IGNORED_BUILDS Ignored build scripts
```

## Routage

- Règles de routage haut niveau : `C:\Users\froment\.agents\OGF_WORKFLOW.md`
- Workflow transverse OGF : `C:\Users\froment\.agents\skills\ogf-workflow\SKILL.md`
- Règles ADO/Git/PR/worktree : `C:\Users\froment\.agents\skills\ado-workitem\SKILL.md`
- Conventions code HA/HE : skills `ha-*` et `he-*`

## Pipeline ADO recommandé

Pipeline par défaut pour traiter un sujet ADO :

1. `/ado-bootstrap <workItemId> [worktree|plan|exec|pr-plan|pr-open]`
2. `/ado-plan <workItemId>`
3. `/ado-exec`
4. `/ado-pr-plan`
5. `/ado-pr-open`

Règle de phase `PLAN` :

- faire une analyse technique du problème dans le code du worktree du sujet avant toute exécution
- identifier cause probable, périmètre, impacts et risques
- proposer ensuite seulement le plan ADO/Git d'exécution

Relais attendu entre étapes :

- après `/ado-bootstrap` -> exécuter `/ado-plan <workItemId>` ou continuer automatiquement jusqu'au jalon demandé
- après `/ado-plan` -> exécuter `/ado-exec`
- après `/ado-pr-plan` -> exécuter `/ado-pr-open`
- après `/worktree-new` -> exécuter `/ado-plan <workItemId>` ou `/ado-exec` selon le niveau de préparation
- après `/verify-conventions` -> exécuter `/commit-msg` ou `/pr-text` si ajustements texte, sinon `/ado-pr-plan`

Règle de chaînage :

- sans jalon explicite, chaque étape demande Oui/Non avant de continuer vers la suivante
- avec un jalon explicite (`worktree`, `plan`, `exec`, `pr-plan`, `pr-open`), l'agent enchaîne sans redemander jusqu'à ce jalon

Objectif : éviter les commandes monolithiques et garder un contexte court par étape.

## But

Éviter :

1. les mauvaises branches / commits / PR
2. les violations de conventions HA/HE
3. les prompts globaux trop lourds
4. la duplication de règles entre contexte, agents et skills
