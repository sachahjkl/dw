use anyhow::Result;
use clap::Subcommand;

use crate::commands;

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    #[command(about = "Verifie qu'une requete SQL respecte le mode read-only.")]
    Guard {
        #[arg(long, help = "Requete SQL a analyser sans execution.")]
        sql: String,
    },
    #[command(about = "Liste les tables et vues accessibles sur une base configuree.")]
    Schema {
        #[arg(long, help = "Projet configure contenant la connexion base.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Nom de connexion declare dans databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Alias d'environnement base declare dans databases.json."
        )]
        env: Option<String>,
        #[arg(long, help = "Emettre le resultat JSON deterministe.")]
        json: bool,
    },
    #[command(about = "Decrit les colonnes d'une table SQL.")]
    Describe {
        #[arg(help = "Table a decrire, au format table ou schema.table.")]
        table: String,
        #[arg(long, help = "Projet configure contenant la connexion base.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Nom de connexion declare dans databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Alias d'environnement base declare dans databases.json."
        )]
        env: Option<String>,
        #[arg(long, help = "Emettre le resultat JSON deterministe.")]
        json: bool,
    },
    #[command(about = "Execute une requete SQL read-only avec garde-fous et limite de lignes.")]
    Query {
        #[arg(long, help = "Requete SQL read-only a executer.")]
        sql: String,
        #[arg(long, help = "Projet configure contenant la connexion base.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Nom de connexion declare dans databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Alias d'environnement base declare dans databases.json."
        )]
        env: Option<String>,
        #[arg(long = "max-rows", help = "Nombre maximum de lignes a afficher.")]
        max_rows: Option<usize>,
        #[arg(long, help = "Emettre le resultat JSON deterministe.")]
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
