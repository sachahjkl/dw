# Workspaces

`dw` separates stable configuration from ephemeral work.

```text
<root>/
  config/
  projects/
    <project>/
      repositories/
        front.git/
        back.git/
      workspaces/
        <subject>/
          front/
          back/
          task.json
          plan.md
```

## Bare Repositories

Project repositories should be anchored as bare repositories. They are not the place where work happens; they are the source from which subject worktrees are created.

## Subject Workspaces

A subject workspace groups local state for one external work subject, regardless of work provider:

- `task.json`: machine-readable task metadata
- `plan.md`: analysis and execution plan
- `front/`: optional front worktree
- `back/`: optional back worktree

Even if only one repository is involved, the subject folder still exists.

All local lifecycle commands live under `dw workspace`. When start or finish needs external work or pull-request capabilities, the operation resolves the configured project work provider (or an applicable `--provider`) through the registry; local Git and filesystem code never imports a concrete provider.

## Naming

Folder subject:

```text
type-id-short-slug
```

Branch:

```text
type/id-task-short-slug
```

Provider-specific work-item and Git naming rules belong in provider configuration or agent skills; the workspace engine consumes only normalized subject, repository, and branch values.
