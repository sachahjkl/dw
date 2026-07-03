# dw

`dw` is a native Rust Dev Workflow CLI for AI-assisted work across Azure DevOps, Git worktrees, multi-repository projects, agent context, and read-only SQL Server introspection.

The CLI is the deterministic rail. AI agents still do the reasoning and editing, but `dw` keeps workflow state, filesystem layout, Git operations, ADO context, database access, and release/update mechanics predictable.

## Build

```bash
cargo run -p dw-cli -- version
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

With Nix:

```bash
nix develop
nix run . -- version
nix run .#check
nix build .#default
```

`VERSION` is the release version source. The full runtime version is rendered as:

```text
Dev Workflow YYYY.MM.DD.N+COMMIT
```

## Main Commands

- `dw init`: create/update a DevWorkflow root with config, schemas and templates.
- `dw doctor`: inspect environment/configuration health.
- `dw auth login/status/logout`: Azure DevOps auth through MSAL/keyring or PAT fallback.
- `dw ado assigned/work-item/context/ai-context/changelog`: Azure DevOps read workflows.
- `dw db guard/schema/describe/query`: SQL Server readonly helpers.
- `dw task start/open/list/current/status/sync/rename/preflight/handoff-validate/add-work-item/remove-work-item/add-repo/repo-latest/commit/finish/teardown/prune`: task workspace lifecycle.
- `dw agent open/config`: agent launch and workspace config generation.
- `dw secret get/set/delete`: local secret storage.
- `dw upgrade --check`: release manifest check for binary installs.

## Release Artifacts

Build local release artifacts:

```bash
VERSION="$(cat VERSION)" COMMIT="$(git rev-parse --short HEAD)" bash ./scripts/publish-linux-x64.sh
pwsh ./scripts/publish-win-x64.ps1 -Version "$(cat VERSION)" -Commit "$(git rev-parse --short HEAD)"
```

The Linux artifact is written to:

```text
artifacts/linux-x64/dw-linux-x64.tar.gz
```

The Windows artifact is written to:

```text
artifacts/win-x64/dw-win-x64.zip
```

Release workflows also produce `release.json`, consumed by `dw upgrade --check` and `dw upgrade`.

## CI

GitHub Actions runs Rust CI on Linux and Windows:

- `cargo fmt --all -- --check`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- Nix flake build/check on Linux
- Linux and Windows artifact publishing smoke paths

## Repository Layout

```text
crates/
  dw-cli            top-level CLI and cross-domain orchestration
  dw-config         config files, init, refresh and config diagnostics
  dw-ado            Azure DevOps auth/client/mapping
  dw-ado-commands   ADO command handlers and rendering
  dw-db             SQL Server readonly commands
  dw-doctor         machine/config diagnostics
  dw-task           task command handlers and UX
  dw-upgrade        release manifest checks and binary self-upgrade
  dw-workspace      workspace planning, manifests and contracts
  dw-agent          agent launch/config support
  dw-secret         secret storage
  dw-git            git/worktree helpers
  dw-ui             terminal styling and prompts
schemas/            JSON schemas copied into DevWorkflow roots
scripts/            release artifact scripts
docs/               architecture and agent reference material
```

## Notes

The former C#/.NET implementation has been removed from this branch. Remaining release blockers are external validation items: real Windows ADO auth/write flows, real SQL Server readonly access, safe ADO PR/linking validation, and upgrade behavior on installed Windows binaries.
