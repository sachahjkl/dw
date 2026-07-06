pub mod command;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

use dw_core::{EnvironmentVariableName, SecretKey, SecretStoreErrorMessage, SecretValue};

pub const KEYRING_SERVICE: &str = "dw";
pub const KEY_PREFIX: &str = "dw/";

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("Secret key is empty.")]
    EmptyKey,
    #[error("Environment variable not found: {0}")]
    MissingEnvironmentVariable(EnvironmentVariableName),
    #[error("Secret store unavailable: {0}")]
    Store(SecretStoreErrorMessage),
}

pub trait SecretStore {
    fn set(&self, key: &SecretKey, secret: &SecretValue) -> Result<(), SecretError>;
    fn get(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretError>;
    fn delete(&self, key: &SecretKey) -> Result<(), SecretError>;
}

#[derive(Debug, Default, Clone)]
pub struct KeyringSecretStore;

impl SecretStore for KeyringSecretStore {
    fn set(&self, key: &SecretKey, secret: &SecretValue) -> Result<(), SecretError> {
        entry(key)?
            .set_password(secret.as_str())
            .map_err(|error| SecretError::Store(SecretStoreErrorMessage::from(error.to_string())))
    }

    fn get(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretError> {
        match entry(key)?.get_password() {
            Ok(value) => Ok(Some(SecretValue::from(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(SecretError::Store(SecretStoreErrorMessage::from(
                error.to_string(),
            ))),
        }
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretError> {
        match entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(SecretError::Store(SecretStoreErrorMessage::from(
                error.to_string(),
            ))),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MemorySecretStore {
    values: Arc<Mutex<BTreeMap<SecretKey, SecretValue>>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn set(&self, key: &SecretKey, secret: &SecretValue) -> Result<(), SecretError> {
        validate_key(key)?;
        self.values
            .lock()
            .map_err(|error| SecretError::Store(SecretStoreErrorMessage::from(error.to_string())))?
            .insert(normalize_key(key), secret.clone());
        Ok(())
    }

    fn get(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretError> {
        validate_key(key)?;
        Ok(self
            .values
            .lock()
            .map_err(|error| SecretError::Store(SecretStoreErrorMessage::from(error.to_string())))?
            .get(&normalize_key(key))
            .cloned())
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretError> {
        validate_key(key)?;
        self.values
            .lock()
            .map_err(|error| SecretError::Store(SecretStoreErrorMessage::from(error.to_string())))?
            .remove(&normalize_key(key));
        Ok(())
    }
}

pub fn secret_from_env(name: &EnvironmentVariableName) -> Result<SecretValue, SecretError> {
    std::env::var(name.as_str())
        .map(SecretValue::from)
        .map_err(|_| SecretError::MissingEnvironmentVariable(name.clone()))
}

pub fn store_secret(
    store: &impl SecretStore,
    key: &SecretKey,
    secret: &SecretValue,
) -> Result<(), SecretError> {
    store.set(key, secret)
}

pub fn secret_exists(store: &impl SecretStore, key: &SecretKey) -> Result<bool, SecretError> {
    Ok(store.get(key)?.is_some())
}

pub fn delete_secret(store: &impl SecretStore, key: &SecretKey) -> Result<(), SecretError> {
    store.delete(key)
}

fn entry(key: &SecretKey) -> Result<keyring::Entry, SecretError> {
    keyring::Entry::new(KEYRING_SERVICE, &target(key)?)
        .map_err(|error| SecretError::Store(SecretStoreErrorMessage::from(error.to_string())))
}

fn target(key: &SecretKey) -> Result<String, SecretError> {
    validate_key(key)?;
    Ok(format!("{KEY_PREFIX}{}", key.as_str().trim()))
}

fn normalize_key(key: &SecretKey) -> SecretKey {
    SecretKey::from(key.as_str().trim())
}

fn validate_key(key: &SecretKey) -> Result<(), SecretError> {
    if key.as_str().trim().is_empty() {
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
        let key = SecretKey::from("db/password");
        let value = SecretValue::from("secret");

        store_secret(&store, &key, &value).expect("secret should be stored");

        assert!(secret_exists(&store, &key).expect("lookup should work"));
        assert_eq!(store.get(&key).expect("secret should be read"), Some(value));
        delete_secret(&store, &key).expect("secret should be deleted");
        assert!(!secret_exists(&store, &key).expect("lookup should work"));
    }

    #[test]
    fn empty_keys_are_rejected() {
        let store = MemorySecretStore::new();
        let key = SecretKey::from(" ");
        let value = SecretValue::from("secret");

        let error = store_secret(&store, &key, &value).expect_err("empty key should fail");

        assert!(matches!(error, SecretError::EmptyKey));
    }

    #[test]
    fn target_prefix_matches_dotnet_shape() {
        assert_eq!(target(&SecretKey::from("demo")).expect("target"), "dw/demo");
    }
}
