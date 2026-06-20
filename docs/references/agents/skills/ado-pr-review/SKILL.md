## Skill: ado-pr-review

### Objectif

Ce skill sert a faire une revue de pull request Azure DevOps orientee risques, regressions et manques de couverture.

Il formalise le workflow utilise dans la revue de la PR `21388` sur `HOMMAGE AGENCE`:

1. identifier exactement la PR, le depot et la branche cible
2. lire les metadonnees PR, work items, threads et fichiers modifies
3. inspecter le diff reel, puis les fichiers complets les plus sensibles
4. distinguer les regressions introduites par la PR des comportements deja existants
5. remonter des findings actionnables avec references fichier/ligne
6. si demande, poster des commentaires inline sur la PR

### Quand utiliser ce skill

Utiliser ce skill quand:

1. l'utilisateur demande une revue de PR Azure DevOps
2. il faut commenter une PR ADO avec des findings techniques
3. il faut verifier une PR avec focus bugs/regressions plutot qu'un simple resume

### Regles de sortie

La sortie de revue doit suivre ces regles:

1. findings d'abord, resumes ensuite
2. ordonner par severite decroissante
3. chaque finding doit contenir:
   - un niveau de severite (`High`, `Medium`, `Low`)
   - au moins une reference fichier/ligne
   - le comportement observe
   - l'impact concret ou la regression possible
4. si aucun finding n'est retenu, le dire explicitement
5. ne pas gonfler artificiellement la revue avec des preferences de style si le risque produit est faible

### Workflow recommande

#### 1. Identifier la PR

Recuperer:

1. le projet ADO
2. le depot
3. l'id de PR
4. la branche source et la branche cible
5. le statut draft/active/completed

Outils utiles:

1. `ado_repo_list_pull_requests_by_repo_or_project`
2. `ado_repo_get_pull_request_by_id`

#### 2. Lire le perimetre reel

Toujours recuperer:

1. la fiche PR complete
2. les fichiers modifies
3. le diff de la PR
4. les threads de review existants

Outils utiles:

1. `ado_repo_get_pull_request_by_id`
2. `ado_repo_get_pull_request_changes`
3. `ado_repo_list_pull_request_threads`

Important:

1. la verite de la PR est son diff, pas l'historique complet de la branche
2. utiliser l'historique de commits seulement pour comprendre l'intention ou detecter des zones a risque
3. ne pas conclure a partir d'un seul commit si la PR en contient plusieurs

#### 3. Lire le code complet autour du diff

Pour chaque zone sensible du diff:

1. lire le fichier complet au commit source de la PR
2. verifier les contrats appeles autour de la modification
3. verifier les effets de bord dans les services/repositories/modeles relies

Cas typiques a verifier:

1. contrats/interfaces modifies
2. mapping DTO/modeles/API
3. persistance base ou repository
4. logique d'autorisation ou de filtrage
5. synchronisation entre anciens flags et nouveau modele
6. gestion des null et fallbacks
7. DI/injection de dependances

Outil utile:

1. `ado_repo_get_file_content`

#### 4. Construire les findings

Un finding est valable s'il verifie au moins un de ces criteres:

1. regression fonctionnelle probable
2. bug de logique introduit par la PR
3. perte de donnees ou ecrasement silencieux possible
4. erreur de null handling pouvant produire une 500 ou un crash
5. oubli d'un chemin de lecture/ecriture apres refacto
6. incoherence entre contrat modele et serialisation
7. absence de fallback alors que l'ancien comportement en avait un

Ne pas remonter comme finding principal:

1. un simple point de style
2. un renommage discutable sans impact
3. une hypothese non verifiee dans le code lu

#### 5. Formulation recommandee

Format recommande:

```markdown
1. `High` `path/file.cs:10-25`
   Description courte du probleme.
   Impact concret ou regression.
```

Bon reflexe:

1. decrire d'abord le mecanisme
2. decrire ensuite pourquoi il casse
3. finir par l'impact concret

#### 6. Commenter la PR si demande

Si l'utilisateur demande de commenter la PR:

1. commenter uniquement les findings retenus
2. preferer un commentaire inline sur la ligne ou le bloc modifie
3. prefixer chaque commentaire par `[IA][{model id}][{infos pertinentes}]`
4. ecrire en francais, court, direct, actionnable
5. ne pas dupliquer un thread deja present sauf si on apporte un angle nouveau

Outil utile:

1. `ado_repo_create_pull_request_thread`

Modele de commentaire inline:

```text
[IA][github-copilot/gpt-5.4][review] Ici <probleme concret>. Dans ce cas <scenario>. Impact: <regression/500/perte de donnees/comportement incorrect>.
```

Infos pertinentes recommandees dans le troisieme bloc:

1. `review`
2. `bug`
3. `regression`
4. `null-handling`
5. `data-loss`

### Heuristiques importantes

1. si une propriete devient source de verite mais reste ignoree en serialisation, verifier le risque d'effacement silencieux au save
2. si un service passe d'un modele legacy a un modele V2, verifier tous les chemins ou l'objet V2 peut etre `null`
3. si une PR remplace un bool legacy par une liste de regles, verifier:
   - chargement
   - sauvegarde
   - mapping legacy -> nouveau modele
   - mapping nouveau modele -> legacy
4. si la PR ajoute une nouvelle injection DI, verifier que les appels dependants gerent l'absence de resolution metier, pas seulement l'injection technique
5. si le diff est gros, cibler d'abord les zones qui combinent modele + service + repository + filtre de recherche

### Checklist

Lire aussi `references/review-checklist.md`.
