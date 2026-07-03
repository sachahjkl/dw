use crate::cli::DbCommand;
use anyhow::Result;
use dw_config::resolve_root;
use dw_db::{
    DatabaseSelection, describe_table_sql, query_sql_server, render_query_result_tsv,
    resolve_connection as resolve_db_connection, schema_sql, validate_read_only_sql,
};

pub(crate) fn handle_db(command: DbCommand) -> Result<()> {
    match command {
        DbCommand::Guard { sql } => {
            let result = validate_read_only_sql(&sql);
            if result.is_allowed {
                println!("SQL autorisee.");
            } else {
                println!(
                    "SQL bloquee: {}",
                    result.reason.unwrap_or_else(|| "raison inconnue".into())
                );
            }
        }
        DbCommand::Schema { project, json } => {
            let result = execute_db_query(project.as_deref(), None, None, schema_sql(), Some(0))?;
            print_db_result(&result, json)?;
        }
        DbCommand::Describe {
            project,
            database,
            table,
            json,
        } => {
            let sql = describe_table_sql(&table);
            let result =
                execute_db_query(project.as_deref(), database.as_deref(), None, &sql, Some(0))?;
            print_db_result(&result, json)?;
        }
        DbCommand::Query {
            project,
            database,
            env,
            sql,
            json,
        } => {
            let result = execute_db_query(
                project.as_deref(),
                database.as_deref(),
                env.as_deref(),
                &sql,
                None,
            )?;
            print_db_result(&result, json)?;
        }
    }
    Ok(())
}

fn execute_db_query(
    project: Option<&str>,
    database: Option<&str>,
    env: Option<&str>,
    sql: &str,
    max_rows_override: Option<usize>,
) -> Result<dw_db::QueryResult> {
    let guard = validate_read_only_sql(sql);
    if !guard.is_allowed {
        return Err(anyhow::anyhow!(
            "Requete bloquee: {}",
            guard.reason.unwrap_or_else(|| "raison inconnue".into())
        ));
    }
    let project = project.unwrap_or("default");
    let database = database.or(env).unwrap_or("dev");
    let root = resolve_root(None);
    let config = dw_config::load_databases_config(&root);
    let resolved = resolve_db_connection(&config, DatabaseSelection { project, database })
        .map_err(anyhow::Error::msg)?;
    query_sql_server(
        &resolved.connection,
        &resolved.defaults,
        sql,
        max_rows_override,
    )
    .map_err(anyhow::Error::msg)
}

fn print_db_result(result: &dw_db::QueryResult, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(result)?);
    } else {
        println!("{}", render_query_result_tsv(result));
    }
    Ok(())
}
