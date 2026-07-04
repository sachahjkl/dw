use crate::{
    DatabaseSelection, QueryResult, describe_table_sql, query_sql_server, resolve_connection,
    schema_sql, validate_read_only_sql,
};
use anyhow::Result;
use dw_config::{load_databases_config, resolve_root};

#[derive(Debug, Clone)]
pub struct GuardArgs {
    pub sql: String,
}

#[derive(Debug, Clone)]
pub struct SchemaArgs {
    pub project: Option<String>,
    pub database: Option<String>,
    pub env: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DescribeArgs {
    pub table: Option<String>,
    pub project: Option<String>,
    pub database: Option<String>,
    pub env: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QueryArgs {
    pub sql: String,
    pub project: Option<String>,
    pub database: Option<String>,
    pub env: Option<String>,
    pub max_rows: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DbQueryKind {
    Schema,
    Describe,
    Query,
}

impl DbQueryKind {
    fn label(self) -> &'static str {
        match self {
            DbQueryKind::Schema => "schéma",
            DbQueryKind::Describe => "description",
            DbQueryKind::Query => "requête",
        }
    }
}

pub fn guard(args: GuardArgs) -> crate::SqlGuardResult {
    validate_read_only_sql(&args.sql)
}

pub async fn schema(args: SchemaArgs) -> Result<QueryResult> {
    execute_db_query(
        args.project.as_deref(),
        args.database.as_deref(),
        args.env.as_deref(),
        schema_sql(),
        Some(0),
        DbQueryKind::Schema,
    )
    .await
}

pub async fn describe(args: DescribeArgs) -> Result<Option<QueryResult>> {
    let Some(table) = resolve_describe_table(
        args.table,
        args.project.as_deref(),
        args.database.as_deref(),
        args.env.as_deref(),
    )
    .await?
    else {
        return Ok(None);
    };
    let sql = describe_table_sql(&table);
    Ok(Some(
        execute_db_query(
            args.project.as_deref(),
            args.database.as_deref(),
            args.env.as_deref(),
            &sql,
            Some(0),
            DbQueryKind::Describe,
        )
        .await?,
    ))
}

pub async fn query(args: QueryArgs) -> Result<QueryResult> {
    execute_db_query(
        args.project.as_deref(),
        args.database.as_deref(),
        args.env.as_deref(),
        &args.sql,
        args.max_rows,
        DbQueryKind::Query,
    )
    .await
}

async fn execute_db_query(
    project: Option<&str>,
    database: Option<&str>,
    env: Option<&str>,
    sql: &str,
    max_rows_override: Option<usize>,
    kind: DbQueryKind,
) -> Result<QueryResult> {
    let guard = validate_read_only_sql(sql);
    if !guard.is_allowed {
        return Err(anyhow::anyhow!(
            "Requête bloquée: {}",
            guard.reason.unwrap_or_else(|| "raison inconnue".into())
        ));
    }

    let root = resolve_root(None);
    let config = load_databases_config(&root);
    let (project, database) = resolve_database_selection(&config, project, database.or(env))?;
    let resolved = resolve_connection(
        &config,
        DatabaseSelection {
            project: &project,
            database: &database,
        },
    )
    .map_err(anyhow::Error::msg)?;
    let _ = kind.label();
    query_sql_server(
        &resolved.connection,
        &resolved.defaults,
        sql,
        max_rows_override,
    )
    .await
    .map_err(anyhow::Error::msg)
}

async fn resolve_describe_table(
    table: Option<String>,
    project: Option<&str>,
    database: Option<&str>,
    env: Option<&str>,
) -> Result<Option<String>> {
    if let Some(table) = table.filter(|value| !value.trim().is_empty()) {
        return Ok(Some(table));
    }
    let _ = (project, database, env);
    Err(anyhow::anyhow!(
        "Table manquante. Fournir un nom de table à décrire."
    ))
}

fn resolve_database_selection(
    _config: &dw_config::DatabasesConfig,
    project: Option<&str>,
    database: Option<&str>,
) -> Result<(String, String)> {
    let project = match project {
        Some(project) => project.to_string(),
        None => "default".into(),
    };
    let database = match database {
        Some(database) => database.to_string(),
        None => "dev".into(),
    };

    Ok((project, database))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_args_build_describe_sql() {
        let args = DescribeArgs {
            table: Some("audit.Events".into()),
            project: None,
            database: None,
            env: None,
        };

        let sql = describe_table_sql(args.table.as_deref().expect("table"));

        assert!(sql.contains("TABLE_SCHEMA = 'audit'"));
        assert!(sql.contains("TABLE_NAME = 'Events'"));
    }

    #[test]
    fn db_query_kind_labels_are_user_facing() {
        assert_eq!(DbQueryKind::Schema.label(), "schéma");
        assert_eq!(DbQueryKind::Describe.label(), "description");
        assert_eq!(DbQueryKind::Query.label(), "requête");
    }
}
