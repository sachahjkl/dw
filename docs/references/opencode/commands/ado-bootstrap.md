---
description: Bootstrap un sujet ADO et l'achemine etape par etape
agent: ado-orchestrator
subtask: true
---

Mode `BOOTSTRAP` sur le work item `$1`.

`$2` peut contenir un jalon de chaînage automatique parmi : `worktree`, `plan`, `exec`, `pr-plan`, `pr-open`.

Attendu :
1. Charger `business-workflow` puis `ado-workitem`.
2. Récupérer le work item, ses enfants utiles, et le contexte minimal nécessaire.
3. Déterminer le type du sujet, les tâches existantes ou manquantes, la famille probable (`ha` / `he`), et le périmètre probable (`front`, `back`, ou `multi-repo`).
4. Déterminer le nom de dossier sujet (`SubjectName`) et le nom de branche (`BranchName`) si les informations sont suffisantes.
5. Si le worktree du sujet est absent et que les informations sont suffisantes, le créer via `new-worktree.ps1`.
6. Ne pas faire l'analyse technique du code ici.
7. Produire un résumé court avec l'état détecté et la prochaine étape.
8. Si aucun jalon `$2` n'est fourni, poser une question Oui/Non pour continuer seulement vers l'étape suivante immédiate.
9. Si un jalon `$2` est fourni, enchaîner automatiquement étape par étape jusqu'à ce jalon, sans redemander de confirmation intermédiaire.

Règles de jalons :
- `worktree` : s'arrêter après création/vérification du worktree.
- `plan` : aller jusqu'à `/ado-plan`.
- `exec` : aller jusqu'à `/ado-exec`.
- `pr-plan` : aller jusqu'à `/ado-pr-plan`.
- `pr-open` : aller jusqu'à `/ado-pr-open`.

Règle de sécurité :
- même en chaînage automatique, ne pas sauter les questions réellement bloquantes.
