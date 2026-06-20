---
name: ado-workitem
description: Guide Azure DevOps pour work items, branches, commits, PR, états et hygiène de travail.
license: MIT
metadata:
  authors:
    - OpenCode
    - Sacha FROMENT
  version: '2.1'
---

# Guide ADO Work Items

Ce skill est le point d'entrée pour tout flux impliquant Azure DevOps dans ce contexte : work items, états, branches, commits, PR, reviewers et hygiène de traçabilité.

Le rôle de ce fichier est volontairement simple :

1. dire quand utiliser le skill
2. rappeler les règles transverses non négociables
3. orienter vers la bonne référence selon le besoin

Le détail des conventions vit dans les fichiers `references/`.

## Quand utiliser ce skill

Utilise ce skill dès qu'au moins un des cas suivants apparaît :

1. Le travail part d'une US, d'une anomalie, d'un bug, d'une tâche ou d'une activité ADO.
2. Il faut créer un work item ADO pour tracer un changement.
3. Il faut nommer une branche, un commit ou une PR à partir d'identifiants ADO.
4. Il faut ouvrir une PR et la lier au bon work item.
5. Il faut mettre à jour l'état d'un work item.

## Règle obligatoire

Dès qu'une demande implique un work item Azure DevOps ou une pull request, lire systématiquement :
- `references/states.md`
- `references/pr-rules.md`

Même si aucun changement d'état n'est explicitement demandé, ces références doivent être sourcées avant d'exécuter le flux.

NE PAS ESSAYER D'UTILISER LE CLI `gh`, IL N'EST **PAS** INSTALLÉ !

## Règle de préfixe commit et PR

Le préfixe du commit principal et le titre de PR doivent refléter le type de work item parent.

Règles :

1. Une `User Story` utilise `feat(...)`.
2. Une `Anomalie` utilise `fix(...)`.
3. Un `Bug` peut utiliser `bug(...)` s'il porte directement le changement.
4. Une `Activité` utilise le préfixe adapté au sujet réel, par exemple `chore(...)`, `feat(...)` ou `fix(...)`.
5. Si le travail concret est porté par une `Tâche` enfant, cette règle reste pilotée par le parent métier attendu dans le flux d'équipe.

## Règle de structuration du travail

Pour une `User Story` ou une `Anomalie`, le travail concret doit être porté par une ou plusieurs `Tâches`.

Le principe est le même pour une anomalie que pour une US :

1. on découpe le travail réel en tâches concrètes
2. on en crée souvent moins que pour une US, mais on ne développe pas directement l'anomalie si aucune tâche adaptée n'existe
3. chaque tâche créée doit ensuite suivre le workflow habituel de l'équipe : assignation, passage à l'état actif attendu, branche, commit, PR

Règles :

1. Une `User Story` ou une `Anomalie` ne doit pas être traitée directement en code sans vérifier d'abord ses tâches enfants.
2. Si des tâches enfants pertinentes existent déjà, utiliser ces tâches comme support du travail.
3. Si aucune tâche adaptée n'existe, créer les tâches enfants nécessaires avant de développer.
4. Le découpage doit refléter le travail réel : `FRONT`, `BACK`, ou plusieurs tâches si plusieurs chantiers distincts existent réellement.
5. Ne crée pas de tâches artificielles de phase comme `lecture`, `analyse`, `vérification` ou `tests`.
6. Un `Bug` peut être traité sans tâche enfant si le bug lui-même porte directement le changement.

## Ordre de création

Quand une branche, un worktree ou une PR doivent être créés à partir d'une `User Story` ou d'une `Anomalie` :

1. Vérifier d'abord les tâches enfants existantes.
2. Si nécessaire, créer les tâches enfants **avant** le worktree et la branche.
3. Utiliser les IDs de tâches pour nommer la branche réelle.
4. Ne jamais créer une branche finale avant de connaître l'ID du work item concret qui porte le changement.

## Usage des activités

Les `Activités` sont réservées aux sujets techniques qui ne relèvent pas directement d'une `User Story`, d'une `Anomalie` ou d'un `Bug` métier.

Exemples typiques : `chore`, `perf`, `refactor`, `style`, `ci`, `devtools`, maintenance technique ponctuelle.

Règles :

1. Ne crée pas d'`Activité` pour porter un morceau concret d'une `User Story` ou d'une `Anomalie` si une `Tâche` enfant est le bon support.
2. Utilise une `Activité` quand le sujet est purement technique et n'a pas de support métier naturel.
3. Une `Activité` doit rester une vraie tranche de travail technique, pas un fourre-tout.

## Hygiène ADO

1. Ne crée pas de faux work items de confort du type `lecture`, `analyse`, `vérification`, `tests`.
2. Une activité technique ponctuelle doit être assignée immédiatement à l'utilisateur Git local.
3. L'utilisateur à utiliser doit être lu depuis `git config --global user.name`.
4. Dans cet environnement, la valeur observée est `Sacha FROMENT`, mais il faut toujours relire la vraie valeur au moment du besoin.
5. Quand le travail est poussé, l'agent doit ouvrir une PR. Si c'est pertinent, il peut l'ouvrir plus tôt en draft.
6. En cas de sujet multi-repo `FRONT` + `BACK`, créer des tâches enfants distinctes et suivre chaque repo avec sa propre branche, ses propres commits et sa propre PR.
7. Si une PR a été ouverte contre la mauvaise branche cible, l'abandonner explicitement puis ouvrir la bonne PR. Ne laisse pas deux PR actives pour le même report si l'une est obsolète.
8. Quand l'agent crée une `Tâche`, il doit l'assigner immédiatement à l'utilisateur demandé et la passer dans l'état actif attendu par l'équipe.
9. Quand l'agent crée un work item ADO (`Tâche`, `Activité`, etc.), il ne doit pas ajouter de badge `[AI]` dans le titre.
10. À la place, l'agent doit ajouter un commentaire de traçabilité sur le work item juste après sa création, au format `Créé par IA <model> <tool>`.
11. Exemple : `Créé par IA github-copilot/gpt-5.4 opencode`.
12. Dans ce contexte, pour les tâches créées par l'agent, l'état attendu par défaut est `En développement` sauf indication contraire explicite de l'utilisateur ou contrainte du board.
13. Quand un flux est porté par une `User Story`, une `Anomalie` ou un `Bug`, l'agent doit assigner **à la fois** :
    - le work item parent ;
    - le ou les work items enfants concrets.
14. Avant de passer une `User Story` ou une `Anomalie` à `En réalisation`, l'agent doit demander explicitement la valeur de `Story Points`.
15. Propositions par défaut pour la question : `1`, `2`, `3`, `5`, `autre`.

## Conventions de rédaction

1. Tout ce qui est écrit dans ADO doit être en français, sauf contrainte explicite contraire.
2. Les commentaires dans le code doivent être en français, sauf si le fichier est déjà majoritairement documenté en anglais.
3. La réflexion doit rester concise et directe. Le skill doit aider à décider vite, pas à produire du bruit.

## Références

Références à lire systématiquement pour tout flux ADO :

- `references/states.md`
- `references/pr-rules.md`

Références à lire selon le besoin courant :

- `references/task-naming.md`
- `references/git-naming.md`
- `references/worktree-scripts.md`

### États ADO

Lire `references/states.md` pour :

1. savoir quel état est valide selon le type de work item
2. choisir l'état de départ
3. savoir quand passer à `PR en attente`

### Titres de tâches et activités

Lire `references/task-naming.md` pour :

1. nommer correctement les tâches et activités
2. choisir le bon préfixe (`FRONT`, `BACK`, `CI`, `DEVTOOLS`, etc.)
3. éviter les intitulés trop vagues

### Nommage Git

Lire `references/git-naming.md` pour :

1. nommer les branches
2. formater les commits
3. formater les titres de PR
4. nommer les dossiers parents de worktrees par sujet, puis utiliser `front` et `back` comme sous-dossiers repo

Important : le **dossier sujet** et la **branche Git** n'ont pas le même format.

- Dossier sujet (`SubjectName`) : `type-id-slug`
- Branche Git (`BranchName`) : `type/id-task-slug`

Exemple :

- dossier : `fix-55044-heures-psfs`
- branche : `fix/55044-55201-heures-psfs`

### PR et lien ADO

Lire `references/pr-rules.md` pour :

1. structurer les descriptions de PR
2. lier correctement la PR aux work items
3. ajouter les reviewers
4. abandonner les PR obsolètes ou intermédiaires

### Scripts de worktree

Lire `references/worktree-scripts.md` pour :

1. initialiser les anchors bare
2. créer un nouveau sujet `front` / `back`
3. supprimer proprement un sujet
4. relancer une migration de worktrees existants

Le script `new-worktree.ps1` doit être utilisé avec :

- `-SubjectName` pour le nom du dossier sujet
- `-BranchName` pour le nom de branche réel
- `-Only` pour cibler les repos à créer (`front`, `back`, etc.)

## Workflow multi-repo et hotfix

Quand un même sujet touche plusieurs dépôts, appliquer ces règles :

1. Créer une tâche `FRONT` et une tâche `BACK` si le parent est une `User Story` ou une `Anomalie` et qu'aucune tâche enfant adaptée n'existe.
2. Utiliser un nom de branche cohérent entre les dépôts quand c'est le même sujet, même si les commits et PR restent distincts.
3. Regrouper les worktrees du même sujet sous un dossier parent unique dérivé de la branche réelle, au format `type-id-slug`, puis créer `front` et `back` dedans selon les dépôts concernés.
4. Si un sujet ne touche qu'un seul dépôt, créer quand même le dossier parent sujet et n'y mettre que `front` ou `back`.
5. Ouvrir une PR par dépôt, avec les bons work items liés.

Quand la demande implique une branche `hotfix/<date>` :

1. La branche `hotfix/<date>` est une branche d'intégration commune, pas la branche source de la PR de l'agent.
2. La branche `hotfix/<date>` doit partir de la branche de référence du dépôt : souvent `main` pour le front, souvent `master` pour le back. Toujours vérifier la vraie branche par défaut du dépôt avant d'agir.
3. La branche source de l'agent doit être une branche séparée, par exemple `fix/...-on-hotfix-YYYY-MM-DD`.
4. La PR correcte va de la branche source de l'agent vers `hotfix/<date>`.
5. La branche `hotfix/<date>` sera ensuite fusionnée plus tard vers `main` ou `master` par le flux collectif. Ne pas ouvrir par défaut une PR `hotfix/<date>` vers la branche par défaut si la demande n'est pas explicite.

Quand il faut réaligner une branche `hotfix/<date>` créée à tort comme branche source :

1. Sauvegarder d'abord le contenu existant sur une branche source dédiée.
2. Réaligner ensuite `hotfix/<date>` sur sa vraie base (`main` ou `master`).
3. Forcer la mise à jour distante seulement si c'est explicitement nécessaire pour remettre la branche d'intégration à plat et que le besoin utilisateur est clair.
4. Ouvrir ensuite la vraie PR depuis la branche source dédiée vers `hotfix/<date>`.

Quand un report hotfix se fait par `cherry-pick` :

1. Appliquer les commits demandés dans l'ordre explicite fourni par l'utilisateur.
2. Si un commit technique intermédiaire embarque un ajustement de config non souhaité pour le hotfix, retirer cet ajustement sur la branche source hotfix avant ouverture de la PR.
3. Sur conflit, privilégier une résolution minimale qui reporte seulement le correctif utile au lieu d'écraser un fichier entier avec la version d'une autre branche.

## Workflow recommandé

### Cas 1 : sujet déjà rattaché à ADO

1. Identifier le bon work item.
2. Lire les références obligatoires, puis les références complémentaires utiles.
3. Si le sujet est une `User Story` ou une `Anomalie`, vérifier d'abord les tâches enfants existantes.
4. Si une `Tâche` enfant pertinente existe, elle devient le support principal du développement, du commit et de la PR.
5. Si aucune tâche adaptée n'existe, créer les tâches enfants nécessaires avant de développer.
6. Si l'agent crée une `Tâche`, l'assigner immédiatement à l'utilisateur demandé puis la passer à `En développement`, sauf si le board ou l'utilisateur impose un autre état.
7. Si l'agent crée un work item, ajouter immédiatement le commentaire de traçabilité `Créé par IA <model> <tool>`.
8. Assigner le parent métier et l'élément de travail concret à l'utilisateur Git local.
9. Si le parent est une `User Story` ou une `Anomalie` et qu'il doit passer à `En réalisation`, demander d'abord la valeur de `Story Points`.
10. Positionner les états corrects.
11. Créer le worktree et la branche selon `git-naming.md` et `worktree-scripts.md`.
12. Développer.
13. Committer avec les IDs ADO.
14. Pousser.
15. Ouvrir la PR, la lier au work item, ajouter les reviewers, puis faire les mises à jour d'état nécessaires.

### Cas 2 : sujet technique ponctuel sans support ADO clair

1. Créer une `Activité` si le changement mérite une traçabilité.
2. Nommer l'activité via `task-naming.md`.
3. Ajouter immédiatement le commentaire de traçabilité `Créé par IA <model> <tool>` si l'agent a créé le work item.
4. L'assigner à l'utilisateur Git local.
5. Positionner l'état actif approprié.
6. Créer la branche selon `git-naming.md`.
7. Développer, committer avec `#<id>`, pousser, puis ouvrir la PR liée à l'activité.
