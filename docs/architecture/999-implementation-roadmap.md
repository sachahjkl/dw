# Implementation Roadmap

## Phase 0 - Foundation

- .NET 10 CLI
- buildable solution
- `version`
- `doctor`
- `init`
- `agent context`
- initial docs

## Phase 1 - Workspaces

- load `projects.json`
- initialize bare repositories
- create worktrees
- create `task.json`
- create `plan.md`
- detect changed repos

## Phase 2 - Azure DevOps

- MSAL/browser auth
- Windows Credential Manager secret store
- read work item
- update states
- create child tasks
- create PRs
- TODO: assign work item

## Phase 3 - Git Finish Flow

- status checks
- configured verification commands
- push
- PR creation
- PR descriptions from `plan.md`
- state transitions after PR
- commit message work item reference enforcement
- TODO: tune repo-specific verification commands from real projects

## Phase 4 - SQL

- config loading
- secret resolution
- schema introspection
- guarded query execution
- max rows and timeout

## Phase 5 - Updates and Packaging

- publish win-x64
- generate release manifest
- verify SHA256
- `dw upgrade --check`
- `dw upgrade`
- self-replace executable strategy

## Phase 6 - Config Schemas and CI

- JSON Schemas for `projects.json`, `workflow.json`, `databases.json`, `release.json`
- `dw init` writes schemas next to generated config
- GitHub Actions .NET build/test/publish
- GitHub Actions Nix flake check

## Phase 7 - Rust Rewrite Bootstrap

- dedicated `rust/` workspace kept side by side with `.NET`
- Cargo workspace aligned with target architecture
- Phase 0 feasibility notes for ADO, SQL, upgrade and Windows validation
- no release cutover before parity gates are validated
