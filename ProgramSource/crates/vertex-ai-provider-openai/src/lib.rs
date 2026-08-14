//! OpenAI Responses API adapter. OpenAI-specific wire types stay in this crate.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use uuid::Uuid;
use vertex_ai_provider::{
    CostEstimate, GenerationStream, ModelProvider, ProviderCapabilities, ProviderError,
    ProviderHealth,
};
use vertex_ai_secrets::{SecretId, SecretStore};
use vertex_ai_types::{
    GenerationRequest, GenerationResponse, MessageRole, ModelCapabilities, ModelDescriptor,
    ModelId, ModelRef, ProviderId, TokenUsage,
};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone)]
pub struct OpenAiProviderConfig {
    pub base_url: String,
    pub request_timeout: Duration,
}

impl Default for OpenAiProviderConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            request_timeout: Duration::from_secs(60),
        }
    }
}

pub struct OpenAiProvider {
    id: ProviderId,
    config: OpenAiProviderConfig,
    client: reqwest::Client,
    secrets: Arc<dyn SecretStore>,
}

impl OpenAiProvider {
    pub fn new(
        config: OpenAiProviderConfig,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Self, ProviderError> {
        let base_url = config.base_url.trim_end_matches('/');
        reqwest::Url::parse(base_url)
            .map_err(|_| ProviderError::InvalidRequest("invalid OpenAI base URL".to_owned()))?;
        let client = reqwest::Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(|_| ProviderError::Failed("failed to build HTTP client".to_owned()))?;
        Ok(Self {
            id: ProviderId::new("openai")
                .map_err(|_| ProviderError::Failed("invalid provider id".to_owned()))?,
            config: OpenAiProviderConfig {
                base_url: base_url.to_owned(),
                ..config
            },
            client,
            secrets,
        })
    }

    async fn api_key(&self) -> Result<vertex_ai_secrets::SecretValue, ProviderError> {
        self.secrets
            .get(&SecretId::provider_api_key(&self.id))
            .await
            .map_err(|_| {
                ProviderError::Unavailable("OpenAI credential store unavailable".to_owned())
            })?
            .ok_or_else(|| {
                ProviderError::Unavailable("OpenAI API key is not configured".to_owned())
            })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.config.base_url, path.trim_start_matches('/'))
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, ProviderError> {
        let response = request
            .send()
            .await
            .map_err(|_| ProviderError::Unavailable("OpenAI request failed".to_owned()))?;
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let code = response
            .json::<OpenAiErrorEnvelope>()
            .await
            .ok()
            .and_then(|error| error.error.code)
            .unwrap_or_else(|| "unknown_error".to_owned());
        let message = format!("OpenAI returned HTTP {} ({code})", status.as_u16());
        if status.is_server_error() || status.as_u16() == 429 {
            Err(ProviderError::Unavailable(message))
        } else {
            Err(ProviderError::InvalidRequest(message))
        }
    }

    fn build_response_request(
        &self,
        request: &GenerationRequest,
    ) -> Result<OpenAiResponseRequest, ProviderError> {
        let privacy = &request.context.privacy_policy;
        if privacy.local_only || !privacy.cloud_allowed {
            return Err(ProviderError::InvalidRequest(
                "Vertex Context privacy policy forbids cloud processing".to_owned(),
            ));
        }
        if request.parameters.structured_output_schema.is_some() {
            return Err(ProviderError::Unsupported(
                "structured output is not implemented by this adapter yet".to_owned(),
            ));
        }

        let context_json = serde_json::to_string(&request.context).map_err(|_| {
            ProviderError::InvalidRequest("context serialization failed".to_owned())
        })?;
        let mut input = vec![OpenAiInputMessage {
            role: "developer",
            content: format!("Vertex Context Protocol JSON:\n{context_json}"),
        }];
        for message in &request.messages {
            let role = match message.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => {
                    return Err(ProviderError::Unsupported(
                        "tool messages are not implemented by this adapter yet".to_owned(),
                    ));
                }
            };
            input.push(OpenAiInputMessage {
                role,
                content: message.content.clone(),
            });
        }
        Ok(OpenAiResponseRequest {
            model: request.model.model_id.as_str().to_owned(),
            input,
            store: false,
            temperature: request.parameters.temperature,
            max_output_tokens: request.parameters.max_output_tokens,
        })
    }
}

#[async_trait]
impl ModelProvider for OpenAiProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            model_discovery: true,
            streaming: false,
            cost_estimation: false,
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, ProviderError> {
        let api_key = self.api_key().await?;
        let response = self
            .send(
                self.client
                    .get(self.endpoint("models"))
                    .bearer_auth(api_key.expose()),
            )
            .await?
            .json::<OpenAiModelList>()
            .await
            .map_err(|_| ProviderError::Failed("invalid OpenAI model response".to_owned()))?;

        response
            .data
            .into_iter()
            .map(|model| {
                let model_id = ModelId::new(model.id).map_err(|_| {
                    ProviderError::Failed("OpenAI returned an empty model id".to_owned())
                })?;
                let mut metadata = BTreeMap::new();
                metadata.insert("owned_by".to_owned(), Value::String(model.owned_by));
                metadata.insert("created".to_owned(), json!(model.created));
                Ok(ModelDescriptor {
                    display_name: model_id.as_str().to_owned(),
                    reference: ModelRef::new(self.id.clone(), model_id),
                    capabilities: ModelCapabilities::default(),
                    context_size: None,
                    local: false,
                    input_cost_per_million: None,
                    output_cost_per_million: None,
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
        let api_key = self.api_key().await?;
        let wire_request = self.build_response_request(&request)?;
        let response = self
            .send(
                self.client
                    .post(self.endpoint("responses"))
                    .bearer_auth(api_key.expose())
                    .json(&wire_request),
            )
            .await?
            .json::<OpenAiResponse>()
            .await
            .map_err(|_| ProviderError::Failed("invalid OpenAI response payload".to_owned()))?;
        let text = response
            .output
            .into_iter()
            .flat_map(|item| item.content)
            .filter(|content| content.kind == "output_text")
            .filter_map(|content| content.text)
            .collect::<Vec<_>>()
            .join("");
        if text.is_empty() {
            return Err(ProviderError::Failed(
                "OpenAI response contained no text output".to_owned(),
            ));
        }
        let created_at = DateTime::from_timestamp(response.created_at, 0).unwrap_or_else(Utc::now);
        Ok(GenerationResponse {
            response_id: Uuid::new_v4(),
            model: request.model,
            text,
            usage: response
                .usage
                .map(|usage| TokenUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                })
                .unwrap_or_default(),
            finish_reason: Some(response.status),
            created_at,
        })
    }

    async fn stream(&self, _request: GenerationRequest) -> Result<GenerationStream, ProviderError> {
        Err(ProviderError::Unsupported(
            "OpenAI streaming is not implemented yet".to_owned(),
        ))
    }

    async fn health(&self) -> ProviderHealth {
        match self.list_models().await {
            Ok(_) => ProviderHealth::Healthy,
            Err(error) => ProviderHealth::Unavailable(error.to_string()),
        }
    }

    fn estimate_cost(&self, _request: &GenerationRequest) -> Option<CostEstimate> {
        None
    }
}

#[derive(Debug, Serialize)]
struct OpenAiResponseRequest {
    model: String,
    input: Vec<OpenAiInputMessage>,
    store: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct OpenAiInputMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelList {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
    created: i64,
    owned_by: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    created_at: i64,
    output: Vec<OpenAiOutputItem>,
    status: String,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiOutputItem {
    #[serde(default)]
    content: Vec<OpenAiOutputContent>,
}

#[derive(Debug, Deserialize)]
struct OpenAiOutputContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorEnvelope {
    error: OpenAiError,
}

#[derive(Debug, Deserialize)]
struct OpenAiError {
    code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::{Method::GET, Method::POST, MockServer};
    use vertex_ai_secrets::{InMemorySecretStore, SecretValue};
    use vertex_ai_types::{GenerationParameters, Message, PrivacyPolicy, VertexContext};

    async fn provider(server: &MockServer) -> OpenAiProvider {
        let secrets = InMemorySecretStore::shared();
        let id = ProviderId::new("openai").unwrap();
        secrets
            .put(
                &SecretId::provider_api_key(&id),
                SecretValue::new("test-key").unwrap(),
            )
            .await
            .unwrap();
        OpenAiProvider::new(
            OpenAiProviderConfig {
                base_url: server.base_url(),
                ..OpenAiProviderConfig::default()
            },
            secrets,
        )
        .unwrap()
    }

    fn cloud_request() -> GenerationRequest {
        let context = VertexContext {
            privacy_policy: PrivacyPolicy {
                cloud_allowed: true,
                ..PrivacyPolicy::default()
            },
            ..VertexContext::default()
        };
        GenerationRequest {
            model: ModelRef::new(
                ProviderId::new("openai").unwrap(),
                ModelId::new("gpt-test").unwrap(),
            ),
            messages: vec![Message::user("hello")],
            context,
            parameters: GenerationParameters::default(),
        }
    }

    #[tokio::test]
    async fn discovers_models_with_bearer_auth() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/models")
                    .header("authorization", "Bearer test-key");
                then.status(200).json_body(json!({
                    "object": "list",
                    "data": [{"id": "gpt-test", "created": 1, "object": "model", "owned_by": "openai"}]
                }));
            })
            .await;
        let provider = provider(&server).await;
        let models = provider.list_models().await.unwrap();
        mock.assert_async().await;
        assert_eq!(models[0].reference.model_id.as_str(), "gpt-test");
    }

    #[tokio::test]
    async fn generates_with_responses_api_and_disables_storage() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/responses")
                    .header("authorization", "Bearer test-key")
                    .json_body_includes(r#"{"model":"gpt-test","store":false}"#);
                then.status(200).json_body(json!({
                    "id": "resp_1",
                    "created_at": 1,
                    "model": "gpt-test",
                    "object": "response",
                    "output": [{"type": "message", "content": [{"type": "output_text", "text": "answer"}]}],
                    "status": "completed",
                    "usage": {"input_tokens": 5, "output_tokens": 2, "total_tokens": 7}
                }));
            })
            .await;
        let provider = provider(&server).await;
        let response = provider.generate(cloud_request()).await.unwrap();
        mock.assert_async().await;
        assert_eq!(response.text, "answer");
        assert_eq!(response.usage.input_tokens, 5);
    }

    #[tokio::test]
    async fn rejects_context_that_forbids_cloud_processing_before_http() {
        let server = MockServer::start_async().await;
        let provider = provider(&server).await;
        let mut request = cloud_request();
        request.context.privacy_policy.cloud_allowed = false;
        let error = provider.generate(request).await.unwrap_err();
        assert!(matches!(error, ProviderError::InvalidRequest(_)));
    }
}
