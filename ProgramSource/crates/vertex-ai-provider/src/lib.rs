//! Provider-neutral interfaces, registries, and a deterministic mock provider.

use async_trait::async_trait;
use chrono::Utc;
use futures_core::Stream;
use futures_util::stream;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, pin::Pin, sync::Arc};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;
use vertex_ai_types::{
    GenerationRequest, GenerationResponse, ModelCapabilities, ModelDescriptor, ModelId, ModelRef,
    ProviderId, StreamChunk, TokenUsage,
};

pub type GenerationStream =
    Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub model_discovery: bool,
    pub streaming: bool,
    pub cost_estimation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "message", rename_all = "snake_case")]
pub enum ProviderHealth {
    Healthy,
    Degraded(String),
    Unavailable(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CostEstimate {
    pub amount: f64,
    pub currency: &'static str,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider is unavailable: {0}")]
    Unavailable(String),
    #[error("provider rejected the request: {0}")]
    InvalidRequest(String),
    #[error("provider operation is unsupported: {0}")]
    Unsupported(String),
    #[error("provider operation failed: {0}")]
    Failed(String),
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> &ProviderId;
    fn capabilities(&self) -> ProviderCapabilities;
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, ProviderError>;
    async fn generate(
        &self,
        request: GenerationRequest,
    ) -> Result<GenerationResponse, ProviderError>;
    async fn stream(&self, request: GenerationRequest) -> Result<GenerationStream, ProviderError>;
    async fn health(&self) -> ProviderHealth;
    fn estimate_cost(&self, request: &GenerationRequest) -> Option<CostEstimate>;
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("provider is already registered: {0}")]
    DuplicateProvider(ProviderId),
    #[error("provider is not registered: {0}")]
    ProviderNotFound(ProviderId),
    #[error("model is not registered: {0}/{1}")]
    ModelNotFound(ProviderId, ModelId),
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: RwLock<BTreeMap<ProviderId, Arc<dyn ModelProvider>>>,
}

impl ProviderRegistry {
    pub async fn register(&self, provider: Arc<dyn ModelProvider>) -> Result<(), RegistryError> {
        let id = provider.id().clone();
        let mut providers = self.providers.write().await;
        if providers.contains_key(&id) {
            return Err(RegistryError::DuplicateProvider(id));
        }
        providers.insert(id, provider);
        Ok(())
    }

    pub async fn get(&self, id: &ProviderId) -> Result<Arc<dyn ModelProvider>, RegistryError> {
        self.providers
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| RegistryError::ProviderNotFound(id.clone()))
    }

    pub async fn ids(&self) -> Vec<ProviderId> {
        self.providers.read().await.keys().cloned().collect()
    }
}

#[derive(Debug, Default)]
pub struct ModelRegistry {
    models: RwLock<BTreeMap<ModelRef, ModelDescriptor>>,
}

impl ModelRegistry {
    pub async fn replace_provider_models(
        &self,
        provider_id: &ProviderId,
        models: Vec<ModelDescriptor>,
    ) {
        let mut registry = self.models.write().await;
        registry.retain(|reference, _| &reference.provider_id != provider_id);
        registry.extend(
            models
                .into_iter()
                .map(|descriptor| (descriptor.reference.clone(), descriptor)),
        );
    }

    pub async fn get(&self, reference: &ModelRef) -> Result<ModelDescriptor, RegistryError> {
        self.models
            .read()
            .await
            .get(reference)
            .cloned()
            .ok_or_else(|| {
                RegistryError::ModelNotFound(
                    reference.provider_id.clone(),
                    reference.model_id.clone(),
                )
            })
    }

    pub async fn list(&self, provider_id: Option<&ProviderId>) -> Vec<ModelDescriptor> {
        self.models
            .read()
            .await
            .values()
            .filter(|model| provider_id.is_none_or(|id| &model.reference.provider_id == id))
            .cloned()
            .collect()
    }
}

#[derive(Debug)]
pub struct MockProvider {
    id: ProviderId,
    models: Vec<ModelDescriptor>,
}

impl MockProvider {
    pub fn new(id: ProviderId, model_ids: impl IntoIterator<Item = ModelId>) -> Self {
        let models = model_ids
            .into_iter()
            .map(|model_id| {
                let reference = ModelRef::new(id.clone(), model_id);
                ModelDescriptor {
                    display_name: format!("Mock {}", reference.model_id),
                    reference,
                    capabilities: ModelCapabilities {
                        streaming: true,
                        structured_output: true,
                        ..ModelCapabilities::default()
                    },
                    context_size: Some(8_192),
                    local: true,
                    input_cost_per_million: Some(0.0),
                    output_cost_per_million: Some(0.0),
                    available: true,
                    metadata: BTreeMap::new(),
                }
            })
            .collect();
        Self { id, models }
    }

    fn supports(&self, reference: &ModelRef) -> bool {
        self.models
            .iter()
            .any(|model| &model.reference == reference)
    }
}

#[async_trait]
impl ModelProvider for MockProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            model_discovery: true,
            streaming: true,
            cost_estimation: true,
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, ProviderError> {
        Ok(self.models.clone())
    }

    async fn generate(
        &self,
        request: GenerationRequest,
    ) -> Result<GenerationResponse, ProviderError> {
        if !self.supports(&request.model) {
            return Err(ProviderError::InvalidRequest(format!(
                "unknown model {}/{}",
                request.model.provider_id, request.model.model_id
            )));
        }
        let input = request
            .messages
            .last()
            .map(|message| message.content.as_str())
            .unwrap_or_default();
        Ok(GenerationResponse {
            response_id: Uuid::new_v4(),
            model: request.model,
            text: format!("mock:{input}"),
            usage: TokenUsage {
                input_tokens: input.split_whitespace().count() as u64,
                output_tokens: 1,
            },
            finish_reason: Some("stop".to_owned()),
            created_at: Utc::now(),
        })
    }

    async fn stream(&self, request: GenerationRequest) -> Result<GenerationStream, ProviderError> {
        let response = self.generate(request).await?;
        Ok(Box::pin(stream::iter([Ok(StreamChunk {
            text: response.text,
            finished: true,
        })])))
    }

    async fn health(&self) -> ProviderHealth {
        ProviderHealth::Healthy
    }

    fn estimate_cost(&self, _request: &GenerationRequest) -> Option<CostEstimate> {
        Some(CostEstimate {
            amount: 0.0,
            currency: "USD",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vertex_ai_types::{GenerationParameters, Message, VertexContext};

    fn model_ref() -> ModelRef {
        ModelRef::new(
            ProviderId::new("mock").unwrap(),
            ModelId::new("mock-1").unwrap(),
        )
    }

    #[tokio::test]
    async fn mock_provider_discovers_and_generates() {
        let reference = model_ref();
        let provider =
            MockProvider::new(reference.provider_id.clone(), [reference.model_id.clone()]);
        assert_eq!(provider.list_models().await.unwrap().len(), 1);

        let response = provider
            .generate(GenerationRequest {
                model: reference,
                messages: vec![Message::user("hello")],
                context: VertexContext::default(),
                parameters: GenerationParameters::default(),
            })
            .await
            .unwrap();
        assert_eq!(response.text, "mock:hello");
    }

    #[tokio::test]
    async fn registry_rejects_duplicate_provider() {
        let registry = ProviderRegistry::default();
        let id = ProviderId::new("mock").unwrap();
        registry
            .register(Arc::new(MockProvider::new(id.clone(), [])))
            .await
            .unwrap();
        let error = registry
            .register(Arc::new(MockProvider::new(id, [])))
            .await
            .unwrap_err();
        assert!(matches!(error, RegistryError::DuplicateProvider(_)));
    }
}
