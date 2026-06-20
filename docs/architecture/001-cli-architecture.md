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

The current implementation starts with a minimal internal command parser to avoid external NuGet dependencies during bootstrap. This can be replaced later with `System.CommandLine` if the command surface becomes complex enough.

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
