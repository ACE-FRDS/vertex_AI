//! Local Ollama adapter. Requests are loopback-only and never fall back to cloud providers.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;
use vertex_ai_provider::{
    CostEstimate, GenerationStream, ModelProvider, ProviderCapabilities, ProviderError,
    ProviderHealth,
};
use vertex_ai_runtime::{
    LocalRuntimeManager, ModelRuntimeAdapter, RuntimeError, RuntimeModelState,
    RuntimeModelStateKind, RuntimeOperationControl,
};
use vertex_ai_types::{
    GenerationRequest, GenerationResponse, HealthState, InstalledLocalModel, LoadedLocalModel,
    LocalRuntimeSnapshot, MessageRole, ModelCapabilities, ModelDescriptor, ModelDownloadProgress,
    ModelId, ModelRef, ProviderId, TokenUsage,
};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434";

#[derive(Debug, Clone)]
pub struct OllamaProviderConfig {
    pub base_url: String,
    pub request_timeout: Duration,
    pub health_timeout: Duration,
}

impl Default for OllamaProviderConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            request_timeout: Duration::from_secs(180),
            health_timeout: Duration::from_secs(3),
        }
    }
}

pub struct OllamaProvider {
    id: ProviderId,
    config: OllamaProviderConfig,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(config: OllamaProviderConfig) -> Result<Self, ProviderError> {
        let base_url = config.base_url.trim_end_matches('/');
        let parsed = reqwest::Url::parse(base_url)
            .map_err(|_| ProviderError::InvalidRequest("invalid Ollama base URL".to_owned()))?;
        let loopback = matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "::1"));
        if !loopback || parsed.scheme() != "http" {
            return Err(ProviderError::InvalidRequest(
                "Ollama endpoint must use HTTP on the loopback interface".to_owned(),
            ));
        }
        let client = reqwest::Client::builder()
            .connect_timeout(config.health_timeout)
            .build()
            .map_err(|_| ProviderError::Failed("failed to build Ollama HTTP client".to_owned()))?;
        Ok(Self {
            id: ProviderId::new("ollama")
                .map_err(|_| ProviderError::Failed("invalid provider id".to_owned()))?,
            config: OllamaProviderConfig {
                base_url: base_url.to_owned(),
                ..config
            },
            client,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.config.base_url, path.trim_start_matches('/'))
    }

    fn validate_request(&self, request: &GenerationRequest) -> Result<(), ProviderError> {
        if request.model.provider_id != self.id {
            return Err(ProviderError::InvalidRequest(
                "request model belongs to a different provider".to_owned(),
            ));
        }
        if !request.context.privacy_policy.local_only
            || request.context.privacy_policy.cloud_allowed
        {
            return Err(ProviderError::InvalidRequest(
                "Ollama requests require a local-only context".to_owned(),
            ));
        }
        Ok(())
    }

    async fn success(
        &self,
        request: reqwest::RequestBuilder,
        operation: &str,
    ) -> Result<reqwest::Response, ProviderError> {
        let response = request
            .send()
            .await
            .map_err(|_| ProviderError::Unavailable(format!("Ollama {operation} failed")))?;
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let message = format!("Ollama {operation} returned HTTP {}", status.as_u16());
        if status.is_server_error() || status.as_u16() == 429 {
            Err(ProviderError::Unavailable(message))
        } else {
            Err(ProviderError::InvalidRequest(message))
        }
    }

    async fn runtime_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        operation: &str,
    ) -> Result<T, RuntimeError> {
        self.client
            .get(self.endpoint(path))
            .timeout(self.config.health_timeout)
            .send()
            .await
            .map_err(|_| RuntimeError::Unavailable(format!("Ollama {operation} failed")))?
            .error_for_status()
            .map_err(|error| {
                RuntimeError::Unavailable(format!(
                    "Ollama {operation} returned HTTP {}",
                    error
                        .status()
                        .map(|status| status.as_u16().to_string())
                        .unwrap_or_else(|| "unknown".to_owned())
                ))
            })?
            .json::<T>()
            .await
            .map_err(|_| RuntimeError::Failed(format!("invalid Ollama {operation} response")))
    }

    fn executable_path() -> Option<String> {
        let executable = if cfg!(windows) {
            "ollama.exe"
        } else {
            "ollama"
        };
        if let Some(path) = env::var_os("PATH") {
            for root in env::split_paths(&path) {
                let candidate = root.join(executable);
                if candidate.is_file() {
                    return Some(candidate.to_string_lossy().into_owned());
                }
            }
        }
        if cfg!(windows)
            && let Some(local_app_data) = env::var_os("LOCALAPPDATA")
        {
            let candidate = Path::new(&local_app_data)
                .join("Programs")
                .join("Ollama")
                .join("ollama.exe");
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
        None
    }

    fn model_storage_path() -> Option<String> {
        env::var_os("OLLAMA_MODELS")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                    .map(|home| Path::new(&home).join(".ollama").join("models"))
            })
            .map(|path| path.to_string_lossy().into_owned())
    }

    fn storage_stats(path: Option<&str>) -> (Option<u64>, Option<u64>) {
        let Some(path) = path else {
            return (None, None);
        };
        let mut candidate = PathBuf::from(path);
        while !candidate.exists() {
            if !candidate.pop() {
                return (None, None);
            }
        }
        (
            fs2::total_space(&candidate).ok(),
            fs2::available_space(&candidate).ok(),
        )
    }
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            model_discovery: true,
            streaming: false,
            cost_estimation: true,
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, ProviderError> {
        let response = self
            .success(
                self.client
                    .get(self.endpoint("api/tags"))
                    .timeout(self.config.health_timeout),
                "model discovery",
            )
            .await?
            .json::<OllamaTagsResponse>()
            .await
            .map_err(|_| ProviderError::Failed("invalid Ollama model response".to_owned()))?;

        response
            .models
            .into_iter()
            .map(|model| {
                let model_id = ModelId::new(model.name).map_err(|_| {
                    ProviderError::Failed("Ollama returned an empty model name".to_owned())
                })?;
                let mut metadata = BTreeMap::new();
                metadata.insert("size_bytes".to_owned(), json!(model.size));
                metadata.insert("digest".to_owned(), Value::String(model.digest));
                let details = model.details.unwrap_or_default();
                let context_size = details
                    .context_length
                    .and_then(|value| u32::try_from(value).ok());
                if details.format.is_some() || details.family.is_some() {
                    metadata.insert("format".to_owned(), json!(details.format));
                    metadata.insert("family".to_owned(), json!(details.family));
                    metadata.insert("parameter_size".to_owned(), json!(details.parameter_size));
                    metadata.insert(
                        "quantization_level".to_owned(),
                        json!(details.quantization_level),
                    );
                }
                Ok(ModelDescriptor {
                    display_name: model_id.as_str().to_owned(),
                    reference: ModelRef::new(self.id.clone(), model_id),
                    capabilities: ModelCapabilities {
                        structured_output: true,
                        streaming: false,
                        ..ModelCapabilities::default()
                    },
                    context_size,
                    local: true,
                    input_cost_per_million: Some(0.0),
                    output_cost_per_million: Some(0.0),
                    available: true,
                    metadata,
                })
            })
            .collect()
    }

    async fn generate(
        &self,
        request: GenerationRequest,
    ) -> Result<GenerationResponse, ProviderError> {
        self.validate_request(&request)?;
        let context = serde_json::to_string(&request.context).map_err(|_| {
            ProviderError::InvalidRequest("context serialization failed".to_owned())
        })?;
        let mut messages = vec![OllamaMessage {
            role: "system",
            content: format!("Vertex Context Protocol JSON:\n{context}"),
        }];
        for message in &request.messages {
            let role = match message.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => {
                    return Err(ProviderError::Unsupported(
                        "tool messages are not implemented by the Ollama adapter yet".to_owned(),
                    ));
                }
            };
            messages.push(OllamaMessage {
                role,
                content: message.content.clone(),
            });
        }
        let wire = OllamaChatRequest {
            model: request.model.model_id.as_str(),
            messages,
            stream: false,
            think: false,
            format: request.parameters.structured_output_schema.as_ref(),
            options: OllamaOptions {
                temperature: request.parameters.temperature,
                num_predict: request.parameters.max_output_tokens,
                stop: request.parameters.stop,
            },
        };
        let response = self
            .success(
                self.client
                    .post(self.endpoint("api/chat"))
                    .timeout(self.config.request_timeout)
                    .json(&wire),
                "generation",
            )
            .await?
            .json::<OllamaChatResponse>()
            .await
            .map_err(|_| ProviderError::Failed("invalid Ollama generation response".to_owned()))?;
        if response.message.content.trim().is_empty() {
            return Err(ProviderError::Failed(
                "Ollama response contained no text output".to_owned(),
            ));
        }
        Ok(GenerationResponse {
            response_id: Uuid::new_v4(),
            model: request.model,
            text: response.message.content,
            usage: TokenUsage {
                input_tokens: response.prompt_eval_count.unwrap_or(0),
                output_tokens: response.eval_count.unwrap_or(0),
            },
            finish_reason: response.done_reason,
            created_at: Utc::now(),
        })
    }

    async fn stream(&self, _request: GenerationRequest) -> Result<GenerationStream, ProviderError> {
        Err(ProviderError::Unsupported(
            "Ollama streaming is not implemented yet".to_owned(),
        ))
    }

    async fn health(&self) -> ProviderHealth {
        match self
            .client
            .get(self.endpoint("api/version"))
            .timeout(self.config.health_timeout)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => ProviderHealth::Healthy,
            Ok(response) => ProviderHealth::Degraded(format!(
                "Ollama returned HTTP {}",
                response.status().as_u16()
            )),
            Err(_) => ProviderHealth::Unavailable("Ollama is not reachable".to_owned()),
        }
    }

    fn estimate_cost(&self, _request: &GenerationRequest) -> Option<CostEstimate> {
        Some(CostEstimate {
            amount: 0.0,
            currency: "USD",
        })
    }
}

#[async_trait]
impl LocalRuntimeManager for OllamaProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    async fn inspect(&self) -> Result<LocalRuntimeSnapshot, RuntimeError> {
        let checked_at = Utc::now();
        let model_storage_path = Self::model_storage_path();
        let (storage_total_bytes, storage_available_bytes) =
            Self::storage_stats(model_storage_path.as_deref());
        let version = match self
            .runtime_json::<OllamaVersionResponse>("api/version", "version check")
            .await
        {
            Ok(response) => response.version,
            Err(_) => {
                return Ok(LocalRuntimeSnapshot {
                    provider_id: self.id.clone(),
                    display_name: "Ollama".to_owned(),
                    endpoint: self.config.base_url.clone(),
                    health: HealthState::Offline,
                    version: None,
                    executable_path: Self::executable_path(),
                    model_storage_path,
                    storage_total_bytes,
                    storage_available_bytes,
                    installed_models: Vec::new(),
                    loaded_models: Vec::new(),
                    checked_at,
                });
            }
        };
        let tags = self
            .runtime_json::<OllamaTagsResponse>("api/tags", "model inventory")
            .await?;
        let running = self
            .runtime_json::<OllamaPsResponse>("api/ps", "running model inventory")
            .await?;

        let installed_models = tags
            .models
            .into_iter()
            .map(|model| {
                let model_id = ModelId::new(model.name.clone()).map_err(|_| {
                    RuntimeError::Failed("Ollama returned an empty model name".to_owned())
                })?;
                let details = model.details.unwrap_or_default();
                Ok(InstalledLocalModel {
                    reference: ModelRef::new(self.id.clone(), model_id),
                    display_name: model.name,
                    size_bytes: model.size,
                    digest: non_empty(model.digest),
                    format: details.format,
                    family: details.family,
                    parameter_size: details.parameter_size,
                    quantization_level: details.quantization_level,
                    context_length: details.context_length,
                    modified_at: model.modified_at,
                })
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let loaded_models = running
            .models
            .into_iter()
            .map(|model| {
                let model_id = ModelId::new(model.name).map_err(|_| {
                    RuntimeError::Failed("Ollama returned an empty running model name".to_owned())
                })?;
                Ok(LoadedLocalModel {
                    reference: ModelRef::new(self.id.clone(), model_id),
                    size_bytes: model.size,
                    size_vram_bytes: model.size_vram,
                    context_length: model.context_length,
                    expires_at: model.expires_at,
                })
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;

        Ok(LocalRuntimeSnapshot {
            provider_id: self.id.clone(),
            display_name: "Ollama".to_owned(),
            endpoint: self.config.base_url.clone(),
            health: HealthState::Ready,
            version: Some(version),
            executable_path: Self::executable_path(),
            model_storage_path,
            storage_total_bytes,
            storage_available_bytes,
            installed_models,
            loaded_models,
            checked_at,
        })
    }

    async fn unload_model(&self, model_id: &ModelId) -> Result<(), RuntimeError> {
        let response = self
            .client
            .post(self.endpoint("api/generate"))
            .timeout(self.config.health_timeout)
            .json(&OllamaUnloadRequest {
                model: model_id.as_str(),
                keep_alive: 0,
                stream: false,
            })
            .send()
            .await
            .map_err(|_| RuntimeError::Unavailable("Ollama unload request failed".to_owned()))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(RuntimeError::Failed(format!(
                "Ollama unload returned HTTP {}",
                response.status().as_u16()
            )))
        }
    }

    async fn download_model(
        &self,
        model_id: &ModelId,
        progress: tokio::sync::mpsc::UnboundedSender<ModelDownloadProgress>,
        mut cancellation: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), RuntimeError> {
        if *cancellation.borrow() {
            return Err(RuntimeError::Cancelled);
        }
        let response = self
            .client
            .post(self.endpoint("api/pull"))
            .json(&OllamaPullRequest {
                model: model_id.as_str(),
                stream: true,
            })
            .send()
            .await
            .map_err(|_| RuntimeError::Unavailable("Ollama pull request failed".to_owned()))?;
        let response = response.error_for_status().map_err(|error| {
            RuntimeError::Failed(format!(
                "Ollama pull returned HTTP {}",
                error
                    .status()
                    .map(|status| status.as_u16().to_string())
                    .unwrap_or_else(|| "unknown".to_owned())
            ))
        })?;
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        loop {
            tokio::select! {
                changed = cancellation.changed() => {
                    if changed.is_ok() && *cancellation.borrow() {
                        return Err(RuntimeError::Cancelled);
                    }
                }
                chunk = stream.next() => {
                    let Some(chunk) = chunk else { break };
                    let chunk = chunk.map_err(|_| RuntimeError::Unavailable("Ollama pull stream failed".to_owned()))?;
                    buffer.push_str(&String::from_utf8_lossy(&chunk));
                    while let Some(newline) = buffer.find('\n') {
                        let line = buffer[..newline].trim().to_owned();
                        buffer.drain(..=newline);
                        if line.is_empty() { continue; }
                        let update: OllamaPullProgress = serde_json::from_str(&line)
                            .map_err(|_| RuntimeError::Failed("invalid Ollama pull progress".to_owned()))?;
                        if let Some(error) = update.error {
                            return Err(RuntimeError::Failed(error));
                        }
                        let _ = progress.send(ModelDownloadProgress {
                            status: update.status.clone(),
                            completed_bytes: update.completed.unwrap_or(0),
                            total_bytes: update.total,
                        });
                        if update.status == "success" {
                            return Ok(());
                        }
                    }
                }
            }
        }
        Err(RuntimeError::Failed(
            "Ollama pull stream ended before success".to_owned(),
        ))
    }
}

#[async_trait]
impl ModelRuntimeAdapter for OllamaProvider {
    fn runtime_id(&self) -> &ProviderId {
        &self.id
    }

    async fn model_state(&self, model_id: &ModelId) -> Result<RuntimeModelState, RuntimeError> {
        let snapshot = self.inspect().await?;
        let installed = snapshot
            .installed_models
            .iter()
            .any(|model| &model.reference.model_id == model_id);
        let loaded = snapshot
            .loaded_models
            .iter()
            .any(|model| &model.reference.model_id == model_id);
        let (state, detail) = if snapshot.health != HealthState::Ready {
            (
                RuntimeModelStateKind::Unavailable,
                "Ollama runtime is unavailable".to_owned(),
            )
        } else if loaded {
            (
                RuntimeModelStateKind::Loaded,
                "Ollama /api/ps reports the model as loaded".to_owned(),
            )
        } else if installed {
            (
                RuntimeModelStateKind::Unloaded,
                "Model is installed but absent from Ollama /api/ps".to_owned(),
            )
        } else {
            (
                RuntimeModelStateKind::Unavailable,
                "Model is not installed in Ollama".to_owned(),
            )
        };
        Ok(RuntimeModelState {
            runtime_id: self.id.clone(),
            model_id: model_id.clone(),
            state,
            observed: true,
            detail,
            observed_at: Utc::now(),
        })
    }

    async fn load_model(
        &self,
        model_id: &ModelId,
        mut control: tokio::sync::watch::Receiver<RuntimeOperationControl>,
    ) -> Result<RuntimeModelState, RuntimeError> {
        ensure_runtime_operation_continues(*control.borrow())?;
        let response = self
            .client
            .post(self.endpoint("api/generate"))
            .timeout(self.config.request_timeout)
            .json(&OllamaResidencyRequest {
                model: model_id.as_str(),
                prompt: "",
                keep_alive: -1,
                stream: false,
            })
            .send();
        tokio::select! {
            result = response => {
                result
                    .map_err(|_| RuntimeError::Unavailable("Ollama preload request failed".to_owned()))?
                    .error_for_status()
                    .map_err(|error| RuntimeError::Failed(format!("Ollama preload returned HTTP {}", error.status().map_or_else(|| "unknown".to_owned(), |status| status.as_u16().to_string()))))?;
            }
            changed = control.changed() => {
                changed.map_err(|_| RuntimeError::Cancelled)?;
                ensure_runtime_operation_continues(*control.borrow())?;
            }
        }
        self.wait_for_model_state(model_id, RuntimeModelStateKind::Loaded, control)
            .await
    }

    async fn release_model(
        &self,
        model_id: &ModelId,
        mut control: tokio::sync::watch::Receiver<RuntimeOperationControl>,
    ) -> Result<RuntimeModelState, RuntimeError> {
        ensure_runtime_operation_continues(*control.borrow())?;
        let response = self
            .client
            .post(self.endpoint("api/generate"))
            .timeout(self.config.request_timeout)
            .json(&OllamaResidencyRequest {
                model: model_id.as_str(),
                prompt: "",
                keep_alive: 0,
                stream: false,
            })
            .send();
        tokio::select! {
            result = response => {
                result
                    .map_err(|_| RuntimeError::Unavailable("Ollama unload request failed".to_owned()))?
                    .error_for_status()
                    .map_err(|error| RuntimeError::Failed(format!("Ollama unload returned HTTP {}", error.status().map_or_else(|| "unknown".to_owned(), |status| status.as_u16().to_string()))))?;
            }
            changed = control.changed() => {
                changed.map_err(|_| RuntimeError::Cancelled)?;
                ensure_runtime_operation_continues(*control.borrow())?;
            }
        }
        self.wait_for_model_state(model_id, RuntimeModelStateKind::Unloaded, control)
            .await
    }
}

impl OllamaProvider {
    async fn wait_for_model_state(
        &self,
        model_id: &ModelId,
        expected: RuntimeModelStateKind,
        mut control: tokio::sync::watch::Receiver<RuntimeOperationControl>,
    ) -> Result<RuntimeModelState, RuntimeError> {
        for _ in 0..20 {
            ensure_runtime_operation_continues(*control.borrow())?;
            let state = <Self as ModelRuntimeAdapter>::model_state(self, model_id).await?;
            if state.state == expected
                || (expected == RuntimeModelStateKind::Unloaded
                    && state.state == RuntimeModelStateKind::Unavailable)
            {
                return Ok(state);
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(250)) => {}
                changed = control.changed() => {
                    changed.map_err(|_| RuntimeError::Cancelled)?;
                    ensure_runtime_operation_continues(*control.borrow())?;
                }
            }
        }
        Err(RuntimeError::Failed(format!(
            "Ollama model state did not reach {expected:?}"
        )))
    }
}

fn ensure_runtime_operation_continues(
    control: RuntimeOperationControl,
) -> Result<(), RuntimeError> {
    match control {
        RuntimeOperationControl::Continue => Ok(()),
        RuntimeOperationControl::Pause | RuntimeOperationControl::Cancel => {
            Err(RuntimeError::Cancelled)
        }
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: String,
    modified_at: Option<DateTime<Utc>>,
    details: Option<OllamaModelDetails>,
}

#[derive(Debug, Deserialize, Default)]
struct OllamaModelDetails {
    format: Option<String>,
    family: Option<String>,
    parameter_size: Option<String>,
    quantization_level: Option<String>,
    context_length: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OllamaVersionResponse {
    version: String,
}

#[derive(Debug, Deserialize)]
struct OllamaPsResponse {
    models: Vec<OllamaRunningModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaRunningModel {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    size_vram: u64,
    context_length: Option<u64>,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct OllamaUnloadRequest<'a> {
    model: &'a str,
    keep_alive: u8,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OllamaResidencyRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    keep_alive: i8,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OllamaPullRequest<'a> {
    model: &'a str,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaPullProgress {
    #[serde(default)]
    status: String,
    completed: Option<u64>,
    total: Option<u64>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage<'a>>,
    stream: bool,
    think: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'a Value>,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
    done_reason: Option<String>,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::{MockServer, prelude::*};
    use vertex_ai_types::{GenerationParameters, Message, PrivacyPolicy, VertexContext};

    fn provider(server: &MockServer) -> OllamaProvider {
        OllamaProvider::new(OllamaProviderConfig {
            base_url: server.base_url(),
            ..OllamaProviderConfig::default()
        })
        .expect("provider")
    }

    fn local_request() -> GenerationRequest {
        let context = VertexContext {
            privacy_policy: PrivacyPolicy {
                local_only: true,
                cloud_allowed: false,
                ..PrivacyPolicy::default()
            },
            ..VertexContext::default()
        };
        GenerationRequest {
            model: ModelRef::new(
                ProviderId::new("ollama").expect("provider id"),
                ModelId::new("qwen3:8b").expect("model id"),
            ),
            messages: vec![Message::user("こんにちは")],
            context,
            parameters: GenerationParameters::default(),
        }
    }

    #[tokio::test]
    async fn discovers_local_models() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/api/tags");
            then.status(200).json_body(json!({
                "models": [{
                    "name": "qwen3:8b",
                    "size": 123,
                    "digest": "sha256:abc",
                    "details": {"format": "gguf", "family": "qwen3", "parameter_size": "8B", "quantization_level": "Q4_K_M"}
                }]
            }));
        });
        let models = provider(&server).list_models().await.expect("models");
        mock.assert();
        assert_eq!(models.len(), 1);
        assert!(models[0].local);
        assert_eq!(models[0].reference.model_id.as_str(), "qwen3:8b");
    }

    #[tokio::test]
    async fn generates_without_cloud_fallback() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/api/chat")
                .json_body_includes(r#"{"model":"qwen3:8b","stream":false,"think":false}"#);
            then.status(200).json_body(json!({
                "message": {"role": "assistant", "content": "応答"},
                "done_reason": "stop",
                "prompt_eval_count": 7,
                "eval_count": 2
            }));
        });
        let response = provider(&server)
            .generate(local_request())
            .await
            .expect("response");
        mock.assert();
        assert_eq!(response.text, "応答");
        assert_eq!(response.usage.input_tokens, 7);
    }

    #[tokio::test]
    async fn inspects_and_unloads_runtime_models() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/version");
            then.status(200).json_body(json!({"version": "0.32.9"}));
        });
        server.mock(|when, then| {
            when.method(GET).path("/api/tags");
            then.status(200).json_body(json!({
                "models": [{
                    "name": "qwen3:8b", "size": 5200000000_u64, "digest": "sha256:abc",
                    "modified_at": "2026-08-12T08:00:00Z",
                    "details": {"format": "gguf", "family": "qwen3", "parameter_size": "8B", "quantization_level": "Q4_K_M"}
                }]
            }));
        });
        server.mock(|when, then| {
            when.method(GET).path("/api/ps");
            then.status(200).json_body(json!({
                "models": [{
                    "name": "qwen3:8b", "size": 6000000000_u64, "size_vram": 5100000000_u64,
                    "context_length": 4096, "expires_at": "2026-08-12T09:00:00Z"
                }]
            }));
        });
        let unload = server.mock(|when, then| {
            when.method(POST)
                .path("/api/generate")
                .json_body_includes(r#"{"model":"qwen3:8b","keep_alive":0,"stream":false}"#);
            then.status(200).json_body(json!({"done": true}));
        });
        let provider = provider(&server);
        let snapshot = provider.inspect().await.expect("runtime snapshot");
        assert_eq!(snapshot.version.as_deref(), Some("0.32.9"));
        assert_eq!(snapshot.installed_models.len(), 1);
        assert_eq!(snapshot.loaded_models[0].size_vram_bytes, 5_100_000_000);
        provider
            .unload_model(&ModelId::new("qwen3:8b").expect("model id"))
            .await
            .expect("unload");
        unload.assert();
    }

    #[tokio::test]
    async fn streams_model_download_progress_to_the_runtime_coordinator() {
        let server = MockServer::start();
        let pull = server.mock(|when, then| {
            when.method(POST)
                .path("/api/pull")
                .json_body_includes(r#"{"model":"qwen3:4b","stream":true}"#);
            then.status(200)
                .header("content-type", "application/x-ndjson")
                .body(
                    "{\"status\":\"downloading\",\"completed\":50,\"total\":100}\n{\"status\":\"success\"}\n",
                );
        });
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
        let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        provider(&server)
            .download_model(
                &ModelId::new("qwen3:4b").expect("model id"),
                progress_tx,
                cancel_rx,
            )
            .await
            .expect("download completes");
        pull.assert();
        let updates = std::iter::from_fn(|| progress_rx.try_recv().ok()).collect::<Vec<_>>();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].completed_bytes, 50);
        assert_eq!(updates[0].total_bytes, Some(100));
        assert_eq!(updates[1].status, "success");
    }

    #[tokio::test]
    async fn runtime_adapter_preloads_and_observes_loaded_model() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/version");
            then.status(200).json_body(json!({"version": "0.32.9"}));
        });
        server.mock(|when, then| {
            when.method(GET).path("/api/tags");
            then.status(200).json_body(json!({"models": [{
                "name": "qwen3:8b", "size": 123, "digest": "sha256:abc"
            }]}));
        });
        server.mock(|when, then| {
            when.method(GET).path("/api/ps");
            then.status(200).json_body(json!({"models": [{
                "name": "qwen3:8b", "size": 123, "size_vram": 100,
                "context_length": 32768
            }]}));
        });
        let preload = server.mock(|when, then| {
            when.method(POST).path("/api/generate").json_body_includes(
                r#"{"model":"qwen3:8b","prompt":"","keep_alive":-1,"stream":false}"#,
            );
            then.status(200).json_body(json!({"done": true}));
        });
        let (_control_tx, control_rx) =
            tokio::sync::watch::channel(RuntimeOperationControl::Continue);
        let state = provider(&server)
            .load_model(&ModelId::new("qwen3:8b").unwrap(), control_rx)
            .await
            .expect("loaded state");
        preload.assert();
        assert_eq!(state.state, RuntimeModelStateKind::Loaded);
        assert!(state.observed);
    }

    #[tokio::test]
    async fn runtime_adapter_honors_cancel_before_network_operation() {
        let server = MockServer::start();
        let (_control_tx, control_rx) =
            tokio::sync::watch::channel(RuntimeOperationControl::Cancel);
        let result = provider(&server)
            .load_model(&ModelId::new("qwen3:8b").unwrap(), control_rx)
            .await;
        assert!(matches!(result, Err(RuntimeError::Cancelled)));
    }

    #[test]
    fn rejects_non_loopback_endpoints() {
        let result = OllamaProvider::new(OllamaProviderConfig {
            base_url: "http://example.com:11434".to_owned(),
            ..OllamaProviderConfig::default()
        });
        assert!(matches!(result, Err(ProviderError::InvalidRequest(_))));
    }
}
