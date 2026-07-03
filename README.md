# dw

`dw` is the Dev Workflow CLI for AI-assisted work across Azure DevOps, Git worktrees, multi-repository projects, agent context, and read-only SQL Server introspection.

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

## Install

### Nix

Run the CLI without installing it:

```bash
nix run github:sachahjkl/dw -- version
nix run github:sachahjkl/dw -- doctor
```

Refresh to the latest pushed revision when needed:

```bash
nix run --refresh github:sachahjkl/dw -- version
```

Install it into your Nix profile for repeated use:

```bash
nix profile install github:sachahjkl/dw
dw version
```

Upgrade a profile install:

```bash
nix profile upgrade github:sachahjkl/dw
```

`dw upgrade` is disabled for Nix-managed installs. Use `nix run --refresh ...` or `nix profile upgrade ...` instead.

### Release Binaries

Windows install from the latest GitHub release:

```powershell
irm https://raw.githubusercontent.com/sachahjkl/dw/master/scripts/install.ps1 | iex
# or:
iwr https://raw.githubusercontent.com/sachahjkl/dw/master/scripts/install.ps1 -UseBasicParsing | iex
```

Linux/WSL install from the latest GitHub release:

```bash
curl -fsSL https://raw.githubusercontent.com/sachahjkl/dw/master/scripts/install.sh | sh
```

Default install locations:

```text
Windows: %LOCALAPPDATA%\DevWorkflow\bin
Linux/WSL: ~/.local/bin
```

The installers update the user shell/profile PATH unless `-NoPathUpdate` or `--no-path-update` is passed.

Manual downloads are also available from GitHub Releases:

- `dw-linux-x64.tar.gz`
- `dw-win-x64.zip`

For release-binary installs, `dw upgrade --check` can inspect the latest release manifest and `dw upgrade` updates the current binary.

### Local Build

Build and run the binary from source:

```bash
cargo build --locked --release -p dw-cli
./target/release/dw-cli version
```

Build local release artifacts:

```bash
VERSION="$(cat VERSION)" COMMIT="$(git rev-parse --short HEAD)" bash ./scripts/publish-linux-x64.sh
```

```powershell
$Version = Get-Content .\VERSION
$Commit = git rev-parse --short HEAD
powershell -ExecutionPolicy Bypass -File .\scripts\publish-win-x64.ps1 -Version $Version -Commit $Commit
```

## Main Commands

- `dw init`: create/update a DevWorkflow root with config, schemas and templates.
- `dw doctor`: inspect environment/configuration health.
- `dw auth login/status/logout`: Azure DevOps auth through OAuth/keyring or PAT fallback.
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
```

```powershell
$Version = Get-Content .\VERSION
$Commit = git rev-parse --short HEAD
powershell -ExecutionPolicy Bypass -File .\scripts\publish-win-x64.ps1 -Version $Version -Commit $Commit
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

GitHub Actions runs CI on Linux and Windows:

- `cargo fmt --all -- --check`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- Nix flake build/check on Linux
- Linux and Windows artifact publishing smoke paths

## Repository Layout

```text
crates/
  dw-cli            top-level CLI and cross-domain orchestration
  dw-completion     dynamic shell completion engine
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

## Workflow

The intended end-to-end flow is:

1. `dw task start ...` creates the workspace, agent files and handoffs.
2. The AI reads `dw ado work-item` and `dw ado ai-context`.
3. The AI runs `dw task preflight --continue` before implementation or child-task creation.
4. The plan is written in `plan.md` and split by domain when useful.
5. Domain handoffs such as `handoff-front.md`, `handoff-back.md`, `handoff-db.md` guide sub-agents.
6. The AI implements, verifies, commits with `dw task commit`, then finishes with `dw task finish`.

```mermaid
flowchart TD
    A[ADO Work Item] --> B[dw task start]
    B --> C[Workspace Created]
    C --> C1[task.json]
    C --> C2[plan.md]
    C --> C3[AGENTS.md]
    C --> C4[handoff-front/back/db.md]

    C --> D[AI reads dw ado work-item]
    D --> E[AI reads dw ado ai-context]
    E --> F[AI runs dw task preflight]

    F -->|blocking or warning| G[AI surfaces checks to user]
    G --> H{Proceed?}
    H -->|no| I[Clarify or wait]
    H -->|yes| J[Write plan.md]
    F -->|clean| J

    J --> K[Split work by domain]
    K --> L[Create child tasks if needed]
    K --> M[Launch sub-agents on independent tracks]

    L --> N[Implement in repos]
    M --> N
    N --> O[Update handoff summary blocks]
    O --> P[Run verification]
    P --> Q[dw task commit]
    Q --> R[dw task finish]
    R --> S[Push + PR + ADO updates]
```
