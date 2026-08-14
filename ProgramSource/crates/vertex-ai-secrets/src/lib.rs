//! Secret-store abstraction. Production adapters must use an operating-system secret store.

use async_trait::async_trait;
use std::{collections::HashMap, fmt, sync::Arc};
use thiserror::Error;
use tokio::sync::RwLock;
use vertex_ai_types::ProviderId;
use zeroize::Zeroizing;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretId(String);

impl SecretId {
    pub fn provider_api_key(provider_id: &ProviderId) -> Self {
        Self(format!("provider/{provider_id}/api-key"))
    }

    pub fn memory_database_password() -> Self {
        Self("memory/postgresql/application-password".to_owned())
    }

    pub fn memory_bootstrap_password() -> Self {
        Self("memory/postgresql/bootstrap-password".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SecretValue(Zeroizing<String>);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Result<Self, SecretStoreError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SecretStoreError::Invalid(
                "secret cannot be empty".to_owned(),
            ));
        }
        Ok(Self(Zeroizing::new(value)))
    }

    pub fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretValue([REDACTED])")
    }
}

#[derive(Debug, Error)]
pub enum SecretStoreError {
    #[error("invalid secret: {0}")]
    Invalid(String),
    #[error("secret store is unavailable: {0}")]
    Unavailable(String),
}

#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn put(&self, id: &SecretId, value: SecretValue) -> Result<(), SecretStoreError>;
    async fn get(&self, id: &SecretId) -> Result<Option<SecretValue>, SecretStoreError>;
    async fn delete(&self, id: &SecretId) -> Result<bool, SecretStoreError>;
}

/// Development and test implementation only. It never persists secrets to disk.
#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    values: RwLock<HashMap<SecretId, SecretValue>>,
}

/// Operating-system backed secret storage. On Windows this uses Credential Manager.
/// Blocking platform calls are serialized and moved off the async executor.
#[derive(Debug, Clone)]
pub struct WindowsCredentialStore {
    service_name: Arc<str>,
    operation_lock: Arc<std::sync::Mutex<()>>,
}

impl WindowsCredentialStore {
    pub fn new(service_name: impl Into<String>) -> Result<Self, SecretStoreError> {
        let service_name = service_name.into();
        if service_name.trim().is_empty() {
            return Err(SecretStoreError::Invalid(
                "secret-store service name cannot be empty".to_owned(),
            ));
        }
        if !cfg!(target_os = "windows") {
            return Err(SecretStoreError::Unavailable(
                "Windows Credential Manager is unavailable on this platform".to_owned(),
            ));
        }
        Ok(Self {
            service_name: Arc::from(service_name),
            operation_lock: Arc::new(std::sync::Mutex::new(())),
        })
    }

    async fn run<T, F>(&self, operation: F) -> Result<T, SecretStoreError>
    where
        T: Send + 'static,
        F: FnOnce(&str) -> Result<T, SecretStoreError> + Send + 'static,
    {
        let service_name = self.service_name.clone();
        let operation_lock = self.operation_lock.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = operation_lock.lock().map_err(|_| {
                SecretStoreError::Unavailable("secret-store operation lock failed".to_owned())
            })?;
            operation(&service_name)
        })
        .await
        .map_err(|_| SecretStoreError::Unavailable("secret-store task failed".to_owned()))?
    }
}

#[async_trait]
impl SecretStore for WindowsCredentialStore {
    async fn put(&self, id: &SecretId, value: SecretValue) -> Result<(), SecretStoreError> {
        let username = id.as_str().to_owned();
        self.run(move |service_name| {
            let entry = keyring::Entry::new(service_name, &username).map_err(map_keyring_error)?;
            entry
                .set_password(value.expose())
                .map_err(map_keyring_error)
        })
        .await
    }

    async fn get(&self, id: &SecretId) -> Result<Option<SecretValue>, SecretStoreError> {
        let username = id.as_str().to_owned();
        self.run(move |service_name| {
            let entry = keyring::Entry::new(service_name, &username).map_err(map_keyring_error)?;
            match entry.get_password() {
                Ok(value) => SecretValue::new(value).map(Some),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(error) => Err(map_keyring_error(error)),
            }
        })
        .await
    }

    async fn delete(&self, id: &SecretId) -> Result<bool, SecretStoreError> {
        let username = id.as_str().to_owned();
        self.run(move |service_name| {
            let entry = keyring::Entry::new(service_name, &username).map_err(map_keyring_error)?;
            match entry.delete_credential() {
                Ok(()) => Ok(true),
                Err(keyring::Error::NoEntry) => Ok(false),
                Err(error) => Err(map_keyring_error(error)),
            }
        })
        .await
    }
}

#[cfg(not(test))]
fn map_keyring_error(_error: keyring::Error) -> SecretStoreError {
    SecretStoreError::Unavailable("operating-system secret store operation failed".to_owned())
}

#[cfg(test)]
fn map_keyring_error(error: keyring::Error) -> SecretStoreError {
    SecretStoreError::Unavailable(format!("test-only OS error classification: {error:?}"))
}

impl InMemorySecretStore {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl SecretStore for InMemorySecretStore {
    async fn put(&self, id: &SecretId, value: SecretValue) -> Result<(), SecretStoreError> {
        self.values.write().await.insert(id.clone(), value);
        Ok(())
    }

    async fn get(&self, id: &SecretId) -> Result<Option<SecretValue>, SecretStoreError> {
        Ok(self.values.read().await.get(id).cloned())
    }

    async fn delete(&self, id: &SecretId) -> Result<bool, SecretStoreError> {
        Ok(self.values.write().await.remove(id).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_store_round_trip_and_redaction() {
        let provider = ProviderId::new("mock").unwrap();
        let id = SecretId::provider_api_key(&provider);
        let store = InMemorySecretStore::default();
        let value = SecretValue::new("do-not-log-me").unwrap();

        assert!(!format!("{value:?}").contains("do-not-log-me"));
        store.put(&id, value).await.unwrap();
        assert_eq!(
            store.get(&id).await.unwrap().unwrap().expose(),
            "do-not-log-me"
        );
        assert!(store.delete(&id).await.unwrap());
        assert!(store.get(&id).await.unwrap().is_none());
    }

    #[test]
    fn os_store_rejects_blank_service_name_without_platform_access() {
        assert!(WindowsCredentialStore::new("  ").is_err());
    }

    #[tokio::test]
    #[ignore = "accesses the real Windows Credential Manager"]
    async fn windows_credential_manager_round_trip_when_enabled() {
        let suffix = uuid::Uuid::new_v4();
        let store = WindowsCredentialStore::new(format!("Vertex AI Integration Test {suffix}"))
            .expect("create Windows credential adapter");
        let provider = ProviderId::new(format!("integration-test-{suffix}")).unwrap();
        let id = SecretId::provider_api_key(&provider);
        store
            .put(&id, SecretValue::new("temporary-secret").unwrap())
            .await
            .expect("store temporary credential");
        let result = store.get(&id).await;
        let cleanup = store.delete(&id).await;
        assert_eq!(result.unwrap().unwrap().expose(), "temporary-secret");
        assert!(cleanup.expect("remove temporary credential"));
    }
}
