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

A subject workspace groups everything for one Azure DevOps subject:

- `task.json`: machine-readable task metadata
- `plan.md`: analysis and execution plan
- `front/`: optional front worktree
- `back/`: optional back worktree

Even if only one repository is involved, the subject folder still exists.

## Naming

Folder subject:

```text
type-id-short-slug
```

Branch:

```text
type/id-task-short-slug
```

The detailed naming rules live in `docs/references/agents/skills/ado-workitem/references/git-naming.md`.
