use serde_json::Value;
use std::collections::BTreeMap;

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
    pub provider: String,
    pub connection_string: Option<String>,
    pub connection_string_environment_variable: Option<String>,
    pub credential_key: Option<String>,
    pub readonly: Option<bool>,
    pub max_rows: Option<usize>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectDatabases {
    pub databases: BTreeMap<String, DatabaseConnectionConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDatabase {
    pub connection: DatabaseConnectionConfig,
    pub defaults: DatabaseDefaults,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseSelection<'a> {
    pub project: &'a str,
    pub database: &'a str,
}

pub fn resolve_connection(
    config: &dw_config::DatabasesConfig,
    selection: DatabaseSelection<'_>,
) -> Result<ResolvedDatabase, String> {
    let defaults = parse_defaults(config.defaults.as_ref())?;
    let connection = try_resolve_connection(config, selection.project, selection.database)
        .ok_or_else(|| {
            format!(
                "Base introuvable dans databases.json: {}/{}",
                selection.project, selection.database
            )
        })?;

    if connection.readonly == Some(false) || !defaults.readonly {
        return Err("Exécution SQL refusée: readonly doit rester true.".into());
    }

    Ok(ResolvedDatabase {
        connection,
        defaults,
    })
}

pub fn try_resolve_connection(
    config: &dw_config::DatabasesConfig,
    project: &str,
    database: &str,
) -> Option<DatabaseConnectionConfig> {
    config
        .projects
        .get(project)
        .and_then(parse_project_databases)
        .and_then(|project| project.databases.get(database).cloned())
        .or_else(|| {
            config
                .globals
                .get(database)
                .and_then(parse_database_connection)
        })
}

fn parse_defaults(value: Option<&Value>) -> Result<DatabaseDefaults, String> {
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
            .filter_map(|(key, value)| Some((key.clone(), parse_database_connection(value)?)))
            .collect(),
    })
}

fn parse_database_connection(value: &Value) -> Option<DatabaseConnectionConfig> {
    Some(DatabaseConnectionConfig {
        provider: value.get("provider")?.as_str()?.to_string(),
        connection_string: optional_string(value, "connectionString"),
        connection_string_environment_variable: optional_string(
            value,
            "connectionStringEnvironmentVariable",
        ),
        credential_key: optional_string(value, "credentialKey"),
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
                project: "ha",
                database: "shared",
            },
        )
        .expect("connection should resolve");

        assert_eq!(
            resolved.connection.connection_string.as_deref(),
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
                project: "ha",
                database: "shared",
            },
        )
        .expect("connection should resolve");

        assert_eq!(
            resolved.connection.connection_string.as_deref(),
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
                project: "ha",
                database: "dev",
            },
        )
        .expect_err("non readonly should fail");

        assert!(error.contains("readonly"));
    }
}
