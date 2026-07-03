# Rust Cutover Status

The Rust implementation has been hoisted to the repository root and is now the active implementation.

The former C#/.NET source tree, solution, tests, build scripts and CI jobs have been removed from this branch.

## Implemented

- self-contained Rust workspace, `VERSION`, Nix flake, schemas, release scripts and GitHub workflows
- command crates for config, ADO, DB, task/workspace, agent, secret, git and UI
- dynamic completion catalogs exposed by domain crates and composed by `dw-cli`
- MSAL/keyring-based ADO auth plus PAT/environment fallback
- ADO read flows: `assigned`, `work-item`, `context`, `ai-context`, `changelog`
- ADO write flows used by task lifecycle: child task creation, PR creation/linking, work item state updates
- DB guard/schema/describe/query using `tiberius`, readonly guard and SQL Server ApplicationIntent readonly
- task workspace flows: start/open/list/current/status/sync/rename/preflight/handoff-validate/add/remove work items/add repo/repo-latest/commit/finish/teardown/prune
- richer terminal output via `dw-ui`, plus interactive project/work-item/repository/workspace selection where useful
- Linux and Windows release artifact scripts plus `release.json` continuity for `dw upgrade`

## Remaining Proof Gates

These gates require real external environments and should be validated before a production release cutover:

- validate ADO auth/read/write flows on real Windows environments
- validate DB schema/describe/query against a real SQL Server connection
- validate PR creation and work item linking on a safe real ADO project
- validate Rust self-upgrade on Windows, including running-executable replacement behavior
- run side-by-side parity checks on representative real work items and workspaces
- capture/update real golden fixtures for ADO context, preflight and handoff validation

## Local Validation

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
nix run .#check
```
