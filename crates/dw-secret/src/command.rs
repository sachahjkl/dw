use anyhow::Result;
use dw_core::{DevWorkflowRoot, SecretKey, SecretValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

use crate::{KeyringSecretStore, SecretStore, delete_secret, secret_exists, store_secret};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretSetReport {
    pub key: SecretKey,
    pub storage: SecretStorage,
    pub value_masked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretGetReport {
    pub key: SecretKey,
    pub exists: bool,
    pub value_masked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretDeleteReport {
    pub key: SecretKey,
    pub deleted_if_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretListReport {
    pub root: DevWorkflowRoot,
    pub items: Vec<SecretListItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretListItem {
    pub key: SecretKey,
    pub exists: bool,
    #[serde(rename = "valueMasked")]
    pub value_masked: bool,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecretStorage {
    SystemKeyring,
}

impl fmt::Display for SecretStorage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemKeyring => formatter.write_str("system keyring"),
        }
    }
}

pub fn set_secret(key: &SecretKey, secret: &SecretValue) -> Result<SecretSetReport> {
    set_secret_with_store(&KeyringSecretStore, key, secret)
}

pub fn list_secrets(root: Option<DevWorkflowRoot>) -> Result<SecretListReport> {
    list_secrets_with_store(root, &KeyringSecretStore)
}

pub fn list_secrets_with_store(
    root: Option<DevWorkflowRoot>,
    store: &impl SecretStore,
) -> Result<SecretListReport> {
    let root = DevWorkflowRoot::from(dw_config::resolve_root(
        root.as_ref().map(DevWorkflowRoot::as_str),
    ));
    let mut references = BTreeMap::<SecretKey, Vec<String>>::new();
    let mut warnings = Vec::new();
    for file_name in ["databases.json", "projects.json", "workflow.json"] {
        let path = Path::new(root.as_str()).join("config").join(file_name);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                warnings.push(format!("Could not read '{}': {error}", path.display()));
                continue;
            }
        };
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!("Could not parse '{}': {error}", path.display()));
                continue;
            }
        };
        collect_secret_references(&value, file_name, &mut references);
    }
    let items = references
        .into_iter()
        .map(|(key, references)| {
            Ok(SecretListItem {
                exists: secret_exists(store, &key)?,
                key,
                value_masked: true,
                references,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(SecretListReport {
        root,
        items,
        warnings,
    })
}

pub fn get_secret(key: &SecretKey) -> Result<SecretGetReport> {
    get_secret_with_store(&KeyringSecretStore, key)
}

pub fn delete_secret_key(key: &SecretKey) -> Result<SecretDeleteReport> {
    delete_secret_with_store(&KeyringSecretStore, key)
}

pub fn set_secret_with_store(
    store: &impl SecretStore,
    key: &SecretKey,
    secret: &SecretValue,
) -> Result<SecretSetReport> {
    store_secret(store, key, secret)?;
    Ok(SecretSetReport {
        key: key.clone(),
        storage: SecretStorage::SystemKeyring,
        value_masked: true,
    })
}

pub fn get_secret_with_store(store: &impl SecretStore, key: &SecretKey) -> Result<SecretGetReport> {
    Ok(SecretGetReport {
        key: key.clone(),
        exists: secret_exists(store, key)?,
        value_masked: true,
    })
}

pub fn delete_secret_with_store(
    store: &impl SecretStore,
    key: &SecretKey,
) -> Result<SecretDeleteReport> {
    delete_secret(store, key)?;
    Ok(SecretDeleteReport {
        key: key.clone(),
        deleted_if_present: true,
    })
}

fn collect_secret_references(
    value: &Value,
    path: &str,
    references: &mut BTreeMap<SecretKey, Vec<String>>,
) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let child_path = format!("{path}/{key}");
                if matches!(key.as_str(), "credentialKey" | "gitCredentialSecret")
                    && let Some(secret_key) = value
                        .as_str()
                        .filter(|secret_key| !secret_key.trim().is_empty())
                {
                    references
                        .entry(SecretKey::from(secret_key))
                        .or_default()
                        .push(child_path);
                } else {
                    collect_secret_references(value, &child_path, references);
                }
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                collect_secret_references(value, &format!("{path}/{index}"), references);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemorySecretStore;

    #[test]
    fn secret_set_report_never_includes_secret_value() {
        let store = MemorySecretStore::new();
        let key = SecretKey::from("db/password");
        let secret = SecretValue::from("password-value");
        let report = set_secret_with_store(&store, &key, &secret).expect("set secret");

        assert_eq!(report.key, key);
        assert_eq!(report.storage, SecretStorage::SystemKeyring);
        assert!(report.value_masked);
        assert!(!format!("{report:?}").contains("password-value"));
    }

    #[test]
    fn get_report_exposes_presence_only() {
        let store = MemorySecretStore::new();
        let key = SecretKey::from("db/password");
        let secret = SecretValue::from("password-value");
        let missing = get_secret_with_store(&store, &key).expect("get missing");
        set_secret_with_store(&store, &key, &secret).expect("set secret");
        let present = get_secret_with_store(&store, &key).expect("get present");

        assert!(!missing.exists);
        assert!(present.exists);
        assert!(present.value_masked);
    }

    #[test]
    fn delete_report_does_not_reveal_secret() {
        let store = MemorySecretStore::new();
        let key = SecretKey::from("db/password");
        let secret = SecretValue::from("password-value");
        set_secret_with_store(&store, &key, &secret).expect("set secret");

        let report = delete_secret_with_store(&store, &key).expect("delete secret");

        assert_eq!(report.key, key);
        assert!(report.deleted_if_present);
    }

    #[test]
    fn list_reports_configured_keys_without_values() {
        let root = tempfile::tempdir().expect("tempdir");
        let config = root.path().join("config");
        fs::create_dir(&config).expect("config directory");
        fs::write(
            config.join("databases.json"),
            r#"{"projects":{"acme":{"databases":{"dev":{"credentialKey":"db/acme/dev"}}}}}"#,
        )
        .expect("databases config");
        fs::write(
            config.join("projects.json"),
            r#"{"projects":{"acme":{"repositories":{"api":{"gitCredentialSecret":"git/acme"}}}}}"#,
        )
        .expect("projects config");
        fs::write(config.join("workflow.json"), "{}").expect("workflow config");
        let store = MemorySecretStore::new();
        store
            .set(
                &SecretKey::from("db/acme/dev"),
                &SecretValue::from("sensitive"),
            )
            .expect("stored secret");

        let report = list_secrets_with_store(
            Some(DevWorkflowRoot::from(root.path().display().to_string())),
            &store,
        )
        .expect("secret list");
        let serialized = serde_json::to_string(&report).expect("serialized report");

        assert_eq!(report.items.len(), 2);
        assert!(report.items.iter().any(|item| {
            item.key.as_str() == "db/acme/dev" && item.exists && item.value_masked
        }));
        assert!(
            report
                .items
                .iter()
                .any(|item| item.key.as_str() == "git/acme" && !item.exists)
        );
        assert!(!serialized.contains("sensitive"));
    }
}
