use anyhow::Result;
use clap::Subcommand;

use crate::commands;

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    #[command(about = "Verifie qu'une requete SQL respecte le mode read-only.")]
    Guard {
        #[arg(long)]
        sql: String,
    },
    #[command(about = "Liste les tables et vues accessibles sur une base configuree.")]
    Schema {
        #[arg(long)]
        project: Option<String>,
        #[arg(long, conflicts_with = "env")]
        database: Option<String>,
        #[arg(long, conflicts_with = "database")]
        env: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Decrit les colonnes d'une table SQL.")]
    Describe {
        table: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, conflicts_with = "env")]
        database: Option<String>,
        #[arg(long, conflicts_with = "database")]
        env: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Execute une requete SQL read-only avec garde-fous et limite de lignes.")]
    Query {
        #[arg(long)]
        sql: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, conflicts_with = "env")]
        database: Option<String>,
        #[arg(long, conflicts_with = "database")]
        env: Option<String>,
        #[arg(long = "max-rows")]
        max_rows: Option<usize>,
        #[arg(long)]
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
