# États Azure DevOps

## Objectif

Avant de changer un état, vérifier qu'il est valide pour le type de work item traité.

Le but n'est pas de forcer un état "qui ressemble" au bon. Le but est de garder un board propre et compatible avec le workflow réel du projet.

## États observés

### User Story

États valides :

```text
En instruction
En affinage
A développer
En réalisation
À déployer
En test
Validé
Clôturé
Abandonné
```

### Anomalie

États valides :

```text
En instruction
En affinage
A développer
En réalisation
À déployer
En test
Validé
Clôturé
Abandonné
```

### Bug

États valides :

```text
A faire
En développement
PR en attente
À déployer
En test
Clôturé
Abandonné
```

### Tâche / Activité

Si le board expose explicitement les états valides pour `Tâche` ou `Activité`, utiliser uniquement ces états.

À défaut, règles de départ :

1. Une `Tâche` démarre usuellement en `En développement` ou en `En réalisation` selon le board du projet.
2. Une `Activité` démarre usuellement dans l'état actif le plus cohérent avec le board ; dans ce contexte, `En réalisation` est généralement le meilleur choix si disponible.
3. Une `Tâche` ou une `Activité` peut passer à `PR en attente` si ce type supporte cet état dans le board concerné.
4. Dans ce contexte de travail, si l'agent crée une `Tâche`, il doit par défaut l'assigner immédiatement à l'utilisateur demandé et la passer à `En développement`, sauf instruction contraire de l'utilisateur ou contrainte explicite du board.

## Règles d'usage

1. Une US ou une anomalie démarre usuellement en `En réalisation` quand le développement commence.
2. Un bug ou une tâche technique démarre usuellement en `En développement`.
3. Une PR ouverte pour un bug, une tâche ou une activité peut justifier le passage à `PR en attente`.
4. Ne pas utiliser `PR en attente` pour une US ou une anomalie si ce n'est pas prévu par le workflow.

## Règle importante

Quand l'agent ouvre une PR pour un élément éligible à `PR en attente`, il doit faire la mise à jour d'état dans le même flux de travail, sans attendre une demande explicite de l'utilisateur.
