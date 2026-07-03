use anyhow::Result;
use clap::Subcommand;

use crate::commands;

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    Guard {
        #[arg(long)]
        sql: String,
    },
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
