# Command Specification

This document defines the public namespace contract. Command paths are generic; concrete product names belong in provider configuration and provider-specific output only.

## External Work

```text
dw work item list [--provider <provider>]
dw work item show <work-item>... [--provider <provider>]
dw work item doing <work-item>... [--provider <provider>]
dw work item state set <state> <work-item>... [--provider <provider>]
dw work item child create ... [--provider <provider>]
dw work pr list [--provider <provider>]
dw work context show <work-item>... [--provider <provider>]
dw work context ai <work-item>... [--provider <provider>]
dw work changelog ... [--provider <provider>]
```

These commands resolve an optional `--provider` first, then the configured project work provider. They request typed capabilities such as item reads, state writes, child creation, pull-request reads, or rich context. Unsupported operations return a provider capability error rather than switching providers or invoking product-specific code.

## Local Workspaces

```text
dw workspace status
dw workspace list
dw workspace current
dw workspace open ...
dw workspace start ...
dw workspace pr start ...
dw workspace preflight ...
dw workspace sync ...
dw workspace rename ...
dw workspace repo add ...
dw workspace repo latest ...
dw workspace item add ...
dw workspace item remove ...
dw workspace commit ...
dw workspace finish ...
dw workspace handoff validate ...
dw workspace teardown ...
dw workspace prune ...
```

`workspace` owns local filesystem, Git repository, worktree, manifest, preflight, commit, handoff, and teardown lifecycle. Provider calls needed by start or finish use the configured project work provider without changing the local namespace.

## Data Sources

```text
dw data source list [--provider <provider>]
dw data source collect [--provider <provider>] [--save]
dw data guard --query <query> [--provider <provider>]
dw data catalog [--source <source> | --env <environment>] [--provider <provider>]
dw data describe [RESOURCE] [--source <source> | --env <environment>] [--provider <provider>]
dw data query [QUERY...] [--query <query>] [--source <source> | --env <environment>] [--provider <provider>]
```

Each configured source names a provider. An explicit `--provider` may narrow source listing and select a provider for discovery, guard policy, catalog, description, or query operations. Provider capabilities decide whether a source supports native queries, tabular reads, workbooks, documents, catalogs, descriptions, or a read policy; orchestration never constructs a concrete SQL implementation directly.

## Provider Administration

```text
dw provider list
dw provider show <provider>
dw provider capabilities <provider>
dw provider auth login <provider>
dw provider auth status <provider>
dw provider auth logout <provider>
```

Provider names are positional for administration and authentication. List, show, and capabilities reports are derived from the ordered work and data registries. A provider registered for both kinds appears once with kinds ordered `work`, then `data`; capability names are deterministic. Authentication requires the selected provider to implement the work authentication capability.

Azure DevOps is the current work provider. GitHub and Jira are expected future work providers. SQL Server is the current data provider; SQLite, Excel, and NoSQL providers can supply narrower capability sets without changing these paths.

## Other Commands

`version`, `guide`, `doctor`, `init`, `refresh`, `tui`, `agent`, `completion`, `config`, `secret`, and `upgrade` retain their existing behavior and output contracts. Help and completion are generated from the authoritative command grammar.

The namespace cutover is clean: removed namespaces and misplaced lifecycle commands are ordinary unknown commands. There are no aliases, deprecated routes, or special rejection shims.
