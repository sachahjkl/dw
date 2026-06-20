# PR et lien ADO

## Principe général

Une PR doit être le prolongement naturel du work item et des commits.

Le nommage Git détaillé est documenté dans `git-naming.md`.

Ici, on se concentre sur :

1. l'ouverture de PR
2. le lien avec les work items ADO
3. les reviewers
4. l'hygiène de remplacement d'anciennes PR

## Description de PR

Écrire en français.

Format conseillé :

```markdown
## Résumé
- point concret 1
- point concret 2
- point concret 3

## Vérifications
- typecheck OK
- tests ciblés OK
```

Règles :

1. Les puces doivent parler du comportement ou de l'impact fonctionnel/technique.
2. Évite les formulations creuses comme `mise à jour du code`.
3. Si des vérifications ont été exécutées, elles doivent être listées explicitement.

## Lien ADO

Règles :

1. Le commit doit inclure `#<workItemId>` pour créer le lien automatique.
2. La PR doit être liée explicitement au work item quand l'outil le permet.
3. Un bug sans tâche enfant ne doit pas recevoir une fausse tâche juste pour le format.

## Reviewers et hygiène PR

1. Quand la PR est ouverte, ajouter les reviewers demandés dans la même séquence si possible.
2. Quand une PR en remplace une autre mal nommée, intermédiaire ou obsolète, abandonner l'ancienne pour éviter le bruit.
3. Si le work item concerné supporte l'état `PR en attente`, le passer à cet état dans le même flux.
