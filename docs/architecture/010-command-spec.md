# Command Specification

This document describes command intent and current implementation status.

## `dw version`

Implemented.

Prints:

- informational version
- runtime version
- OS platform

## `dw doctor [--fix]`

Implemented as first pass.

Checks:

- DevWorkflow root
- user settings
- Git
- .NET 10 runtime
- OpenCode on PATH

`--fix` initializes the default root if it is missing.

## `dw init [--root <path>] [--no-save]`

Implemented.

Creates:

```text
config/projects.json
config/workflow.json
config/databases.json
config/opencode/AGENTS.md
config/opencode/opencode.jsonc
schemas/projects.schema.json
schemas/workflow.schema.json
schemas/databases.schema.json
schemas/release.schema.json
projects/
cache/
```

By default it persists the selected root in the user settings file.

Generated config files include local `$schema` links that resolve to the generated `schemas/` directory.

`--no-save` is intended for tests/smoke runs.

## `dw agent context`

Implemented.

Prints stable AI-agent context.

## `dw task start`

Implemented.

Current behavior:

```text
dw task start <workItemId> --project <project> --task <taskId> --slug <slug> --type <feat|fix|bug|chore> --only front,back [--create-child-tasks] [--skip-ado]
```

Creates:

- subject workspace
- per-repo Git worktrees when repository URLs are configured
- placeholder directories when repository URLs are intentionally empty
- `task.json`
- `plan.md`
- optional ADO child tasks named `[FRONT][AI] ...` / `[BACK][AI] ...`
- optional ADO start-state transition when auth is available

## `dw task status`

Implemented as first pass.

Lists detected `task.json` files under the configured root.

## `dw task finish`

Implemented.

Current behavior:

- inspect repo changes
- dry-run by default
- run configured `taskFinish.verificationCommands`
- with `--execute --message "<message>"`: `git add`, `git commit`, `git push -u origin <branch>`
- with `--execute --create-pr`: attempts Azure DevOps PR creation for changed repositories
- enrich PR descriptions from `plan.md`
- link known work item ids to PR creation payload
- move Bug/Task work items to `PR en attente`
- never move User Story/Anomalie to `PR en attente`

Status: functionally implemented. Remaining work is real-environment validation against BUSINESS Azure DevOps boards/repositories and tuning configured verification commands per repo.

## `dw auth`

Implemented first pass.

Current behavior:

- browser/MSAL login
- token acquisition using configured Azure DevOps scopes
- PAT/environment fallback for non-interactive usage
- status
- logout placeholder

## `dw db`

Implemented first pass.

Target behavior:

- SQL Server read-only introspection
- guarded query execution
- max rows and timeout enforcement
- connection strings from config, environment variables, or Windows Credential Manager `credentialKey`

## `dw upgrade`

Implemented first pass.

Current behavior:

- GitHub Releases lookup
- `release.json` asset parsing
- SHA256 validation
- direct binary upgrade

Nix-managed installs must use Nix upgrade commands instead.
