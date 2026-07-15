use crate::{load_databases_config, load_projects_config};

pub fn project_values(root: &str) -> Vec<String> {
    load_projects_config(root)
        .projects
        .keys()
        .cloned()
        .collect::<Vec<_>>()
}

pub fn database_values(root: &str, project: Option<&str>) -> Vec<String> {
    let config = load_databases_config(root);
    let mut values = config.globals.keys().cloned().collect::<Vec<_>>();
    if let Some(project) = project.and_then(|project| config.projects.get(project))
        && let Some(map) = project
            .get("databases")
            .and_then(serde_json::Value::as_object)
    {
        values.extend(map.keys().cloned());
    }
    values.sort();
    values.dedup();
    values
}

pub fn env_values(root: &str, project: Option<&str>) -> Vec<String> {
    database_values(root, project)
}

pub fn secret_key_values(root: &str) -> Vec<String> {
    let config = load_databases_config(root);
    let globals = config.globals.values().filter_map(database_credential_key);
    let projects = config
        .projects
        .values()
        .filter_map(|project| {
            project
                .get("databases")
                .and_then(serde_json::Value::as_object)
        })
        .flat_map(|databases| databases.values().filter_map(database_credential_key));

    let mut values = globals
        .chain(projects)
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn database_credential_key(value: &serde_json::Value) -> Option<&str> {
    value
        .get("credentialKey")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn secret_key_values_come_from_database_credentials() {
        let root = temp_root("secret-key-completion");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(
            root.join("config/databases.json"),
            r#"{
  "globals": {
    "shared": { "provider": "sqlserver", "credentialKey": "db/shared" }
  },
  "projects": {
    "acme": {
      "databases": {
        "dev": { "provider": "sqlserver", "credentialKey": "db/acme-dev" },
        "inline": { "provider": "sqlserver", "connectionString": "Server=." }
      }
    }
  }
}"#,
        )
        .expect("databases config");

        assert_eq!(
            secret_key_values(root.to_str().expect("root")),
            vec!["db/acme-dev", "db/shared"]
        );
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("dw-{name}-{suffix}"))
    }
}
