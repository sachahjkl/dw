---
description: Verifie la conformite branche / commit / PR en cours
agent: ogf-code-reviewer
subtask: true
---

Revue read-only du sujet courant.

Attendu :
1. Charger `ogf-workflow`.
2. Charger `ado-workitem` si nécessaire.
3. Charger seulement le skill technique utile selon le repo courant.
4. Produire un rapport de conformité sans rien modifier.
5. Terminer par un relais explicite :
   - si des corrections de texte sont nécessaires : `Étape suivante recommandée : /commit-msg` ou `/pr-text`
   - si les conventions sont OK pour une PR : `Étape suivante recommandée : /ado-pr-plan`
