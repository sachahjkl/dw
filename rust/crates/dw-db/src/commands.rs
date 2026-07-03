use crate::{
    DatabaseSelection, QueryResult, describe_table_sql, query_sql_server,
    render_query_result_table, render_query_result_tsv, render_sql_guard, resolve_connection,
    schema_sql, validate_read_only_sql,
};
use anyhow::Result;
use dw_config::{load_databases_config, resolve_root};
use dw_ui::TerminalTheme;
use std::io::IsTerminal;

#[derive(Debug, Clone)]
pub struct GuardArgs {
    pub sql: String,
}

#[derive(Debug, Clone)]
pub struct SchemaArgs {
    pub project: Option<String>,
    pub database: Option<String>,
    pub env: Option<String>,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct DescribeArgs {
    pub table: String,
    pub project: Option<String>,
    pub database: Option<String>,
    pub env: Option<String>,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct QueryArgs {
    pub sql: String,
    pub project: Option<String>,
    pub database: Option<String>,
    pub env: Option<String>,
    pub max_rows: Option<usize>,
    pub json: bool,
}

pub fn guard(args: GuardArgs) {
    let result = validate_read_only_sql(&args.sql);
    println!(
        "{}",
        render_sql_guard(&result, &TerminalTheme::stdout_auto())
    );
}

pub fn schema(args: SchemaArgs) -> Result<()> {
    let result = execute_db_query(
        args.project.as_deref(),
        args.database.as_deref(),
        args.env.as_deref(),
        schema_sql(),
        Some(0),
    )?;
    print_db_result(&result, args.json)
}

pub fn describe(args: DescribeArgs) -> Result<()> {
    let sql = describe_table_sql(&args.table);
    let result = execute_db_query(
        args.project.as_deref(),
        args.database.as_deref(),
        args.env.as_deref(),
        &sql,
        Some(0),
    )?;
    print_db_result(&result, args.json)
}

pub fn query(args: QueryArgs) -> Result<()> {
    let result = execute_db_query(
        args.project.as_deref(),
        args.database.as_deref(),
        args.env.as_deref(),
        &args.sql,
        args.max_rows,
    )?;
    print_db_result(&result, args.json)
}

fn execute_db_query(
    project: Option<&str>,
    database: Option<&str>,
    env: Option<&str>,
    sql: &str,
    max_rows_override: Option<usize>,
) -> Result<QueryResult> {
    let guard = validate_read_only_sql(sql);
    if !guard.is_allowed {
        return Err(anyhow::anyhow!(
            "Requête bloquée: {}",
            guard.reason.unwrap_or_else(|| "raison inconnue".into())
        ));
    }

    let project = project.unwrap_or("default");
    let database = database.or(env).unwrap_or("dev");
    let root = resolve_root(None);
    let config = load_databases_config(&root);
    let resolved = resolve_connection(&config, DatabaseSelection { project, database })
        .map_err(anyhow::Error::msg)?;
    query_sql_server(
        &resolved.connection,
        &resolved.defaults,
        sql,
        max_rows_override,
    )
    .map_err(anyhow::Error::msg)
}

fn print_db_result(result: &QueryResult, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(result)?);
    } else if std::io::stdout().is_terminal() {
        println!(
            "{}",
            render_query_result_table(result, &TerminalTheme::stdout_auto())
        );
    } else {
        println!("{}", render_query_result_tsv(result));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_args_build_describe_sql() {
        let args = DescribeArgs {
            table: "audit.Events".into(),
            project: None,
            database: None,
            env: None,
            json: false,
        };

        let sql = describe_table_sql(&args.table);

        assert!(sql.contains("TABLE_SCHEMA = 'audit'"));
        assert!(sql.contains("TABLE_NAME = 'Events'"));
    }
}
