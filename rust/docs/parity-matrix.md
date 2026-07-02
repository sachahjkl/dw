# Rust Parity Matrix

## Phase 0

| Area | .NET Reference | Rust Status | Evidence | Gap |
| --- | --- | --- | --- | --- |
| ADO auth env/PAT | `AzureDevOpsTokenProvider` | bootstrap | env detection added | MSAL Windows auth not validated |
| ADO expanded read | `AzureDevOpsClient` | bootstrap | endpoint URI generation added | real HTTP fetch not implemented |
| SQL readonly guard | `SqlReadOnlyGuard` | bootstrap | Rust guard added | parity tests vs .NET not run |
| SQL Server connection | `SqlServerQueryService` | not started | reference inspected | `tiberius` spike not implemented |
| Upgrade continuity | `UpgradeCommand` | not started | reference inspected | `.NET -> Rust` path not implemented |

## Phase 1 Candidates

| Command | .NET Reference | Rust Status | Notes |
| --- | --- | --- | --- |
| `dw version` | `VersionCommand` | bootstrap | simple early parity target |
| `dw config show` | `ConfigCommand` | bootstrap | root/color + paths locales disponibles |
| `dw task status` | `TaskListService` | not started | needs workspace parsing |
| `dw task current` | `WorkspaceCurrentService` | not started | deterministic local target |
| `dw task handoff-validate` | `TaskHandoffValidateService` | bootstrap | validation par `--workspace`, parser strict ajoute |
