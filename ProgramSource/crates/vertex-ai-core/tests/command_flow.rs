use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::{fs, sync::Arc};
use uuid::Uuid;
use vertex_ai_context::{ContextBuildRequest, TargetLocation};
use vertex_ai_core::{Command, CommandResponse, CoreConfig, VertexAiCore};
use vertex_ai_environment::PersistentEnvironmentIndex;
use vertex_ai_memory::{
    CreateMemory, InMemoryMemoryRepository, MemoryCategory, MemoryPrivacy, MemoryProposal,
    MemoryQuery, MemoryScope, MemoryWritePermit,
};
use vertex_ai_provider::{MockProvider, ProviderHealth};
use vertex_ai_runtime::{LocalRuntimeManager, RuntimeError};
use vertex_ai_secrets::{InMemorySecretStore, SecretId, SecretStore, SecretValue};
use vertex_ai_types::{
    GenerationParameters, HealthState, InstalledLocalModel, LoadedLocalModel, LocalRuntimeSnapshot,
    Message, ModelId, ModelRef, ProviderId, VertexContext,
};

struct FakeRuntime {
    id: ProviderId,
    snapshot: tokio::sync::Mutex<LocalRuntimeSnapshot>,
}

#[async_trait]
impl LocalRuntimeManager for FakeRuntime {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    async fn inspect(&self) -> Result<LocalRuntimeSnapshot, RuntimeError> {
        Ok(self.snapshot.lock().await.clone())
    }

    async fn unload_model(&self, model_id: &ModelId) -> Result<(), RuntimeError> {
        self.snapshot
            .lock()
            .await
            .loaded_models
            .retain(|model| &model.reference.model_id != model_id);
        Ok(())
    }

    async fn download_model(
        &self,
        _model_id: &ModelId,
        progress: tokio::sync::mpsc::UnboundedSender<vertex_ai_types::ModelDownloadProgress>,
        _cancellation: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), RuntimeError> {
        let _ = progress.send(vertex_ai_types::ModelDownloadProgress {
            status: "success".to_owned(),
            completed_bytes: 1,
            total_bytes: Some(1),
        });
        Ok(())
    }
}

fn ids() -> (ProviderId, ModelRef) {
    let provider = ProviderId::new("mock").unwrap();
    let model = ModelRef::new(provider.clone(), ModelId::new("mock-1").unwrap());
    (provider, model)
}

#[tokio::test]
async fn transport_neutral_command_flow_works() {
    let (provider_id, model) = ids();
    let secrets = InMemorySecretStore::shared();
    let core = VertexAiCore::new(CoreConfig::default(), secrets)
        .with_memory_repository(InMemoryMemoryRepository::shared());
    core.register_provider(Arc::new(MockProvider::new(
        provider_id.clone(),
        [model.model_id.clone()],
    )))
    .await
    .unwrap();

    let health = core
        .execute(Command::GetProviderHealth {
            provider_id: provider_id.clone(),
        })
        .await
        .unwrap();
    assert!(matches!(
        health,
        CommandResponse::ProviderHealth(ProviderHealth::Healthy)
    ));

    let models = core
        .execute(Command::GetModels {
            provider_id: Some(provider_id),
            refresh: true,
        })
        .await
        .unwrap();
    assert!(matches!(models, CommandResponse::Models(items) if items.len() == 1));

    core.execute(Command::SelectModel {
        model: model.clone(),
    })
    .await
    .unwrap();

    let prepared = match core
        .execute(Command::BuildContext {
            request: Box::new(ContextBuildRequest {
                base: VertexContext::default(),
                scope: MemoryScope::project(Uuid::new_v4(), Uuid::new_v4()),
                query: "foundation".to_owned(),
                target_location: TargetLocation::Local,
                allow_sensitive: false,
                max_context_tokens: 2_000,
                reserved_output_tokens: 500,
                memory_candidate_limit: 10,
            }),
        })
        .await
        .unwrap()
    {
        CommandResponse::ContextBuilt(built) => built.prepared,
        other => panic!("unexpected response: {other:?}"),
    };
    let generated = core
        .execute(Command::Generate {
            model: None,
            messages: vec![Message::user("foundation")],
            context: Box::new(prepared),
            parameters: GenerationParameters::default(),
        })
        .await
        .unwrap();
    assert!(
        matches!(generated, CommandResponse::Generated(response) if response.text == "mock:foundation" && response.model == model)
    );
}

#[tokio::test]
async fn ai_environment_aggregates_runtime_facts_and_unloads_models() {
    let provider_id = ProviderId::new("ollama").unwrap();
    let model = ModelRef::new(provider_id.clone(), ModelId::new("qwen3:8b").unwrap());
    let runtime = Arc::new(FakeRuntime {
        id: provider_id.clone(),
        snapshot: tokio::sync::Mutex::new(LocalRuntimeSnapshot {
            provider_id: provider_id.clone(),
            display_name: "Ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            health: HealthState::Ready,
            version: Some("test".to_owned()),
            executable_path: Some("ollama.exe".to_owned()),
            model_storage_path: Some("D:/Ollama/Models".to_owned()),
            storage_total_bytes: Some(1_000_000_000_000),
            storage_available_bytes: Some(900_000_000_000),
            installed_models: vec![InstalledLocalModel {
                reference: model.clone(),
                display_name: "qwen3:8b".to_owned(),
                size_bytes: 5_200_000_000,
                digest: Some("sha256:test".to_owned()),
                format: Some("gguf".to_owned()),
                family: Some("qwen3".to_owned()),
                parameter_size: Some("8B".to_owned()),
                quantization_level: Some("Q4_K_M".to_owned()),
                context_length: Some(40_960),
                modified_at: Some(Utc::now()),
            }],
            loaded_models: vec![LoadedLocalModel {
                reference: model.clone(),
                size_bytes: 6_000_000_000,
                size_vram_bytes: 5_000_000_000,
                context_length: Some(4_096),
                expires_at: Some(Utc::now()),
            }],
            checked_at: Utc::now(),
        }),
    });
    let core = VertexAiCore::new(CoreConfig::default(), InMemorySecretStore::shared());
    core.register_runtime(runtime).await.unwrap();

    let summary = core.execute(Command::GetAiEnvironment).await.unwrap();
    assert!(matches!(
        summary,
        CommandResponse::AiEnvironment(summary)
            if summary.local_inference_ready
                && summary.installed_model_count == 1
                && summary.loaded_model_count == 1
                && summary.total_vram_bytes == 5_000_000_000
    ));

    let unloaded = core
        .execute(Command::UnloadLocalModel {
            provider_id,
            model_id: model.model_id,
        })
        .await
        .unwrap();
    assert!(matches!(
        unloaded,
        CommandResponse::LocalModelUnloaded(snapshot) if snapshot.loaded_models.is_empty()
    ));
}

#[tokio::test]
async fn environment_scan_crosses_the_transport_neutral_core_boundary() {
    let root = std::env::temp_dir().join(format!("vertex-core-environment-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("node.exe"), b"test").unwrap();
    let search_path = std::env::join_paths([&root])
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let core = VertexAiCore::new(CoreConfig::default(), InMemorySecretStore::shared());

    let response = core
        .execute(Command::ScanEnvironment {
            path_override: Some(search_path),
        })
        .await
        .unwrap();

    match response {
        CommandResponse::EnvironmentScanned(result) => {
            assert_eq!(result.snapshot.assets.len(), 1);
            assert_eq!(result.snapshot.assets[0].name, "Node.js");
            assert!(
                result.snapshot.assets[0]
                    .capabilities
                    .contains(&"runtime.nodejs".to_owned())
            );
            assert!(result.delta.is_none());
        }
        other => panic!("unexpected response: {other:?}"),
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn environment_index_survives_core_commands_and_reports_deltas() {
    let root = std::env::temp_dir().join(format!("vertex-core-index-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("node.exe"), b"test").unwrap();
    let search_path = std::env::join_paths([&root])
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let index =
        PersistentEnvironmentIndex::open(root.join("cache").join("environment.json")).unwrap();
    let core = VertexAiCore::new(CoreConfig::default(), InMemorySecretStore::shared())
        .with_environment_index(index);

    let scanned = core
        .execute(Command::ScanEnvironment {
            path_override: Some(search_path),
        })
        .await
        .unwrap();
    assert!(matches!(
        scanned,
        CommandResponse::EnvironmentScanned(result)
            if result.delta.as_ref().is_some_and(|delta| delta.added.len() == 1)
    ));

    let cached = core.execute(Command::GetEnvironmentSnapshot).await.unwrap();
    assert!(matches!(
        cached,
        CommandResponse::EnvironmentSnapshot(Some(snapshot)) if snapshot.assets.len() == 1
    ));
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn provider_secret_crosses_the_command_boundary_without_plaintext_debug_output() {
    let (provider_id, _) = ids();
    let secrets = InMemorySecretStore::shared();
    let store_view = secrets.clone();
    let core = VertexAiCore::new(CoreConfig::default(), secrets);
    let secret = SecretValue::new("vertex-secret-value").unwrap();
    let command = Command::StoreProviderSecret {
        provider_id: provider_id.clone(),
        value: secret,
    };
    assert!(!format!("{command:?}").contains("vertex-secret-value"));

    core.execute(command).await.unwrap();
    let stored = store_view
        .get(&SecretId::provider_api_key(&provider_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.expose(), "vertex-secret-value");
}

#[tokio::test]
async fn memory_remains_available_after_model_switch() {
    let provider_id = ProviderId::new("mock").unwrap();
    let model_a = ModelRef::new(provider_id.clone(), ModelId::new("model-a").unwrap());
    let model_b = ModelRef::new(provider_id.clone(), ModelId::new("model-b").unwrap());
    let core = VertexAiCore::new(CoreConfig::default(), InMemorySecretStore::shared())
        .with_memory_repository(InMemoryMemoryRepository::shared());
    core.register_provider(Arc::new(MockProvider::new(
        provider_id.clone(),
        [model_a.model_id.clone(), model_b.model_id.clone()],
    )))
    .await
    .unwrap();
    core.execute(Command::GetModels {
        provider_id: Some(provider_id),
        refresh: true,
    })
    .await
    .unwrap();
    core.execute(Command::SelectModel { model: model_a })
        .await
        .unwrap();

    let actor_id = Uuid::new_v4();
    let scope = MemoryScope::project(Uuid::new_v4(), Uuid::new_v4());
    core.execute(Command::ProposeMemory {
        proposal: Box::new(MemoryProposal {
            candidate: CreateMemory {
                category: MemoryCategory::Knowledge,
                scope: scope.clone(),
                owner_id: None,
                content: "Project Alpha uses PostgreSQL as its Memory Engine.".to_owned(),
                structured_content: json!({}),
                priority: 1.0,
                confidence: 1.0,
                source: "integration-test".to_owned(),
                expires_at: None,
                privacy: MemoryPrivacy::default(),
                metadata: json!({}),
            },
        }),
        permit: Box::new(MemoryWritePermit {
            actor_id: Some(actor_id),
            scope: scope.clone(),
            allow_sensitive: false,
        }),
    })
    .await
    .unwrap();

    core.execute(Command::SelectModel { model: model_b })
        .await
        .unwrap();
    let recalled = core
        .execute(Command::RecallMemory {
            query: Box::new(MemoryQuery {
                scope,
                text: Some("PostgreSQL".to_owned()),
                category: None,
                include_expired: false,
                limit: 10,
            }),
        })
        .await
        .unwrap();
    assert!(matches!(
        recalled,
        CommandResponse::Memories(records)
            if records.len() == 1 && records[0].content.contains("PostgreSQL")
    ));
}

#[tokio::test]
async fn core_builds_cloud_context_without_local_only_memory() {
    let repository = InMemoryMemoryRepository::shared();
    let core = VertexAiCore::new(CoreConfig::default(), InMemorySecretStore::shared())
        .with_memory_repository(repository);
    let scope = MemoryScope::project(Uuid::new_v4(), Uuid::new_v4());
    for (content, privacy) in [
        (
            "PostgreSQL public fact",
            MemoryPrivacy {
                cloud_allowed: true,
                ..MemoryPrivacy::default()
            },
        ),
        (
            "PostgreSQL private fact",
            MemoryPrivacy {
                local_only: true,
                ..MemoryPrivacy::default()
            },
        ),
    ] {
        core.execute(Command::ProposeMemory {
            proposal: Box::new(MemoryProposal {
                candidate: CreateMemory {
                    category: MemoryCategory::Knowledge,
                    scope: scope.clone(),
                    owner_id: None,
                    content: content.to_owned(),
                    structured_content: json!({}),
                    priority: 1.0,
                    confidence: 1.0,
                    source: "integration-test".to_owned(),
                    expires_at: None,
                    privacy,
                    metadata: json!({}),
                },
            }),
            permit: Box::new(MemoryWritePermit {
                actor_id: None,
                scope: scope.clone(),
                allow_sensitive: false,
            }),
        })
        .await
        .unwrap();
    }

    let response = core
        .execute(Command::BuildContext {
            request: Box::new(ContextBuildRequest {
                base: VertexContext {
                    privacy_policy: vertex_ai_types::PrivacyPolicy {
                        cloud_allowed: true,
                        ..vertex_ai_types::PrivacyPolicy::default()
                    },
                    ..VertexContext::default()
                },
                scope,
                query: "PostgreSQL".to_owned(),
                target_location: TargetLocation::Cloud,
                allow_sensitive: false,
                max_context_tokens: 2_000,
                reserved_output_tokens: 500,
                memory_candidate_limit: 10,
            }),
        })
        .await
        .unwrap();
    assert!(matches!(
        response,
        CommandResponse::ContextBuilt(built)
            if built.report.included_count == 1
                && built.report.excluded_privacy_count == 1
                && built.prepared.context().memories[0]["content"] == "PostgreSQL public fact"
    ));
}

#[tokio::test]
async fn prepared_context_cannot_be_used_with_a_different_model_location() {
    let provider_id = ProviderId::new("mock").unwrap();
    let local_model = ModelRef::new(provider_id.clone(), ModelId::new("local").unwrap());
    let core = VertexAiCore::new(CoreConfig::default(), InMemorySecretStore::shared())
        .with_memory_repository(InMemoryMemoryRepository::shared());
    core.register_provider(Arc::new(MockProvider::new(
        provider_id.clone(),
        [local_model.model_id.clone()],
    )))
    .await
    .unwrap();
    core.execute(Command::GetModels {
        provider_id: Some(provider_id),
        refresh: true,
    })
    .await
    .unwrap();
    let scope = MemoryScope::project(Uuid::new_v4(), Uuid::new_v4());
    let prepared = match core
        .execute(Command::BuildContext {
            request: Box::new(ContextBuildRequest {
                base: VertexContext::default(),
                scope,
                query: "test".to_owned(),
                target_location: TargetLocation::Cloud,
                allow_sensitive: false,
                max_context_tokens: 1_000,
                reserved_output_tokens: 200,
                memory_candidate_limit: 10,
            }),
        })
        .await
        .unwrap()
    {
        CommandResponse::ContextBuilt(built) => built.prepared,
        other => panic!("unexpected response: {other:?}"),
    };
    let result = core
        .execute(Command::Generate {
            model: Some(local_model),
            messages: vec![Message::user("test")],
            context: Box::new(prepared),
            parameters: GenerationParameters::default(),
        })
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn reason_command_builds_context_and_generates_in_one_core_operation() {
    let (provider_id, model) = ids();
    let core = VertexAiCore::new(CoreConfig::default(), InMemorySecretStore::shared())
        .with_memory_repository(InMemoryMemoryRepository::shared());
    core.register_provider(Arc::new(MockProvider::new(
        provider_id.clone(),
        [model.model_id.clone()],
    )))
    .await
    .unwrap();
    core.execute(Command::GetModels {
        provider_id: Some(provider_id),
        refresh: true,
    })
    .await
    .unwrap();

    let response = core
        .execute(Command::Reason {
            model: Some(model),
            messages: vec![Message::user("reason safely")],
            context_request: Box::new(ContextBuildRequest {
                base: VertexContext::default(),
                scope: MemoryScope::project(Uuid::new_v4(), Uuid::new_v4()),
                query: "reason safely".to_owned(),
                target_location: TargetLocation::Local,
                allow_sensitive: false,
                max_context_tokens: 2_000,
                reserved_output_tokens: 500,
                memory_candidate_limit: 10,
            }),
            parameters: GenerationParameters::default(),
        })
        .await
        .unwrap();
    assert!(matches!(
        response,
        CommandResponse::Reasoned { response, context_report }
            if response.text == "mock:reason safely"
                && context_report.included_count == 0
    ));
}
