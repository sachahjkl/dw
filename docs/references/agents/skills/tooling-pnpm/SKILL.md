---
name: tooling-pnpm
description: Node tooling, pnpm, npm, worktree. Use when installing dependencies or running package-manager commands in a JS/TS project and pnpm is available on the machine, especially with multiple worktrees.
---

# Tooling pnpm

Utiliser ce skill quand le sujet touche:

1. l'installation de dependances Node
2. le choix entre `npm` et `pnpm`
3. un projet front JS/TS, Angular ou tooling Node
4. un contexte avec plusieurs worktrees ou plusieurs clones proches

## Regle principale

Si `pnpm` est detecte sur la machine, le preferer a `npm` pour les projets Node/Angular, sauf contrainte explicite du projet.

## Pourquoi preferer pnpm

1. `pnpm` reutilise un cache global de packages
2. il lie les dependances via liens symboliques au lieu de recopier inutilement les memes contenus
3. en contexte multi-worktree, cela evite de repliquer un gros `node_modules` complet dans chaque arborescence
4. cela reduit la charge disque et accelere souvent les installations repetitives
5. c'est particulierement utile sur des fronts volumineux comme Hommage Agence ou Hommage Exploitation

## Cas worktree

Quand plusieurs worktrees existent pour un meme repo ou pour des branches proches:

1. `npm` duplique plus facilement beaucoup de contenu disque entre worktrees
2. `pnpm` mutualise bien mieux les packages communs
3. le gain est surtout visible quand on alterne souvent entre plusieurs sujets front en parallele

## Verification a faire

Avant de choisir le gestionnaire de paquets:

1. verifier si `pnpm` est disponible
2. verifier si le repo impose deja un outil via lockfile ou scripts d'equipe
3. respecter la convention du projet si elle est explicite

Commandes utiles:

```bash
pnpm --version
npm --version
```

Indices utiles dans le repo:

```text
pnpm-lock.yaml
package-lock.json
```

## Regles de decision

1. si `pnpm-lock.yaml` existe, utiliser `pnpm`
2. si `pnpm` est installe et qu'aucune contrainte projet n'impose `npm`, preferer `pnpm`
3. si seul `package-lock.json` existe et que l'equipe depend explicitement de `npm`, rester sur `npm`
4. ne pas melanger sans raison plusieurs lockfiles dans le meme projet

## Formulation recommandee

Quand tu fais ce choix, l'expliquer simplement:

```text
pnpm detecte sur la machine, je le privilegie a npm ici car il mutualise mieux les dependances entre worktrees et limite la charge disque.
```

## Limite

Ce n'est pas une regle ideologique.

Si le projet, la CI, ou l'equipe impose `npm`, alors il faut suivre cette contrainte.
