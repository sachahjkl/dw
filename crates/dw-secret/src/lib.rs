pub mod command;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

use dw_core::EnvironmentVariableName;

pub const KEYRING_SERVICE: &str = "dw";
pub const KEY_PREFIX: &str = "dw/";

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("Clé de secret vide.")]
    EmptyKey,
    #[error("Variable d'environnement introuvable: {0}")]
    MissingEnvironmentVariable(String),
    #[error("Secret store indisponible: {0}")]
    Store(String),
}

pub trait SecretStore {
    fn set(&self, key: &str, secret: &str) -> Result<(), SecretError>;
    fn get(&self, key: &str) -> Result<Option<String>, SecretError>;
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}

#[derive(Debug, Default, Clone)]
pub struct KeyringSecretStore;

impl SecretStore for KeyringSecretStore {
    fn set(&self, key: &str, secret: &str) -> Result<(), SecretError> {
        entry(key)?
            .set_password(secret)
            .map_err(|error| SecretError::Store(error.to_string()))
    }

    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        match entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(SecretError::Store(error.to_string())),
        }
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        match entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(SecretError::Store(error.to_string())),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MemorySecretStore {
    values: Arc<Mutex<BTreeMap<String, String>>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn set(&self, key: &str, secret: &str) -> Result<(), SecretError> {
        validate_key(key)?;
        self.values
            .lock()
            .map_err(|error| SecretError::Store(error.to_string()))?
            .insert(normalize_key(key), secret.into());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        validate_key(key)?;
        Ok(self
            .values
            .lock()
            .map_err(|error| SecretError::Store(error.to_string()))?
            .get(&normalize_key(key))
            .cloned())
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        validate_key(key)?;
        self.values
            .lock()
            .map_err(|error| SecretError::Store(error.to_string()))?
            .remove(&normalize_key(key));
        Ok(())
    }
}

pub fn secret_from_env(name: &EnvironmentVariableName) -> Result<String, SecretError> {
    std::env::var(name.as_str())
        .map_err(|_| SecretError::MissingEnvironmentVariable(name.to_string()))
}

pub fn store_secret(store: &impl SecretStore, key: &str, secret: &str) -> Result<(), SecretError> {
    store.set(key, secret)
}

pub fn secret_exists(store: &impl SecretStore, key: &str) -> Result<bool, SecretError> {
    Ok(store.get(key)?.is_some())
}

pub fn delete_secret(store: &impl SecretStore, key: &str) -> Result<(), SecretError> {
    store.delete(key)
}

fn entry(key: &str) -> Result<keyring::Entry, SecretError> {
    keyring::Entry::new(KEYRING_SERVICE, &target(key)?)
        .map_err(|error| SecretError::Store(error.to_string()))
}

fn target(key: &str) -> Result<String, SecretError> {
    validate_key(key)?;
    Ok(format!("{KEY_PREFIX}{}", key.trim()))
}

fn normalize_key(key: &str) -> String {
    key.trim().into()
}

fn validate_key(key: &str) -> Result<(), SecretError> {
    if key.trim().is_empty() {
        Err(SecretError::EmptyKey)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_sets_gets_and_deletes_secret() {
        let store = MemorySecretStore::new();

        store_secret(&store, "db/password", "secret").expect("secret should be stored");

        assert!(secret_exists(&store, "db/password").expect("lookup should work"));
        assert_eq!(
            store.get("db/password").expect("secret should be read"),
            Some("secret".into())
        );
        delete_secret(&store, "db/password").expect("secret should be deleted");
        assert!(!secret_exists(&store, "db/password").expect("lookup should work"));
    }

    #[test]
    fn empty_keys_are_rejected() {
        let store = MemorySecretStore::new();

        let error = store_secret(&store, " ", "secret").expect_err("empty key should fail");

        assert!(matches!(error, SecretError::EmptyKey));
    }

    #[test]
    fn target_prefix_matches_dotnet_shape() {
        assert_eq!(target("demo").expect("target"), "dw/demo");
    }
}
