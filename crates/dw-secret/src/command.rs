use anyhow::Result;
use dw_core::{SecretKey, SecretValue};
use serde::{Deserialize, Serialize};
use std::fmt;

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
}
