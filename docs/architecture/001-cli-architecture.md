# CLI Architecture

The CLI is a native Rust workspace targeting Linux and Windows release binaries.

The top-level `dw-cli` crate owns command-line composition and delegates domain behavior to dedicated crates. Domain crates expose command handlers, completion catalogs, and rendering helpers where appropriate.

## Layers

```text
Commands
  -> application services
     -> providers
        -> filesystem / git / azure devops / sql / secrets / updates
```

The command tree is declared with `clap`. Commands, subcommands, arguments, options, descriptions, help, and shell suggestions must come from that tree plus the domain completion catalogs instead of ad-hoc command parsing or hardcoded usage text.

Handlers receive typed option records. `dw-cli` should stay thin: it routes to domain crates and handles cross-domain orchestration only when necessary.

## Command Groups

- `doctor`: environment diagnostics and guided remediation
- `init`: root bootstrap and template generation
- `agent`: stable context for AI agents
- `task`: task workspace lifecycle
- `auth`: Azure DevOps authentication
- `db`: read-only SQL Server access
- `upgrade`: GitHub Releases update flow

## Versioning

The intended version format is:

```text
YYYY.MM.DD.BUILD+COMMIT
```

The date prefix is supplied by `VERSION`; the commit suffix comes from the build environment or Git metadata.
