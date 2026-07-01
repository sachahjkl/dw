---
description: Cree un nouveau worktree BUSINESS via new-worktree.ps1
agent: build
---

Crée un nouveau worktree pour la famille `$1` avec le sujet `$2`.

Instructions :
1. Vérifie que tu es dans le workspace `S:\ai-agent-workdir\ws\$1` ou un de ses sous-dossiers.
2. Détermine le nom du dossier sujet (`SubjectName`) et le nom de la branche (`BranchName`) selon `git-naming.md` :
   - Dossier : `type-id-slug` (ex: `feat-53847-mon-sujet`)
   - Branche : `type/id-task-slug` (ex: `feat/53847-54200-mon-sujet`)
3. Exécute le script `C:\Users\froment\.agents\skills\ado-workitem\references\worktree-tools\new-worktree.ps1` avec les arguments :
   - `-Family $1`
   - `-SubjectName <dossier>`
   - `-BranchName <branche>`
   - `-WorkspaceRoot S:\ai-agent-workdir` si nécessaire
4. Si `$3` est fourni, transmets `-Only $3` (par exemple `front`, `back`, ou une liste de clés de repo).
   Les anciens switches `-FrontOnly` et `-BackOnly` restent supportes comme alias.
5. Termine par un relais explicite :
   - si le sujet est ADO et non planifié : `Étape suivante recommandée : /ado-plan <workItemId>`
   - si le plan ADO est déjà validé : `Étape suivante recommandée : /ado-exec`
   - si le sujet vient d'un bootstrap : proposer une question Oui/Non pour continuer vers `/ado-plan <workItemId>`
