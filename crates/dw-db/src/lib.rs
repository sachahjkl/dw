mod collect;
pub mod commands;
mod config;
mod guard;
mod query;

pub use collect::{
    CollectArgs, DatabaseCollectFinding, DatabaseCollectReport, DatabaseCollectStatus,
    DatabaseConnectionSource, DatabaseListEntry, DatabaseListReport, ListArgs, collect_databases,
    list_databases,
};
pub use config::{
    DatabaseCatalogEntry, DatabaseConnectionConfig, DatabaseDefaults, DatabaseProvider,
    DatabaseSelection, DbConfigError, ProjectDatabases, ResolvedDatabase, database_catalog,
    resolve_connection,
};
pub use guard::{SqlGuardResult, validate_read_only_sql};
pub use query::{
    DbError, QueryResult, describe_table_sql, query_sql_server, resolve_connection_string,
    schema_sql,
};
