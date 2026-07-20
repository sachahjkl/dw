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

## Provider Model

Work and data integrations use separate static registries composed in `internal/bootstrap`. A provider supplies an identity and implements only the capability interfaces it supports. Callers request the typed capability they need and receive a typed unsupported-capability error when it is absent. Providers are linked into the executable; there is no runtime plugin loading or provider discovery.

The current work provider is Azure DevOps, implemented under `internal/work/ado`. Its optional capabilities cover authentication, work-item and relation reads, state changes, child creation, pull requests, and rich context. The same contracts can support future GitHub or Jira providers by implementing the relevant capabilities and registering them at composition time.

The current data provider is SQL Server, implemented under `internal/data/sqlserver`. Data capabilities distinguish discovery, catalogs, descriptions, native queries, tabular or workbook reads, document reads, read policies, and credential resolution. This permits future SQLite, Excel, or NoSQL providers without adding backend-specific branches to command orchestration.

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
