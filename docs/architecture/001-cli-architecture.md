# CLI Architecture

The CLI is a small .NET 8 executable targeting Windows x64 first.

It is framework-dependent because the target machines are expected to have the .NET 8 runtime. This keeps artifacts smaller than self-contained publishing while preserving straightforward deployment.

## Layers

```text
Commands
  -> application services
     -> providers
        -> filesystem / git / azure devops / sql / secrets / updates
```

The command tree is declared with `System.CommandLine`. Commands, subcommands, arguments, options, descriptions, help, and shell suggestions must come from that tree instead of ad-hoc command parsing or hardcoded usage text.

Handlers receive typed option records or explicit values built from `ParseResult`. They must not redispatch subcommands with local `switch` statements or parse positional values manually.

## Command Groups

- `doctor`: environment diagnostics and guided remediation
- `init`: root bootstrap and template generation
- `agent`: stable context for AI agents
- `task`: task workspace lifecycle
- `auth`: Azure DevOps authentication
- `db`: read-only SQL Server access
- `update`: GitHub Releases update flow

## Versioning

The intended version format is:

```text
YYYY.MM.DD.BUILD+COMMIT
```

The date must be supplied by the release process, not computed during reproducible builds.
