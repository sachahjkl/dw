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

## Target Commands

```text
dw db schema
dw db describe <table>
dw db query <sql>
```

Later commands can include table/column search helpers.
