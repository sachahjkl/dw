use crate::{
    DatabaseSelection, QueryResult, describe_table_sql, query_sql_server, resolve_connection,
    schema_sql, validate_read_only_sql,
};
use anyhow::Result;
use dw_config::{load_databases_config, resolve_root};
use dw_core::{
    DatabaseEnvironmentName, DatabaseKey, DatabaseTableName, DbActionEvent, ProjectKey, SqlQuery,
};

#[derive(Debug, Clone)]
pub struct GuardArgs {
    pub sql: SqlQuery,
}

#[derive(Debug, Clone)]
pub struct SchemaArgs {
    pub project: Option<ProjectKey>,
    pub database: Option<DatabaseKey>,
    pub env: Option<DatabaseEnvironmentName>,
}

#[derive(Debug, Clone)]
pub struct DescribeArgs {
    pub table: Option<DatabaseTableName>,
    pub project: Option<ProjectKey>,
    pub database: Option<DatabaseKey>,
    pub env: Option<DatabaseEnvironmentName>,
}

#[derive(Debug, Clone)]
pub struct QueryArgs {
    pub sql: SqlQuery,
    pub project: Option<ProjectKey>,
    pub database: Option<DatabaseKey>,
    pub env: Option<DatabaseEnvironmentName>,
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
            DbQueryKind::Schema => "schema",
            DbQueryKind::Describe => "description",
            DbQueryKind::Query => "query",
        }
    }
}

pub fn guard_with_events(
    args: GuardArgs,
    mut emit: impl FnMut(DbActionEvent),
) -> crate::SqlGuardResult {
    emit(DbActionEvent::GuardingQuery);
    validate_read_only_sql(args.sql.as_str())
}

pub async fn schema_with_events(
    args: SchemaArgs,
    mut emit: impl FnMut(DbActionEvent),
) -> Result<QueryResult> {
    execute_db_query(
        args.project.as_ref(),
        args.database.as_ref(),
        args.env.as_ref(),
        schema_sql(),
        Some(0),
        DbQueryKind::Schema,
        &mut emit,
    )
    .await
}

pub async fn describe_with_events(
    args: DescribeArgs,
    mut emit: impl FnMut(DbActionEvent),
) -> Result<Option<QueryResult>> {
    let Some(table) = resolve_describe_table(
        args.table,
        args.project.as_ref(),
        args.database.as_ref(),
        args.env.as_ref(),
    )
    .await?
    else {
        return Ok(None);
    };
    let sql = describe_table_sql(table.as_str());
    Ok(Some(
        execute_db_query(
            args.project.as_ref(),
            args.database.as_ref(),
            args.env.as_ref(),
            &sql,
            Some(0),
            DbQueryKind::Describe,
            &mut emit,
        )
        .await?,
    ))
}

pub async fn query_with_events(
    args: QueryArgs,
    mut emit: impl FnMut(DbActionEvent),
) -> Result<QueryResult> {
    execute_db_query(
        args.project.as_ref(),
        args.database.as_ref(),
        args.env.as_ref(),
        args.sql.as_str(),
        args.max_rows,
        DbQueryKind::Query,
        &mut emit,
    )
    .await
}

async fn execute_db_query(
    project: Option<&ProjectKey>,
    database: Option<&DatabaseKey>,
    env: Option<&DatabaseEnvironmentName>,
    sql: &str,
    max_rows_override: Option<usize>,
    kind: DbQueryKind,
    emit: &mut impl FnMut(DbActionEvent),
) -> Result<QueryResult> {
    emit(DbActionEvent::GuardingQuery);
    let guard = validate_read_only_sql(sql);
    if !guard.is_allowed {
        return Err(anyhow::anyhow!(
            "Query blocked: {}",
            guard
                .reason
                .as_ref()
                .map(|reason| reason.as_str())
                .unwrap_or("unknown reason")
        ));
    }

    let root = resolve_root(None);
    let config = load_databases_config(&root);
    let env_database = env.map(|value| DatabaseKey::from(value.as_str()));
    let (project, database) =
        resolve_database_selection(&config, project, database.or(env_database.as_ref()))?;
    emit(DbActionEvent::ResolvingConnection {
        database: Some(database.clone()),
    });
    let resolved = resolve_connection(
        &config,
        DatabaseSelection {
            project: &project,
            database: &database,
        },
    )
    .map_err(anyhow::Error::from)?;
    let _ = kind.label();
    emit(DbActionEvent::ExecutingReadOnlyQuery {
        max_rows: max_rows_override,
    });
    query_sql_server(
        &resolved.connection,
        &resolved.defaults,
        sql,
        max_rows_override,
    )
    .await
    .map_err(anyhow::Error::from)
}

async fn resolve_describe_table(
    table: Option<DatabaseTableName>,
    project: Option<&ProjectKey>,
    database: Option<&DatabaseKey>,
    env: Option<&DatabaseEnvironmentName>,
) -> Result<Option<DatabaseTableName>> {
    if let Some(table) = table.filter(|value| !value.as_str().trim().is_empty()) {
        return Ok(Some(table));
    }
    let _ = (project, database, env);
    Err(anyhow::anyhow!(
        "Missing table. Provide a table name to describe."
    ))
}

fn resolve_database_selection(
    _config: &dw_config::DatabasesConfig,
    project: Option<&ProjectKey>,
    database: Option<&DatabaseKey>,
) -> Result<(ProjectKey, DatabaseKey)> {
    let project = match project {
        Some(project) => project.clone(),
        None => ProjectKey::from("default"),
    };
    let database = match database {
        Some(database) => database.clone(),
        None => DatabaseKey::from("dev"),
    };

    Ok((project, database))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_args_build_describe_sql() {
        let args = DescribeArgs {
            table: Some(DatabaseTableName::from("audit.Events")),
            project: None,
            database: None,
            env: None,
        };

        let sql = describe_table_sql(args.table.as_ref().expect("table").as_str());

        assert!(sql.contains("TABLE_SCHEMA = 'audit'"));
        assert!(sql.contains("TABLE_NAME = 'Events'"));
    }

    #[test]
    fn db_query_kind_labels_are_user_facing() {
        assert_eq!(DbQueryKind::Schema.label(), "schema");
        assert_eq!(DbQueryKind::Describe.label(), "description");
        assert_eq!(DbQueryKind::Query.label(), "query");
    }
}
