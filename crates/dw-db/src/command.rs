use anyhow::{Result, anyhow};
use clap::Subcommand;

use crate::commands;

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    #[command(about = "Vérifie qu'une requête SQL respecte le mode read-only.")]
    Guard {
        #[arg(long, help = "Requête SQL à analyser sans exécution.")]
        sql: String,
    },
    #[command(about = "Liste les tables et vues accessibles sur une base configurée.")]
    Schema {
        #[arg(long, help = "Projet configuré contenant la connexion base.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Nom de connexion déclaré dans databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Alias d'environnement base déclaré dans databases.json."
        )]
        env: Option<String>,
        #[arg(long, help = "Émettre le résultat JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Décrit les colonnes d'une table SQL.")]
    Describe {
        #[arg(help = "Table à décrire, au format table ou schema.table.")]
        table: Option<String>,
        #[arg(long, help = "Projet configuré contenant la connexion base.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Nom de connexion déclaré dans databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Alias d'environnement base déclaré dans databases.json."
        )]
        env: Option<String>,
        #[arg(long, help = "Émettre le résultat JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Exécute une requête SQL read-only avec garde-fous et limite de lignes.")]
    Query {
        #[arg(long, help = "Requête SQL read-only à exécuter.")]
        sql: Option<String>,
        #[arg(long, help = "Projet configuré contenant la connexion base.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Nom de connexion déclaré dans databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Alias d'environnement base déclaré dans databases.json."
        )]
        env: Option<String>,
        #[arg(long = "max-rows", help = "Nombre maximum de lignes à afficher.")]
        #[arg(value_parser = parse_positive_usize)]
        max_rows: Option<usize>,
        #[arg(long, help = "Émettre le résultat JSON déterministe.")]
        json: bool,
        #[arg(
            value_name = "SQL",
            trailing_var_arg = true,
            help = "Requête SQL read-only à exécuter."
        )]
        sql_parts: Vec<String>,
    },
}

pub fn handle_db(command: DbCommand) -> Result<()> {
    match command {
        DbCommand::Guard { sql } => {
            commands::guard(commands::GuardArgs { sql });
        }
        DbCommand::Schema {
            project,
            database,
            env,
            json,
        } => commands::schema(commands::SchemaArgs {
            project,
            database,
            env,
            json,
        })?,
        DbCommand::Describe {
            project,
            database,
            env,
            table,
            json,
        } => commands::describe(commands::DescribeArgs {
            table,
            project,
            database,
            env,
            json,
        })?,
        DbCommand::Query {
            project,
            database,
            env,
            sql,
            max_rows,
            json,
            sql_parts,
        } => commands::query(commands::QueryArgs {
            sql: resolve_query_sql(sql, sql_parts)?,
            project,
            database,
            env,
            max_rows,
            json,
        })?,
    }

    Ok(())
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "--max-rows doit être un entier positif.".to_string())?;
    if parsed == 0 {
        return Err("--max-rows doit être supérieur à 0.".into());
    }
    Ok(parsed)
}

fn resolve_query_sql(sql: Option<String>, sql_parts: Vec<String>) -> Result<String> {
    let sql = sql
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let positional = sql_parts.join(" ");
    let positional = positional.trim();

    match (sql, positional.is_empty()) {
        (Some(_), false) => Err(anyhow!(
            "Utiliser soit --sql, soit la requête positionnelle, pas les deux."
        )),
        (Some(sql), true) => Ok(sql),
        (None, false) => Ok(positional.to_string()),
        (None, true) => Err(anyhow!(
            "Requête SQL manquante. Fournir --sql \"select ...\" ou une requête positionnelle."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_positive_usize, resolve_query_sql};

    #[test]
    fn max_rows_rejects_zero_and_non_numeric_values() {
        assert_eq!(parse_positive_usize("25").expect("valid"), 25);
        assert!(parse_positive_usize("0").is_err());
        assert!(parse_positive_usize("wat").is_err());
    }

    #[test]
    fn resolve_query_sql_uses_long_option() {
        let sql = resolve_query_sql(Some(" select 1 ".into()), Vec::new()).expect("valid sql");

        assert_eq!(sql, "select 1");
    }

    #[test]
    fn resolve_query_sql_joins_positional_parts() {
        let sql = resolve_query_sql(None, vec!["select".into(), "1".into()]).expect("valid sql");

        assert_eq!(sql, "select 1");
    }

    #[test]
    fn resolve_query_sql_rejects_both_sources() {
        let err = resolve_query_sql(Some("select 1".into()), vec!["select".into(), "2".into()])
            .expect_err("exclusive sql sources");

        assert!(err.to_string().contains("soit --sql"));
    }

    #[test]
    fn resolve_query_sql_rejects_missing_sql() {
        let err = resolve_query_sql(Some("   ".into()), Vec::new()).expect_err("missing sql");

        assert!(err.to_string().contains("Requête SQL manquante"));
    }
}
