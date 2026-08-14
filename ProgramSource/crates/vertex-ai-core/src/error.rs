use thiserror::Error;
use vertex_ai_context::ContextError;
use vertex_ai_environment::EnvironmentError;
use vertex_ai_memory::MemoryError;
use vertex_ai_provider::{ProviderError, RegistryError};
use vertex_ai_runtime::{RuntimeError, RuntimeRegistryError};
use vertex_ai_secrets::SecretStoreError;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Environment(#[from] EnvironmentError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    RuntimeRegistry(#[from] RuntimeRegistryError),
    #[error(transparent)]
    SecretStore(#[from] SecretStoreError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error("memory service is not configured")]
    MemoryNotConfigured,
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error("context builder is not configured")]
    ContextNotConfigured,
    #[error("no model has been selected")]
    ModelNotSelected,
    #[error("model download coordinator is not configured")]
    DownloadCoordinatorNotConfigured,
}
