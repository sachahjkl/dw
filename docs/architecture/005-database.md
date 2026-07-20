# Data Providers

`dw data` lets agents inspect configured sources through provider-neutral capabilities. SQL Server is the current implementation, not part of the public command namespace.

Each source in `databases.json` names a `provider`. Commands use that provider unless an applicable `--provider` override is supplied. A provider may implement discovery, catalog, description, native query, tabular, workbook, document, read-policy, and credential capabilities independently. This model allows future SQLite, Excel, and NoSQL providers without adding source-specific routes.

## Default Safety

SQL Server sources default to:

- `readonly: true`
- `maxRows: 500`
- `timeoutSeconds: 600`
- destructive statements blocked
- a read-only account when available

Blocked SQL statements include `INSERT`, `UPDATE`, `DELETE`, `MERGE`, `DROP`, `ALTER`, `TRUNCATE`, and `EXEC`. `dw data guard --query <query> [--provider <provider>]` requests the selected provider's read-policy capability; generic command orchestration does not instantiate SQL Server directly.

## Commands

```text
dw data source list [--provider <provider>]
dw data source collect [--provider <provider>] [--save]
dw data guard --query <query> [--provider <provider>]
dw data catalog [--source <source> | --env <environment>] [--provider <provider>]
dw data describe [RESOURCE] [--source <source> | --env <environment>] [--provider <provider>]
dw data query [QUERY...] [--query <query>] [--source <source> | --env <environment>] [--provider <provider>]
```

Source collection resolves workspace repository roots generically, then invokes the selected provider's `discoverer` capability. SQL Server recognizes ASP.NET `appsettings*.json` connection strings inside its provider package. Preview output masks credentials. Generic application orchestration stores accepted secret material in the system keyring and writes provider/credential references to configuration; it never imports or branches on SQL Server.

Collection is deterministic and conservative: duplicate values are merged, conflicting values are not saved, and existing configuration or keyring values are never overwritten with different data. `databases.json` requires a non-empty provider name and permits provider-specific extension fields. Machine output retains the established ordered JSON, TSV null, truncation, and guard contracts regardless of provider.
