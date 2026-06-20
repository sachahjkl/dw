---
description: Prépare la PR ADO sans l'ouvrir
agent: ado-orchestrator
subtask: true
---

Mode `PR_PLAN` pour la branche courante.

Attendu :
1. Vérifier la branche, le work item et les commits.
2. Charger `ado-workitem` puis les références utiles à la PR.
3. Préparer le titre, la description, la liste des reviewers à proposer, et l'état ADO cible.
4. Si besoin, déléguer le texte court à `ogf-text-ops`.
5. Ne rien pousser et ne pas ouvrir la PR.
6. Si aucun chaînage automatique n'a été demandé, terminer par : `Étape suivante recommandée : /ado-pr-open` puis poser une question Oui/Non.
7. Si le flux courant demande le jalon automatique `pr-open`, enchaîner vers `/ado-pr-open` sans redemander.
