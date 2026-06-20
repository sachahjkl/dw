# Scripts de worktree

Les scripts PowerShell de worktree lives dans le skill `ado-workitem`, sous :

```text
C:\Users\froment\.agents\skills\ado-workitem\references\worktree-tools\
```

Ils s'appuient sur :

- `Common.WorktreeTools.ps1`
- `worktree-tools.config.json`

## Principe

Le modèle attendu est :

```text
ws/<family>/<type-id-slug>/front
ws/<family>/<type-id-slug>/back
```

Les nouveaux worktrees doivent être créés depuis les anchors bare présents dans :

```text
ws/.anchors/
```

## Scripts disponibles

- `new-worktree.ps1`
- `remove-worktree.ps1`
- `migrate-worktrees.ps1`

## Exemples

Créer un sujet HA front + back :

```powershell
.\new-worktree.ps1 -Family ha -SubjectName "feat-53847-type-convoi-planning-rh"
```

Si le script n'est pas lance depuis un sous-dossier du workspace, fournir aussi `-WorkspaceRoot` en pointant le dossier `ws` (celui qui contient `.anchors`).

Compatibilite legacy : si le parent de `ws` est fourni, le script normalise automatiquement vers `.../ws`.

Créer un sujet HE back seul :

```powershell
.\new-worktree.ps1 -Family he -SubjectName "bug-53279-documents-tableau-bord" -BackOnly
```

Supprimer un sujet et ses branches locales :

```powershell
.\remove-worktree.ps1 -Family he -SubjectName "bug-53279-documents-tableau-bord" -DeleteBranch
```

Relancer une migration :

```powershell
.\migrate-worktrees.ps1 -Family ha
.\migrate-worktrees.ps1 -Family he
```

## Configuration

Les URLs des remotes, les noms d'anchors bare et les dossiers de workspace sont declaratifs dans :

```text
references/worktree-tools/worktree-tools.config.json
```

Si l'arborescence ou les remotes changent, mettre a jour ce fichier plutot que le code PowerShell.

## Regles d'usage

1. Preferer les scripts aux commandes Git saisies a la main pour les operations courantes.
2. Ne pas supprimer ou renommer un linked worktree depuis l'explorateur Windows.
3. Utiliser `git worktree move` ou les scripts de migration pour deplacer un worktree existant.
4. Garder le `SubjectName` aligne sur la branche reelle normalisee, par exemple `feat-53847-type-convoi-planning-rh`.
