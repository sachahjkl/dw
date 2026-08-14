# Scratch workspaces

## Goal

Add workspaces for local experiments that are independent of provider work
items. A scratch workspace is a first-class workspace kind, never a tracked
workspace with a fake work item ID.

The first version must support the CLI, Web cockpit, and TUI through shared
application actions.

## Commands

Create a scratch workspace:

```text
dw workspace scratch start \
  --project <PROJECT> \
  --title "Test a cache strategy" \
  [--slug cache-strategy] \
  [--only front,back] \
  [--execute]
```

Promote it to tracked work:

```text
dw workspace scratch promote <WORK_ITEM_ID> \
  --workspace <PATH> \
  [--execute]
```

List and clean scratch workspaces:

```text
dw workspace list --kind scratch
dw workspace prune --kind scratch --older-than 30d
```

Creation and promotion are previews unless `--execute` is supplied.

## Manifest

Introduce manifest schema 2 with a workspace discriminator:

```json
{
  "schema": 2,
  "kind": "scratch",
  "workspaceId": "01K2...",
  "title": "Test a cache strategy",
  "project": "ha",
  "type": "spike",
  "slug": "test-cache-strategy",
  "branchName": "spike/test-cache-strategy",
  "repositories": ["front", "back"]
}
```

Tracked workspaces use `kind: "tracked"` and retain their work item fields.
Existing manifests without `kind` must continue to load as tracked workspaces.
They may be rewritten as schema 2 on a later mutation. Do not require an eager
migration.

Every scratch gets a persistent ULID `workspaceId`. Use its short form in UI.
Keep normal folder and branch names readable; append a short ULID suffix only
when resolving a collision.

## Naming

- A configured project is mandatory.
- A human title is mandatory.
- The default type is `spike`.
- Derive the slug from the title unless `--slug` is supplied.
- Default folder: `scratch-spike-<slug>`.
- Default branch: `spike/<slug>`.
- Promotion preserves `workspaceId` but adopts normal tracked naming based on
  the work item and renames the workspace and all repository branches.

## Generated files

Create the normal manifest, workspace instructions, and repository handoffs.
Generate a lightweight scratch `plan.md` with these sections:

```markdown
## Hypothesis

## Experiment

## Expected result

## Decision
```

Handoffs remain normal so commit, verification, and later promotion do not
need alternate repository contracts.

## Lifecycle

These commands support scratch workspaces:

- `workspace open|current|status|list|rename`
- `workspace repo add|latest`
- `workspace commit`
- `workspace handoff validate`
- `workspace preflight`
- `workspace teardown|prune`

Provider-specific operations are not applicable before promotion:

- `workspace sync`
- `workspace context refresh`
- `work item child create`

Return an actionable not-applicable error without contacting a provider.

`workspace item add` must not create a hybrid workspace. On a scratch, block
it and suggest `workspace scratch promote <WORK_ITEM_ID>`.

## Preflight

Scratch preflight validates:

- manifest validity;
- the lightweight plan structure;
- repositories and Git blockers;
- repository handoffs.

It ignores provider context, work item consistency, child-task policy, and
provider state policy.

## Commit, push, and pull requests

Allow local commits. Block `workspace finish --create-pr` and any finish path
that would push an unpromoted scratch. Return this recovery guidance:

```text
A scratch workspace must be promoted before creating a pull request.
Run:
  dw workspace scratch promote <WORK_ITEM_ID> --workspace <PATH> --execute
```

## Promotion

The preview must report:

- target work item;
- resulting type and slug;
- workspace and branch renames;
- provider state changes;
- child tasks to create;
- affected repositories.

Execution must:

1. Load and validate the target work item and provider capabilities.
2. Detect target workspace and branch collisions before mutation.
3. Verify that every local rename can succeed.
4. Rename repository branches and the workspace.
5. Convert the manifest to `kind: "tracked"` while preserving `workspaceId`.
6. Apply the normal `workspace start` policies.
7. Create required child tasks.
8. Update provider state when configured.
9. Refresh `.dw/work-item-context.json`.
10. Regenerate workspace instructions, plan, and affected handoffs.

Local mutations should roll back on failure. Provider mutations form a saga
and cannot be fully atomic: reports and errors must expose every completed
remote effect so recovery is explicit.

## Cleanup

Scratch cleanup is explicit and age-based:

```text
dw workspace prune --kind scratch --older-than 30d
```

It remains a preview without `--execute --yes`. Determine activity from local
repository commits and relevant file modifications, not provider state and not
only manifest creation time. Never include scratch workspaces in normal
provider-state pruning.

## Application architecture

Add shared typed actions:

- `workspace.scratch.start`
- `workspace.scratch.promote`

CLI, Web, and TUI must project these same requests, plans, results, events, and
validation errors. Do not implement separate lifecycle logic per interface.

## Delivery

Implement on `ogf`, run `go test ./...`, then cherry-pick to the public
`master` worktree and make only profile-specific template edits. Run the full
suite on both branches. Finally build and install the OGF executable locally,
restart the Web service, and verify its version and scratch command help.
