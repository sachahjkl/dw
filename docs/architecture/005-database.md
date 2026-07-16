# Database Module

`dw db` exists so agents can inspect SQL Server safely.

The default must be read-only.

## Default Safety

- `readonly: true`
- `maxRows: 500`
- `timeoutSeconds: 600`
- block destructive statements by default
- prefer a read-only SQL account when available

Blocked by default:

```text
INSERT
UPDATE
DELETE
MERGE
DROP
ALTER
TRUNCATE
EXEC
```

## Commands

```text
dw db list
dw db collect [--save]
dw db schema
dw db describe <table>
dw db query <sql>
```

`db collect` scans `appsettings*.json` files in configured workspace repositories. Preview output
always masks connection values. `--save` accepts only concrete SQL Server connection strings,
stores them in the system keyring, and writes credential references to `databases.json`.

Collection is deterministic and conservative: duplicate values are merged, conflicting values are
not saved, and existing configuration or keyring values are never overwritten with different data.
