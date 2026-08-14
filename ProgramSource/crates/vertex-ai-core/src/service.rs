use crate::{Command, CommandResponse, CoreConfig, CoreError};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{Instrument, info_span};
use vertex_ai_context::{ContextBuilder, TargetLocation};
use vertex_ai_environment::{
    EnvironmentScanner, IndexedEnvironmentSnapshot, PersistentEnvironmentIndex,
};
use vertex_ai_memory::{MemoryRepository, MemoryService};
use vertex_ai_provider::{ModelProvider, ModelRegistry, ProviderRegistry};
use vertex_ai_runtime::{LocalRuntimeManager, ModelDownloadCoordinator, RuntimeRegistry};
use vertex_ai_secrets::{SecretId, SecretStore};
use vertex_ai_types::{
    AiEnvironmentSummary, GenerationParameters, GenerationRequest, GenerationResponse, HealthState,
    Message, ModelRef, ProviderId,
};

pub struct VertexAiCore {
    config: CoreConfig,
    providers: ProviderRegistry,
    runtimes: RuntimeRegistry,
    downloads: Option<ModelDownloadCoordinator>,
    models: ModelRegistry,
    secrets: Arc<dyn SecretStore>,
    selected_model: RwLock<Option<ModelRef>>,
    memory: RwLock<Option<MemoryService>>,
    context_builder: RwLock<Option<ContextBuilder>>,
    environment_scanner: EnvironmentScanner,
    environment_index: Option<Mutex<PersistentEnvironmentIndex>>,
}

impl VertexAiCore {
    pub fn new(config: CoreConfig, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            config,
            providers: ProviderRegistry::default(),
            runtimes: RuntimeRegistry::default(),
            downloads: None,
            models: ModelRegistry::default(),
            secrets,
            selected_model: RwLock::new(None),
            memory: RwLock::new(None),
            context_builder: RwLock::new(None),
            environment_scanner: EnvironmentScanner::default(),
            environment_index: None,
        }
    }

    pub fn with_memory_repository(mut self, repository: Arc<dyn MemoryRepository>) -> Self {
        self.memory = RwLock::new(Some(MemoryService::new(repository.clone())));
        self.context_builder = RwLock::new(Some(ContextBuilder::new(repository)));
        self
    }

    /// Attaches or replaces a Memory repository after a degraded startup has
    /// been repaired, without requiring the desktop process to restart.
    pub async fn configure_memory_repository(&self, repository: Arc<dyn MemoryRepository>) {
        *self.memory.write().await = Some(MemoryService::new(repository.clone()));
        *self.context_builder.write().await = Some(ContextBuilder::new(repository));
    }

    pub fn with_environment_index(mut self, index: PersistentEnvironmentIndex) -> Self {
        self.environment_index = Some(Mutex::new(index));
        self
    }

    pub fn config(&self) -> &CoreConfig {
        &self.config
    }

    pub async fn register_provider(
        &self,
        provider: Arc<dyn ModelProvider>,
    ) -> Result<(), CoreError> {
        self.providers.register(provider).await?;
        Ok(())
    }

    pub async fn register_runtime(
        &self,
        runtime: Arc<dyn LocalRuntimeManager>,
    ) -> Result<(), CoreError> {
        self.runtimes.register(runtime).await?;
        Ok(())
    }

    pub fn with_download_coordinator(mut self, coordinator: ModelDownloadCoordinator) -> Self {
        self.downloads = Some(coordinator);
        self
    }

    pub async fn execute(&self, command: Command) -> Result<CommandResponse, CoreError> {
        let command_name = command.name();
        self.execute_inner(command)
            .instrument(info_span!("vertex_command", command = command_name))
            .await
    }

    async fn execute_inner(&self, command: Command) -> Result<CommandResponse, CoreError> {
        match command {
            Command::ScanEnvironment { path_override } => {
                let snapshot = self
                    .environment_scanner
                    .scan_path(path_override.as_deref().map(std::ffi::OsStr::new))?;
                let delta = match &self.environment_index {
                    Some(index) => Some(index.lock().await.update(snapshot.clone())?),
                    None => None,
                };
                Ok(CommandResponse::EnvironmentScanned(
                    IndexedEnvironmentSnapshot { snapshot, delta },
                ))
            }
            Command::GetEnvironmentSnapshot => {
                let snapshot = match &self.environment_index {
                    Some(index) => index.lock().await.current().cloned(),
                    None => None,
                };
                Ok(CommandResponse::EnvironmentSnapshot(snapshot))
            }
            Command::GetModels {
                provider_id,
                refresh,
            } => {
                if refresh {
                    match &provider_id {
                        Some(id) => self.refresh_provider_models(id).await?,
                        None => {
                            for id in self.providers.ids().await {
                                self.refresh_provider_models(&id).await?;
                            }
                        }
                    }
                }
                Ok(CommandResponse::Models(
                    self.models.list(provider_id.as_ref()).await,
                ))
            }
            Command::GetProviderHealth { provider_id } => {
                let provider = self.providers.get(&provider_id).await?;
                Ok(CommandResponse::ProviderHealth(provider.health().await))
            }
            Command::GetAiEnvironment => {
                let mut runtimes = Vec::new();
                for runtime in self.runtimes.list().await {
                    runtimes.push(runtime.inspect().await?);
                }
                let ready_runtime_count = runtimes
                    .iter()
                    .filter(|runtime| runtime.health == HealthState::Ready)
                    .count();
                let installed_model_count = runtimes
                    .iter()
                    .map(|runtime| runtime.installed_models.len())
                    .sum();
                let loaded_model_count = runtimes
                    .iter()
                    .map(|runtime| runtime.loaded_models.len())
                    .sum();
                let total_model_bytes = runtimes
                    .iter()
                    .flat_map(|runtime| &runtime.installed_models)
                    .map(|model| model.size_bytes)
                    .sum();
                let total_vram_bytes = runtimes
                    .iter()
                    .flat_map(|runtime| &runtime.loaded_models)
                    .map(|model| model.size_vram_bytes)
                    .sum();
                Ok(CommandResponse::AiEnvironment(AiEnvironmentSummary {
                    runtime_count: runtimes.len(),
                    ready_runtime_count,
                    installed_model_count,
                    loaded_model_count,
                    total_model_bytes,
                    total_vram_bytes,
                    local_inference_ready: ready_runtime_count > 0 && installed_model_count > 0,
                    runtimes,
                    observed_at: chrono::Utc::now(),
                }))
            }
            Command::UnloadLocalModel {
                provider_id,
                model_id,
            } => {
                let runtime = self.runtimes.get(&provider_id).await?;
                runtime.unload_model(&model_id).await?;
                Ok(CommandResponse::LocalModelUnloaded(
                    runtime.inspect().await?,
                ))
            }
            Command::StartModelDownload {
                provider_id,
                model_id,
            } => {
                let downloads = self
                    .downloads
                    .as_ref()
                    .ok_or(CoreError::DownloadCoordinatorNotConfigured)?;
                let runtime = self.runtimes.get(&provider_id).await?;
                Ok(CommandResponse::ModelDownloadStarted(
                    downloads.start(runtime, model_id).await?,
                ))
            }
            Command::CancelModelDownload { job_id } => {
                let downloads = self
                    .downloads
                    .as_ref()
                    .ok_or(CoreError::DownloadCoordinatorNotConfigured)?;
                Ok(CommandResponse::ModelDownloadCancelled(
                    downloads.cancel(&job_id).await?,
                ))
            }
            Command::ListModelDownloads => {
                let downloads = self
                    .downloads
                    .as_ref()
                    .ok_or(CoreError::DownloadCoordinatorNotConfigured)?;
                Ok(CommandResponse::ModelDownloads(downloads.list().await))
            }
            Command::SelectModel { model } => {
                self.models.get(&model).await?;
                *self.selected_model.write().await = Some(model.clone());
                Ok(CommandResponse::ModelSelected(model))
            }
            Command::Generate {
                model,
                messages,
                context,
                parameters,
            } => {
                let response = self
                    .generate_prepared(model, messages, *context, parameters)
                    .await?;
                Ok(CommandResponse::Generated(response))
            }
            Command::StoreProviderSecret { provider_id, value } => {
                let id = SecretId::provider_api_key(&provider_id);
                self.secrets.put(&id, value).await?;
                Ok(CommandResponse::SecretStored)
            }
            Command::DeleteProviderSecret { provider_id } => {
                let id = SecretId::provider_api_key(&provider_id);
                let deleted = self.secrets.delete(&id).await?;
                Ok(CommandResponse::SecretDeleted(deleted))
            }
            Command::ProposeMemory { proposal, permit } => {
                let memory = self.memory.read().await;
                let memory = memory.as_ref().ok_or(CoreError::MemoryNotConfigured)?;
                let stored = memory.approve_and_store(*proposal, &permit).await?;
                Ok(CommandResponse::MemoryStored(stored))
            }
            Command::RecallMemory { query } => {
                let memory = self.memory.read().await;
                let memory = memory.as_ref().ok_or(CoreError::MemoryNotConfigured)?;
                let records = memory.repository().search(*query).await?;
                Ok(CommandResponse::Memories(records))
            }
            Command::BuildContext { request } => {
                let builder = self.context_builder.read().await;
                let builder = builder.as_ref().ok_or(CoreError::ContextNotConfigured)?;
                Ok(CommandResponse::ContextBuilt(
                    builder.build(*request).await?,
                ))
            }
            Command::Reason {
                model,
                messages,
                context_request,
                parameters,
            } => {
                let builder = self.context_builder.read().await;
                let builder = builder.as_ref().ok_or(CoreError::ContextNotConfigured)?;
                let built = builder.build(*context_request).await?;
                let response = self
                    .generate_prepared(model, messages, built.prepared, parameters)
                    .await?;
                Ok(CommandResponse::Reasoned {
                    response,
                    context_report: built.report,
                })
            }
        }
    }

    async fn generate_prepared(
        &self,
        model: Option<ModelRef>,
        messages: Vec<Message>,
        context: vertex_ai_context::PreparedContext,
        parameters: GenerationParameters,
    ) -> Result<GenerationResponse, CoreError> {
        let model = match model {
            Some(model) => model,
            None => self
                .selected_model
                .read()
                .await
                .clone()
                .ok_or(CoreError::ModelNotSelected)?,
        };
        let descriptor = self.models.get(&model).await?;
        let expected_location = if descriptor.local {
            TargetLocation::Local
        } else {
            TargetLocation::Cloud
        };
        if context.target_location() != expected_location {
            return Err(CoreError::Context(
                vertex_ai_context::ContextError::Invalid(
                    "prepared context target does not match selected model location".to_owned(),
                ),
            ));
        }
        let provider = self.providers.get(&model.provider_id).await?;
        Ok(provider
            .generate(GenerationRequest {
                model,
                messages,
                context: context.into_context(),
                parameters,
            })
            .await?)
    }

    async fn refresh_provider_models(&self, id: &ProviderId) -> Result<(), CoreError> {
        let provider = self.providers.get(id).await?;
        let models = provider.list_models().await?;
        self.models.replace_provider_models(id, models).await;
        Ok(())
    }
}
