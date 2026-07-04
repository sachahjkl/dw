use crate::config::{DatabaseConnectionConfig, DatabaseDefaults};
use crate::guard::validate_read_only_sql;
use dw_secret::{KeyringSecretStore, SecretStore};
use serde::Serialize;
use std::time::Duration;
use tiberius::{Client, ColumnData, Config, Row};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub truncated: bool,
}

pub fn schema_sql() -> &'static str {
    "select TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE
from INFORMATION_SCHEMA.TABLES
order by TABLE_SCHEMA, TABLE_NAME"
}

pub fn describe_table_sql(table: &str) -> String {
    let (schema, name) = table.split_once('.').unwrap_or(("dbo", table));
    format!(
        "select COLUMN_NAME, DATA_TYPE, IS_NULLABLE, CHARACTER_MAXIMUM_LENGTH
from INFORMATION_SCHEMA.COLUMNS
where TABLE_SCHEMA = '{}'
  and TABLE_NAME = '{}'
order by ORDINAL_POSITION",
        escape_sql_literal(schema),
        escape_sql_literal(name)
    )
}

pub fn resolve_connection_string(connection: &DatabaseConnectionConfig) -> Result<String, String> {
    resolve_connection_string_with_store(connection, &KeyringSecretStore)
}

pub fn resolve_connection_string_with_store(
    connection: &DatabaseConnectionConfig,
    store: &impl SecretStore,
) -> Result<String, String> {
    if let Some(value) = connection
        .connection_string
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(value.to_string());
    }

    if let Some(variable) = connection
        .connection_string_environment_variable
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        && let Ok(value) = std::env::var(variable)
        && !value.trim().is_empty()
    {
        return Ok(value);
    }

    if let Some(key) = connection
        .credential_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return store
            .get(key)
            .map_err(|error| error.to_string())?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("Secret SQL introuvable: {key}"));
    }

    Err("Connection string SQL introuvable. Renseigner connectionString, connectionStringEnvironmentVariable ou credentialKey.".into())
}

pub async fn query_sql_server(
    connection: &DatabaseConnectionConfig,
    defaults: &DatabaseDefaults,
    sql: &str,
    max_rows_override: Option<usize>,
) -> Result<QueryResult, String> {
    if !connection.provider.eq_ignore_ascii_case("sqlserver") {
        return Err(format!("Provider DB non supporté: {}", connection.provider));
    }
    let guard = validate_read_only_sql(sql);
    if !guard.is_allowed {
        return Err(format!(
            "Requête bloquée: {}",
            guard.reason.unwrap_or_else(|| "raison inconnue".into())
        ));
    }

    let connection_string = resolve_connection_string(connection)?;
    let max_rows = max_rows_override
        .or(connection.max_rows)
        .unwrap_or(defaults.max_rows);
    let timeout_seconds = connection
        .timeout_seconds
        .unwrap_or(defaults.timeout_seconds)
        .max(1);
    query_sql_server_async(&connection_string, sql, max_rows, timeout_seconds).await
}

async fn query_sql_server_async(
    connection_string: &str,
    sql: &str,
    max_rows: usize,
    timeout_seconds: u64,
) -> Result<QueryResult, String> {
    timeout(
        Duration::from_secs(timeout_seconds),
        query_sql_server_async_inner(connection_string, sql, max_rows),
    )
    .await
    .map_err(|_| format!("Timeout SQL après {timeout_seconds}s."))?
}

async fn query_sql_server_async_inner(
    connection_string: &str,
    sql: &str,
    max_rows: usize,
) -> Result<QueryResult, String> {
    let mut config =
        Config::from_ado_string(connection_string).map_err(|error| error.to_string())?;
    config.readonly(true);
    config.trust_cert();
    let tcp = TcpStream::connect(config.get_addr())
        .await
        .map_err(|error| error.to_string())?;
    tcp.set_nodelay(true).map_err(|error| error.to_string())?;
    let mut client = Client::connect(config, tcp.compat_write())
        .await
        .map_err(|error| error.to_string())?;
    read_query_result(&mut client, sql, max_rows).await
}

async fn read_query_result(
    client: &mut Client<Compat<TcpStream>>,
    sql: &str,
    max_rows: usize,
) -> Result<QueryResult, String> {
    let result_sets = client
        .simple_query(sql)
        .await
        .map_err(|error| error.to_string())?
        .into_results()
        .await
        .map_err(|error| error.to_string())?;
    let first_result = result_sets.into_iter().next().unwrap_or_default();
    let columns = first_result.first().map(row_columns).unwrap_or_default();
    let mut rows = Vec::new();
    let mut truncated = false;

    for row in first_result {
        if max_rows > 0 && rows.len() >= max_rows {
            truncated = true;
            continue;
        }
        rows.push(
            row.cells()
                .map(|(_, value)| column_data_to_string(value))
                .collect(),
        );
    }

    Ok(QueryResult {
        columns,
        rows,
        truncated,
    })
}

fn row_columns(row: &Row) -> Vec<String> {
    row.columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect()
}

fn column_data_to_string(value: &ColumnData<'_>) -> Option<String> {
    match value {
        ColumnData::U8(value) => value.map(|value| value.to_string()),
        ColumnData::I16(value) => value.map(|value| value.to_string()),
        ColumnData::I32(value) => value.map(|value| value.to_string()),
        ColumnData::I64(value) => value.map(|value| value.to_string()),
        ColumnData::F32(value) => value.map(|value| value.to_string()),
        ColumnData::F64(value) => value.map(|value| value.to_string()),
        ColumnData::Bit(value) => value.map(|value| value.to_string()),
        ColumnData::String(value) => value.as_ref().map(|value| value.to_string()),
        ColumnData::Guid(value) => value.map(|value| value.to_string()),
        ColumnData::Binary(value) => value.as_ref().map(|value| hex_bytes(value)),
        ColumnData::Numeric(value) => value.as_ref().map(|value| value.to_string()),
        ColumnData::Xml(value) => value.as_ref().map(|value| value.to_string()),
        ColumnData::DateTime(value) => value.map(|value| format!("{value:?}")),
        ColumnData::SmallDateTime(value) => value.map(|value| format!("{value:?}")),
        ColumnData::Time(value) => value.map(|value| format!("{value:?}")),
        ColumnData::Date(value) => value.map(|value| format!("{value:?}")),
        ColumnData::DateTime2(value) => value.map(|value| format!("{value:?}")),
        ColumnData::DateTimeOffset(value) => value.map(|value| format!("{value:?}")),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::from("0x");
    for byte in bytes {
        output.push_str(&format!("{byte:02X}"));
    }
    output
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_table_defaults_schema_and_escapes_name() {
        let sql = describe_table_sql("Users'Oops");

        assert!(sql.contains("TABLE_SCHEMA = 'dbo'"));
        assert!(sql.contains("TABLE_NAME = 'Users''Oops'"));
    }

    #[test]
    fn describe_table_accepts_explicit_schema() {
        let sql = describe_table_sql("audit.Events");

        assert!(sql.contains("TABLE_SCHEMA = 'audit'"));
        assert!(sql.contains("TABLE_NAME = 'Events'"));
    }

    #[test]
    fn schema_sql_matches_dotnet_query_shape() {
        assert!(schema_sql().contains("INFORMATION_SCHEMA.TABLES"));
        assert!(schema_sql().contains("order by TABLE_SCHEMA, TABLE_NAME"));
    }

    #[test]
    fn resolve_connection_string_prefers_inline_value() {
        let connection = DatabaseConnectionConfig {
            provider: "sqlserver".into(),
            connection_string: Some("inline".into()),
            connection_string_environment_variable: Some("DW_TEST_DB".into()),
            credential_key: None,
            readonly: Some(true),
            max_rows: None,
            timeout_seconds: None,
        };

        assert_eq!(resolve_connection_string(&connection).unwrap(), "inline");
    }

    #[test]
    fn resolve_connection_string_reads_credential_key() {
        let store = dw_secret::MemorySecretStore::new();
        store
            .set("db/demo", "from-secret")
            .expect("secret should be stored");
        let connection = DatabaseConnectionConfig {
            provider: "sqlserver".into(),
            connection_string: None,
            connection_string_environment_variable: None,
            credential_key: Some("db/demo".into()),
            readonly: Some(true),
            max_rows: None,
            timeout_seconds: None,
        };

        assert_eq!(
            resolve_connection_string_with_store(&connection, &store).unwrap(),
            "from-secret"
        );
    }

    #[test]
    fn resolve_connection_string_reports_missing_credential_key() {
        let store = dw_secret::MemorySecretStore::new();
        let connection = DatabaseConnectionConfig {
            provider: "sqlserver".into(),
            connection_string: None,
            connection_string_environment_variable: None,
            credential_key: Some("db/missing".into()),
            readonly: Some(true),
            max_rows: None,
            timeout_seconds: None,
        };

        let error = resolve_connection_string_with_store(&connection, &store)
            .expect_err("missing secret should fail");
        assert!(error.contains("Secret SQL introuvable: db/missing"));
    }
}
