---
description: Ouvre la PR du sujet courant apres validation
agent: ado-orchestrator
subtask: true
---

Mode `PR_OPEN` pour la branche courante.

Attendu :
1. Utiliser le plan PR validé.
2. Pousser la branche.
3. Ouvrir la PR.
4. Proposer les reviewers via MCP ADO et ajouter uniquement ceux choisis.
5. Passer le work item à `PR en attente` si applicable.
6. Produire un rapport final court avec la PR créée et les mises à jour d'état.
