use crate::{
    DatabaseSelection, QueryResult, describe_table_sql, query_sql_server,
    render_query_result_table, render_query_result_tsv, render_sql_guard, resolve_connection,
    schema_sql, validate_read_only_sql,
};
use anyhow::Result;
use dw_config::{load_databases_config, resolve_root};
use dw_ui::{TerminalTheme, confirm_when_interactive, is_stdin_interactive, select_optional};
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
    pub table: Option<String>,
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
    let table = resolve_describe_table(
        args.table,
        args.project.as_deref(),
        args.database.as_deref(),
        args.env.as_deref(),
    )?;
    let sql = describe_table_sql(&table);
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

fn resolve_describe_table(
    table: Option<String>,
    project: Option<&str>,
    database: Option<&str>,
    env: Option<&str>,
) -> Result<String> {
    if let Some(table) = table.filter(|value| !value.trim().is_empty()) {
        return Ok(table);
    }
    if !is_stdin_interactive() {
        return Err(anyhow::anyhow!(
            "Table manquante. Fournir `dw db describe <table>`."
        ));
    }
    if !confirm_when_interactive("Charger la liste des tables pour choisir une table à décrire ?")?
    {
        return Err(anyhow::anyhow!("Description DB annulée."));
    }

    let schema = execute_db_query(project, database, env, schema_sql(), Some(0))?;
    let choices = schema_table_choices(&schema);
    select_optional("Table SQL", choices)?
        .ok_or_else(|| anyhow::anyhow!("Aucune table disponible pour la sélection."))
}

fn schema_table_choices(result: &QueryResult) -> Vec<String> {
    result
        .rows
        .iter()
        .filter_map(|row| {
            let schema = row.first().and_then(|value| value.as_deref())?;
            let table = row.get(1).and_then(|value| value.as_deref())?;
            Some(format!("{schema}.{table}"))
        })
        .collect()
}

fn resolve_database_selection(
    config: &dw_config::DatabasesConfig,
    project: Option<&str>,
    database: Option<&str>,
) -> Result<(String, String)> {
    let project = match project {
        Some(project) => project.to_string(),
        None => select_optional("Projet DB", configured_database_projects(config))?
            .unwrap_or_else(|| "default".into()),
    };
    let database = match database {
        Some(database) => database.to_string(),
        None => select_optional(
            "Connexion DB",
            configured_databases_for_project(config, &project),
        )?
        .unwrap_or_else(|| "dev".into()),
    };

    Ok((project, database))
}

fn configured_database_projects(config: &dw_config::DatabasesConfig) -> Vec<String> {
    let mut projects = config
        .projects
        .iter()
        .filter_map(|(key, value)| {
            value
                .get("databases")
                .and_then(serde_json::Value::as_object)
                .filter(|databases| !databases.is_empty())
                .map(|_| key.clone())
        })
        .collect::<Vec<_>>();
    if !config.globals.is_empty() && !projects.iter().any(|project| project == "default") {
        projects.insert(0, "default".into());
    }
    projects
}

fn configured_databases_for_project(
    config: &dw_config::DatabasesConfig,
    project: &str,
) -> Vec<String> {
    let mut databases = config
        .projects
        .get(project)
        .and_then(|value| value.get("databases"))
        .and_then(serde_json::Value::as_object)
        .map(|databases| databases.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    for key in config.globals.keys() {
        if !databases.iter().any(|candidate| candidate == key) {
            databases.push(key.clone());
        }
    }
    databases
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
            json: false,
        };

        let sql = describe_table_sql(args.table.as_deref().expect("table"));

        assert!(sql.contains("TABLE_SCHEMA = 'audit'"));
        assert!(sql.contains("TABLE_NAME = 'Events'"));
    }

    #[test]
    fn schema_table_choices_reads_schema_and_name_columns() {
        let result = QueryResult {
            columns: vec![
                "TABLE_SCHEMA".into(),
                "TABLE_NAME".into(),
                "TABLE_TYPE".into(),
            ],
            rows: vec![
                vec![
                    Some("dbo".into()),
                    Some("Users".into()),
                    Some("BASE TABLE".into()),
                ],
                vec![
                    Some("audit".into()),
                    Some("Events".into()),
                    Some("VIEW".into()),
                ],
            ],
            truncated: false,
        };

        assert_eq!(
            schema_table_choices(&result),
            vec!["dbo.Users", "audit.Events"]
        );
    }

    #[test]
    fn configured_database_choices_include_project_and_globals() {
        let config = dw_config::DatabasesConfig {
            defaults: None,
            globals: serde_json::from_value(serde_json::json!({
                "shared": {"provider":"sqlserver","connectionString":"Server=s;Database=d;ApplicationIntent=ReadOnly","readonly":true}
            }))
            .expect("globals object"),
            projects: serde_json::from_value(serde_json::json!({
                "ha": {"databases": {"ha-dev": {"provider":"sqlserver","connectionString":"Server=s;Database=d;ApplicationIntent=ReadOnly","readonly":true}}}
            }))
            .expect("projects object"),
        };

        assert_eq!(configured_database_projects(&config), vec!["default", "ha"]);
        assert_eq!(
            configured_databases_for_project(&config, "ha"),
            vec!["ha-dev", "shared"]
        );
    }
}
