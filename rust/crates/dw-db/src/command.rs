use anyhow::Result;
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
        table: String,
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
        sql: String,
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
        } => commands::query(commands::QueryArgs {
            sql,
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

#[cfg(test)]
mod tests {
    use super::parse_positive_usize;

    #[test]
    fn max_rows_rejects_zero_and_non_numeric_values() {
        assert_eq!(parse_positive_usize("25").expect("valid"), 25);
        assert!(parse_positive_usize("0").is_err());
        assert!(parse_positive_usize("wat").is_err());
    }
}
