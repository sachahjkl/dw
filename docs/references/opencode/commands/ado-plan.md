---
description: Planifie un sujet ADO sans effet de bord
agent: ado-orchestrator
subtask: true
---

Mode `PLAN` sur le work item `$1`.

Entrée recommandée : `/ado-bootstrap <workItemId>` si l'état du sujet n'est pas encore qualifié.

Précondition : un worktree du sujet existe déjà (créé via `/ado-bootstrap` ou `/worktree-new`).
Si absent, ne pas exécuter le plan et terminer par : `Étape suivante recommandée : /ado-bootstrap <workItemId>`.

Attendu :
1. Charger `business-workflow` puis `ado-workitem`.
2. Faire une vraie analyse du problème avant toute proposition d'exécution.
3. Lire seulement les références nécessaires pour qualifier le sujet.
4. Analyser le code concerné (fichiers, flux, cause probable, impacts, risques de régression).
   Cette analyse est strictement en lecture seule sur les fichiers du worktree.
5. Produire un diagnostic technique court : cause, périmètre, stratégie de correction, vérifications prévues.
6. Proposer ensuite : tâches à créer/réutiliser, dossier sujet, branches, états, Story Points si nécessaire.
7. Poser les questions bloquantes s'il manque une information.
8. Ne rien créer et ne rien modifier.
9. Si aucun chaînage automatique n'a été demandé, terminer par : `Étape suivante recommandée : /ado-exec` puis poser une question Oui/Non pour y aller.
10. Si le flux courant demande un jalon automatique `exec` ou au-delà, enchaîner vers `/ado-exec` sans redemander.

Si `$2` est fourni, utilise-le comme slug court proposé.
