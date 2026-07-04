use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{KeyringSecretStore, SecretStore, delete_secret, secret_exists, store_secret};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretSetReport {
    pub key: String,
    pub storage: String,
    pub value_masked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretGetReport {
    pub key: String,
    pub exists: bool,
    pub value_masked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretDeleteReport {
    pub key: String,
    pub deleted_if_present: bool,
}

pub fn set_secret(key: &str, secret: &str) -> Result<SecretSetReport> {
    set_secret_with_store(&KeyringSecretStore, key, secret)
}

pub fn get_secret(key: &str) -> Result<SecretGetReport> {
    get_secret_with_store(&KeyringSecretStore, key)
}

pub fn delete_secret_key(key: &str) -> Result<SecretDeleteReport> {
    delete_secret_with_store(&KeyringSecretStore, key)
}

pub fn set_secret_with_store(
    store: &impl SecretStore,
    key: &str,
    secret: &str,
) -> Result<SecretSetReport> {
    store_secret(store, key, secret)?;
    Ok(SecretSetReport {
        key: key.into(),
        storage: "keyring système".into(),
        value_masked: true,
    })
}

pub fn get_secret_with_store(store: &impl SecretStore, key: &str) -> Result<SecretGetReport> {
    Ok(SecretGetReport {
        key: key.into(),
        exists: secret_exists(store, key)?,
        value_masked: true,
    })
}

pub fn delete_secret_with_store(store: &impl SecretStore, key: &str) -> Result<SecretDeleteReport> {
    delete_secret(store, key)?;
    Ok(SecretDeleteReport {
        key: key.into(),
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
        let report =
            set_secret_with_store(&store, "db/password", "password-value").expect("set secret");

        assert_eq!(report.key, "db/password");
        assert_eq!(report.storage, "keyring système");
        assert!(report.value_masked);
        assert!(!format!("{report:?}").contains("password-value"));
    }

    #[test]
    fn get_report_exposes_presence_only() {
        let store = MemorySecretStore::new();
        let missing = get_secret_with_store(&store, "db/password").expect("get missing");
        set_secret_with_store(&store, "db/password", "password-value").expect("set secret");
        let present = get_secret_with_store(&store, "db/password").expect("get present");

        assert!(!missing.exists);
        assert!(present.exists);
        assert!(present.value_masked);
    }

    #[test]
    fn delete_report_does_not_reveal_secret() {
        let store = MemorySecretStore::new();
        set_secret_with_store(&store, "db/password", "password-value").expect("set secret");

        let report = delete_secret_with_store(&store, "db/password").expect("delete secret");

        assert_eq!(report.key, "db/password");
        assert!(report.deleted_if_present);
    }
}
