use crate::cli::DbCommand;
use anyhow::Result;

pub(crate) fn handle_db(command: DbCommand) -> Result<()> {
    match command {
        DbCommand::Guard { sql } => {
            dw_db::commands::guard(dw_db::commands::GuardArgs { sql });
        }
        DbCommand::Schema {
            project,
            database,
            env,
            json,
        } => dw_db::commands::schema(dw_db::commands::SchemaArgs {
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
        } => dw_db::commands::describe(dw_db::commands::DescribeArgs {
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
        } => dw_db::commands::query(dw_db::commands::QueryArgs {
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
