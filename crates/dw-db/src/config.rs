use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

use dw_core::{
    DatabaseConnectionString, DatabaseEnvironmentName, DatabaseKey, ProjectKey, SecretKey,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseDefaults {
    pub readonly: bool,
    pub max_rows: usize,
    pub timeout_seconds: u64,
}

impl Default for DatabaseDefaults {
    fn default() -> Self {
        Self {
            readonly: true,
            max_rows: 500,
            timeout_seconds: 600,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConnectionConfig {
    pub provider: DatabaseProvider,
    pub connection_string: Option<DatabaseConnectionString>,
    pub connection_string_environment_variable: Option<DatabaseEnvironmentName>,
    pub credential_key: Option<SecretKey>,
    pub readonly: Option<bool>,
    pub max_rows: Option<usize>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseProvider {
    SqlServer,
    Unsupported(DatabaseProviderName),
}

impl DatabaseProvider {
    pub fn parse(value: &str) -> Self {
        value.parse().unwrap_or_else(Self::Unsupported)
    }
}

impl FromStr for DatabaseProvider {
    type Err = DatabaseProviderName;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.trim().eq_ignore_ascii_case("sqlserver") {
            Ok(Self::SqlServer)
        } else {
            Err(DatabaseProviderName::from(value.trim().to_string()))
        }
    }
}

impl fmt::Display for DatabaseProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SqlServer => formatter.write_str("sqlserver"),
            Self::Unsupported(provider) => write!(formatter, "{provider}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseProviderName(String);

impl From<String> for DatabaseProviderName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for DatabaseProviderName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectDatabases {
    pub databases: BTreeMap<DatabaseKey, DatabaseConnectionConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDatabase {
    pub connection: DatabaseConnectionConfig,
    pub defaults: DatabaseDefaults,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseSelection<'a> {
    pub project: &'a ProjectKey,
    pub database: &'a DatabaseKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseCatalogEntry {
    pub project: Option<ProjectKey>,
    pub database: DatabaseKey,
}

#[derive(Debug, Error)]
pub enum DbConfigError {
    #[error("Base introuvable dans databases.json: {project}/{database}")]
    MissingDatabase {
        project: ProjectKey,
        database: DatabaseKey,
    },
    #[error("Exécution SQL refusée: readonly doit rester true.")]
    ReadOnlyRequired,
}

pub fn resolve_connection(
    config: &dw_config::DatabasesConfig,
    selection: DatabaseSelection<'_>,
) -> Result<ResolvedDatabase, DbConfigError> {
    let defaults = parse_defaults(config.defaults.as_ref())?;
    let connection = try_resolve_connection(config, selection.project, selection.database)
        .ok_or_else(|| DbConfigError::MissingDatabase {
            project: selection.project.clone(),
            database: selection.database.clone(),
        })?;

    if connection.readonly == Some(false) || !defaults.readonly {
        return Err(DbConfigError::ReadOnlyRequired);
    }

    Ok(ResolvedDatabase {
        connection,
        defaults,
    })
}

pub fn try_resolve_connection(
    config: &dw_config::DatabasesConfig,
    project: &ProjectKey,
    database: &DatabaseKey,
) -> Option<DatabaseConnectionConfig> {
    config
        .projects
        .get(project.as_str())
        .and_then(parse_project_databases)
        .and_then(|project| project.databases.get(database).cloned())
        .or_else(|| {
            config
                .globals
                .get(database.as_str())
                .and_then(parse_database_connection)
        })
}

pub fn database_catalog(config: &dw_config::DatabasesConfig) -> Vec<DatabaseCatalogEntry> {
    let mut entries = config
        .globals
        .keys()
        .map(|key| DatabaseCatalogEntry {
            project: None,
            database: DatabaseKey::from(key.as_str()),
        })
        .collect::<Vec<_>>();

    for (project, value) in &config.projects {
        let Some(databases) = parse_project_databases(value) else {
            continue;
        };
        entries.extend(
            databases
                .databases
                .keys()
                .map(|database| DatabaseCatalogEntry {
                    project: Some(ProjectKey::from(project.as_str())),
                    database: database.clone(),
                }),
        );
    }

    entries
}

fn parse_defaults(value: Option<&Value>) -> Result<DatabaseDefaults, DbConfigError> {
    let Some(value) = value else {
        return Ok(DatabaseDefaults::default());
    };
    Ok(DatabaseDefaults {
        readonly: value
            .get("readonly")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        max_rows: value
            .get("maxRows")
            .and_then(Value::as_u64)
            .unwrap_or(500)
            .try_into()
            .unwrap_or(500),
        timeout_seconds: value
            .get("timeoutSeconds")
            .and_then(Value::as_u64)
            .unwrap_or(600),
    })
}

fn parse_project_databases(value: &Value) -> Option<ProjectDatabases> {
    let databases = value.get("databases")?.as_object()?;
    Some(ProjectDatabases {
        databases: databases
            .iter()
            .filter_map(|(key, value)| {
                Some((
                    DatabaseKey::from(key.as_str()),
                    parse_database_connection(value)?,
                ))
            })
            .collect(),
    })
}

fn parse_database_connection(value: &Value) -> Option<DatabaseConnectionConfig> {
    Some(DatabaseConnectionConfig {
        provider: DatabaseProvider::parse(value.get("provider")?.as_str()?),
        connection_string: optional_string(value, "connectionString")
            .map(DatabaseConnectionString::from),
        connection_string_environment_variable: optional_string(
            value,
            "connectionStringEnvironmentVariable",
        )
        .map(DatabaseEnvironmentName::from),
        credential_key: optional_string(value, "credentialKey").map(SecretKey::from),
        readonly: value.get("readonly").and_then(Value::as_bool),
        max_rows: value
            .get("maxRows")
            .and_then(Value::as_u64)
            .and_then(|value| value.try_into().ok()),
        timeout_seconds: value.get("timeoutSeconds").and_then(Value::as_u64),
    })
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_connection_prefers_project_database_before_global() {
        let config: dw_config::DatabasesConfig = serde_json::from_str(
            r#"{
  "defaults": { "readonly": true, "maxRows": 500, "timeoutSeconds": 600 },
  "globals": {
    "shared": { "provider": "sqlserver", "connectionString": "global", "readonly": true }
  },
  "projects": {
    "ha": {
      "databases": {
        "shared": { "provider": "sqlserver", "connectionString": "project", "readonly": true }
      }
    }
  }
}"#,
        )
        .expect("db config should parse");

        let resolved = resolve_connection(
            &config,
            DatabaseSelection {
                project: &ProjectKey::from("ha"),
                database: &DatabaseKey::from("shared"),
            },
        )
        .expect("connection should resolve");

        assert_eq!(
            resolved
                .connection
                .connection_string
                .as_ref()
                .map(DatabaseConnectionString::as_str),
            Some("project")
        );
    }

    #[test]
    fn resolve_connection_falls_back_to_global_database() {
        let config: dw_config::DatabasesConfig = serde_json::from_str(
            r#"{
  "defaults": { "readonly": true, "maxRows": 500, "timeoutSeconds": 600 },
  "globals": {
    "shared": { "provider": "sqlserver", "connectionString": "global", "readonly": true }
  },
  "projects": {}
}"#,
        )
        .expect("db config should parse");

        let resolved = resolve_connection(
            &config,
            DatabaseSelection {
                project: &ProjectKey::from("ha"),
                database: &DatabaseKey::from("shared"),
            },
        )
        .expect("connection should resolve");

        assert_eq!(
            resolved
                .connection
                .connection_string
                .as_ref()
                .map(DatabaseConnectionString::as_str),
            Some("global")
        );
    }

    #[test]
    fn resolve_connection_rejects_non_readonly() {
        let config: dw_config::DatabasesConfig = serde_json::from_str(
            r#"{
  "defaults": { "readonly": true, "maxRows": 500, "timeoutSeconds": 600 },
  "globals": {
    "dev": { "provider": "sqlserver", "connectionString": "db", "readonly": false }
  },
  "projects": {}
}"#,
        )
        .expect("db config should parse");

        let error = resolve_connection(
            &config,
            DatabaseSelection {
                project: &ProjectKey::from("ha"),
                database: &DatabaseKey::from("dev"),
            },
        )
        .expect_err("non readonly should fail");

        assert!(error.to_string().contains("readonly"));
    }
}
