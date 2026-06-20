# DW

DW is a deterministic developer workflow CLI.

Its purpose is to provide rails around everyday enterprise development workflows involving:

- Azure DevOps work items and pull requests
- Git repositories and worktrees
- multi-project frontend/backend applications
- OpenCode agent configuration
- SQL Server introspection and read-only investigation
- local bootstrap, diagnostics, updates, and workspace management

The guiding rule is:

> The AI reasons. DW executes.

DW is not an OpenCode plugin. DW is a workflow engine that can generate and support OpenCode configuration, expose context for agents, and provide deterministic commands that agents and humans can use safely.

## Target stack

- Runtime: .NET 8
- Distribution: framework-dependent Windows x64 executable for MVP
- Primary platform: Windows 11
- Repository: public GitHub repository
- Update provider: GitHub Releases with `release.json` and SHA256 verification
- Azure DevOps integration: REST API using browser-based Microsoft authentication, not Azure CLI
- Secrets: local secret abstraction, Windows Credential Manager first

## Core commands

Initial MVP commands:

```bash
dw version
dw doctor
dw init --root <path>
dw update
dw auth login
dw auth status
dw auth logout
dw project list
dw project sync
dw task start <workItemId> --project <projectKey> --auto
dw task status
dw task add-repo frontend|backend
dw task finish
dw workspace list
dw workspace clean
dw db setup
dw db schema
dw db describe <schema.table>
dw db query --readonly "select ..."
dw agent context
```

## Documentation map

Start with:

- `docs/architecture/000-project-overview.md`
- `docs/architecture/001-cli-architecture.md`
- `docs/architecture/002-filesystem-layout.md`
- `docs/architecture/003-work-items-and-workspaces.md`
- `docs/architecture/004-opencode-integration.md`
- `docs/architecture/005-azure-devops.md`
- `docs/architecture/006-database.md`
- `docs/architecture/007-update-system.md`
- `docs/architecture/008-versioning.md`
- `docs/architecture/009-security.md`
- `docs/architecture/010-config-schemas.md`
- `docs/roadmap/000-implementation-roadmap.md`

## Existing project-specific skills

Project-specific rules, agent definitions, Azure DevOps workflow details, branch naming rules, commit conventions and PR process should live under `docs/` and remain the source of truth for team-specific behavior.

DW should convert stable rules into deterministic configuration where possible, but should not hard-code team conventions in random prompts or scattered code.
