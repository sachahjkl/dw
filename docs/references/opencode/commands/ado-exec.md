---
description: Execute le plan ADO valide pour le sujet en cours
agent: ado-orchestrator
subtask: true
---

Mode `EXECUTE` pour le sujet ADO en cours.

Attendu :
1. Appliquer le dernier plan validé.
2. Créer les tâches avant worktree et branches.
3. Utiliser `new-worktree.ps1 -SubjectName <dossier> -BranchName <branche>`.
4. Assigner parent et enfants.
5. Positionner les états après questions obligatoires (Story Points si nécessaires).
6. Réaliser le développement prévu par le plan validé.
7. Produire un rapport factuel des changements et vérifications effectuées.
8. Si aucun chaînage automatique n'a été demandé, terminer par une proposition de suite : `/verify-conventions`, `/commit-msg`, puis `/ado-pr-plan`.
9. Si le flux courant demande un jalon automatique `pr-plan` ou `pr-open`, enchaîner sans redemander vers les étapes suivantes.

Ne pas refaire une planification longue si le plan vient d'être validé.
Ne pas refaire l'analyse technique de cadrage : elle appartient à `PLAN`.
