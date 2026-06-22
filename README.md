# dw

`dw` is a Dev Workflow CLI for AI-assisted work on Azure DevOps, OpenCode/Codex, Git worktrees, multi-repository front/back projects, and read-only SQL Server introspection.

The project goal is not to replace the AI agent. It gives the agent rails:

- deterministic filesystem, Git and workspace operations
- stable context for OpenCode/Codex
- Azure DevOps workflow conventions from `docs/references`
- safe defaults for database access
- a repeatable local layout for task workspaces

## Current State

This repository currently contains the first vertical slice:

- `.NET 8` CLI project
- `dw version`
- `dw doctor`
- `dw init` with `default` and `ogf` profiles
- `dw agent context`
- `dw task start/status/finish`
- Azure DevOps MSAL/PAT auth plumbing
- read-only SQL Server commands
- Windows Credential Manager backed secrets
- GitHub Releases update checks/downloads
- JSON Schemas for generated config files
- GitHub Actions CI, including a Nix flake check
- reference skills under `docs/references`
- command specification under `docs/architecture/010-command-spec.md`

## Build

```powershell
dotnet build .\Dw.slnx -c Release
```

Run locally:

```powershell
dotnet run --project .\src\Dw.Cli -- version
dotnet run --project .\src\Dw.Cli -- init --profile ogf --root C:\Dev\dw
dotnet run --project .\src\Dw.Cli -- init --root .\.smoke\root --no-save
dotnet run --project .\src\Dw.Cli -- doctor
dotnet run --project .\src\Dw.Cli -- agent context
dotnet run --project .\src\Dw.Cli -- task start 27485 --project default --task 55201 --slug "descriptif cours" --type feat --only front,back
dotnet run --project .\src\Dw.Cli -- task start 27485 --project ha --slug "descriptif cours" --create-child-tasks
dotnet run --project .\src\Dw.Cli -- task finish --workspace C:\Dev\dw\projects\ha\workspaces\feat-27485-descriptif-cours --execute --message "feat: descriptif cours" --create-pr
```

With Nix:

```bash
nix develop
nix run . -- help
nix run .#check
nix build .#default
nix run .#set-version
nix run .#set-version -- 2026.06.22.4
```

`VERSION` is the source of truth for the Nix package version and release version. Run `nix run .#set-version` before cutting a release; without arguments it writes `YYYY.MM.DD.<buildId>`, where `<buildId>` is the next number after existing tags matching `vYYYY.MM.DD.*`. Pass an explicit version when needed.

Install locally on Windows:

```powershell
.\scripts\install.ps1
```

Install from the latest GitHub release:

```powershell
irm https://raw.githubusercontent.com/sachahjkl/dw/master/scripts/install.ps1 | iex
```

Publish a Windows artifact and `release.json`:

```powershell
.\scripts\publish-win-x64.ps1 -Version 2026.06.20.1 -Commit abc1234
```

## Install

### Nix

Run the CLI without installing it:

```bash
nix run github:sachahjkl/dw -- help
nix run github:sachahjkl/dw -- auth status
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

`dw update check` and `dw update download` are disabled for Nix-managed installs. Use `nix run --refresh ...` or `nix profile upgrade ...` instead.

### Release Binaries

Windows install from the latest GitHub release:

```powershell
irm https://raw.githubusercontent.com/sachahjkl/dw/master/scripts/install.ps1 | iex
```

Default install location:

```text
%LOCALAPPDATA%\DevWorkflow\bin
```

The installer adds this directory to the user `PATH` unless `-NoPathUpdate` is passed.

Linux/WSL install from the latest GitHub release:

```bash
curl -fsSL https://raw.githubusercontent.com/sachahjkl/dw/master/scripts/install.sh | sh
```

Default install location:

```text
~/.local/bin
```

The Linux/WSL installer detects the current shell and updates the matching init file when possible:

- bash: `~/.bashrc`
- zsh: `~/.zshrc`
- fish: `~/.config/fish/config.fish`
- nushell: `~/.config/nushell/env.nu`
- PowerShell: `~/.config/powershell/Microsoft.PowerShell_profile.ps1`

Use `DW_NO_PATH_UPDATE=1` or `--no-path-update` to skip shell profile changes.

Windows release bundles are framework-dependent: the host machine must have the .NET 8 runtime. The zip contains `dw.exe` plus the native SQL Server dependency required by `Microsoft.Data.SqlClient`.

Linux users can download `dw-linux-x64.tar.gz` from the GitHub release, or build locally through Nix:

```bash
nix develop -c env VERSION=2026.06.22.4 COMMIT=abc1234 bash ./scripts/publish-linux-x64.sh
./artifacts/linux-x64/dw version
```

## CI and Release

Normal development happens on `develop`.

CI runs on pull requests and pushes to `develop`, `main`, or `master`:

- Windows job: restore, build, test, publish `win-x64`
- Linux job: install Nix, run `nix build .#default`, push the Nix derivation to Cachix, run `nix run .#check`, publish `linux-x64`

Releases are automated from `master`.

Before pushing a release commit, bump `VERSION`:

```bash
git fetch --tags
nix run .#set-version
git add VERSION
git commit -m "bump version to $(cat VERSION)"
git push origin master
```

The release workflow reads `VERSION` and fails early if `v<VERSION>` already exists.

When a commit lands on `master`, `.github/workflows/release.yml`:

1. reads `VERSION` and creates the matching tag, for example `v2026.06.22.4`
2. publishes Windows and Linux artifacts
3. creates a GitHub Release
4. uploads:
   - `dw-win-x64.zip`
   - `dw-linux-x64.tar.gz`
   - `release.json`

`release.json` is the manifest used by `dw update check` and `dw update download`.

For release-binary installs, `dw update check` can inspect the latest GitHub release manifest. For Nix-managed installs, use Nix upgrade commands instead.

Store a SQL connection string in Windows Credential Manager:

```powershell
dw secret set ha/dev
dw secret set ha/dev --from-env HA_DEV_CONNECTION_STRING
```

Then reference it from `databases.json`:

```json
{
  "provider": "sqlserver",
  "credentialKey": "ha/dev",
  "readonly": true
}
```

## Target Commands

```text
dw version
dw doctor [--fix]
dw init [--root <path>]
dw update [check]

dw auth login
dw auth status
dw auth logout

dw secret set
dw secret get
dw secret delete

dw task start <workItemId>
dw task status
dw task add-repo
dw task finish

dw db schema
dw db describe
dw db query

dw agent context
```

## Architecture

`dw` owns deterministic operations. Agents reason and edit code, but they should use `dw` for workflow operations.

```text
dw CLI       -> deterministic execution
OpenCode     -> AI engine, agents, analysis, implementation, review
Skills       -> company conventions and routing rules
Azure DevOps -> source of truth for work items, states and PRs
Git          -> source of truth for code
SQL Server   -> read-only introspection/query surface
```

## Local Layout

`dw init` creates this shape:

```text
<root>/
  config/
    projects.json
    workflow.json
    databases.json
    opencode/
      AGENTS.md
      opencode.jsonc
  projects/
  cache/
  schemas/
    projects.schema.json
    workflow.schema.json
    databases.schema.json
    release.schema.json
```

Task workspaces are created under project folders:

```text
<root>/
  projects/
    <project>/
      workspaces/
        feat-27485-descriptif-cours/
          task.json
          plan.md
          front/
          back/
```

## Non-Negotiable Rules

- Keep front and back repositories separate.
- Group worktrees for the same subject in one subject workspace.
- Keep plans as `plan.md` inside the subject workspace.
- Use the skills in `docs/references/agents/skills` as source of truth for ADO, Git naming and PR rules.
- SQL access is read-only by default.
- Do not make `dw` depend on Azure CLI.
- OpenCode is detected, not installed automatically.
