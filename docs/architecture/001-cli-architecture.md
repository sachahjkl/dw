# CLI Architecture

`dw` is one Go 1.26 module that produces one standalone executable per supported platform: `dw` for Linux x64 and `dw.exe` for Windows x64. Release builds disable CGO. Git must be available on `PATH` for repository and worktree operations. macOS is not supported.

## Composition and Layers

`cmd/dw` is only the process entry point. `internal/bootstrap` composes the command grammar, routes, action dispatcher, providers, console renderers, and TUI. Business operations remain below those presentation and composition boundaries.

```text
CLI grammar / Charm v2 TUI
  -> controller routes
     -> typed action dispatcher and application services
        -> capability-based work and data providers
           -> filesystem / Git / Azure DevOps / SQL Server / secrets / updates
```

`internal/cli/spec` is the source of commands, subcommands, arguments, options, descriptions, help, and completion metadata. `internal/cli/controller` maps parsed invocations to typed requests. Both the CLI and TUI dispatch through the same application actions; provider packages do not parse commands or render terminal output.

## Namespace Boundaries

`work` exposes external provider-neutral work items, pull requests, context, and changelogs. `workspace` owns local filesystem and Git lifecycle. `data` operates on configured generic data sources. `provider` owns registry introspection and provider authentication. These boundaries are public contracts: controllers dispatch generic action IDs and do not retain aliases for earlier product-shaped routes.

## Provider Model

Work and data integrations use separate ordered static registries composed in `internal/bootstrap`. A provider supplies an identity and implements only the capability interfaces it supports. Callers request the typed capability they need and receive a typed unsupported-capability error when it is absent. Workspace lifecycle operations resolve the configured project provider through the work registry instead of retaining a concrete provider. Application and core packages are prohibited from importing concrete provider packages; the Nix architecture check enforces that boundary. Providers are linked into the executable; there is no runtime plugin loading.

`dw provider list|show|capabilities` derives reports by walking the work registry followed by data-only entries, coalescing a name registered for both kinds, and inspecting capability interfaces. Kinds remain ordered `work`, then `data`, and capability tokens are deterministic. The reports never use a hardcoded provider-name list.

The current work provider is Azure DevOps, implemented under `internal/work/ado`. Its optional capabilities cover authentication, work-item and relation reads, state changes, child creation, pull requests, rich context, and provider-specific commit-reference extraction for Git changelogs. A project selects its default work provider through generic configuration; `--provider` overrides it for a command, while `dw provider auth ... <provider>` always selects positionally. The same contracts support future GitHub or Jira providers.

The current data provider is SQL Server, implemented under `internal/data/sqlserver`. Every data source names its provider. Data capabilities distinguish discovery, catalogs, descriptions, native queries, tabular or workbook reads, document reads, read policies, and credential resolution. Source collection invokes the selected provider's discovery capability; generic application orchestration owns masked reporting and conservative persistence without recognizing SQL Server. This permits future SQLite, Excel, or NoSQL providers without adding backend-specific branches to command orchestration.

## Presentation and Localization

The interactive interface uses Charm v2: Bubble Tea, Bubbles, and Lip Gloss. `internal/tui` owns interactive state and rendering while invoking the same typed application actions as the CLI.

All human-facing CLI, TUI, and console text crosses the localization bridge in `internal/l10n`. English messages are embedded from `locales/active.en.toml`; command names, JSON keys, error codes, and other machine tokens are not localized.

## Source Layout

```text
cmd/dw/                    process entry point
internal/action/           typed dispatch contracts
internal/bootstrap/        static composition root
internal/cli/              grammar, parsing, completion, and routing
internal/work/             work-provider contracts and registry
internal/work/ado/         Azure DevOps provider
internal/data/             data-provider contracts and registry
internal/data/sqlserver/   SQL Server provider
internal/console/          deterministic human and machine output
internal/tui/              Charm v2 interactive interface
internal/l10n/             English localization bridge
locales/                   embedded message catalogs
schemas/                   generated-root JSON schemas
scripts/                   release packaging and manifest scripts
```

## Build and Release

```bash
go fmt ./...
go test ./...
go vet ./...
go build -o ./dw ./cmd/dw
```

The Nix development shell pins Go 1.26. `nix run .#check` runs formatting checks, tests, vet, and architecture checks; `nix build .#default` builds the package. GitHub Actions builds and smoke-tests CGO-disabled Linux x64 and Windows x64 archives, validates the Nix package, and creates the combined `release.json` manifest used by upgrades. No macOS release is produced.

## Versioning

The version format is:

```text
YYYY.MM.DD.BUILD+COMMIT
```

The date prefix is supplied by `VERSION`; the commit suffix comes from the build environment or Git metadata.
