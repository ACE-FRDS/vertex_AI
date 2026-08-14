use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{Manager, State};
use tokio::time::MissedTickBehavior;
use tracing::{error, warn};
use uuid::Uuid;
use vertex_ai_audit::PersistentAuditLog;
use vertex_ai_context::PreparedContext;
use vertex_ai_core::{Command, CommandResponse, CoreConfig, VertexAiCore, init_logging};
use vertex_ai_developer::{
    AgentModel, ArdAssignment, ArdCoordinator, ArdSession, ArdSessionId, ArdTeam, ArdTeamId,
    ArdWorkflow, ArdWorkflowId, CompleteArdStage, CreateArdTeam, DeveloperCoordinator,
    DeveloperError, DeveloperStore, DeveloperTask, DeveloperTaskId, JsonDeveloperEngine,
    JsonDeveloperStore, PostgresDeveloperStore, StartDeveloperTask, Workspace, WorkspaceRegistry,
};
use vertex_ai_environment::{
    EnvironmentSnapshot, IndexedEnvironmentSnapshot, PersistentEnvironmentIndex,
};
use vertex_ai_memory::{
    CreateMemory, MemoryCategory, MemoryPrivacy, MemoryProposal, MemoryQuery, MemoryRecord,
    MemoryScope, MemoryWritePermit, PostgresMemoryRepository,
};
use vertex_ai_provider::ProviderHealth;
use vertex_ai_provider_ollama::{OllamaProvider, OllamaProviderConfig};
use vertex_ai_runtime::{
    ManagedRuntimeSnapshot, ManagedServiceRuntime, ModelDownloadCoordinator, RuntimeDiagnosis,
};
use vertex_ai_runtime_postgres::{
    MANAGED_POSTGRES_VERSION, ManagedPostgresPaths, ManagedPostgresRuntime,
};
use vertex_ai_secrets::{SecretStore, WindowsCredentialStore};
use vertex_ai_types::{
    AiEnvironmentSummary, AuditEvent, AuditEventId, AuditOutcome, ErrorEnvelope, ErrorId,
    GenerationParameters, GenerationResponse, LocalRuntimeSnapshot, Message, MessageRole,
    ModelDescriptor, ModelDownloadJob, ModelId, ModelRef, ProviderId, RuntimeJobId, Severity,
    VertexContext,
};

const BACKGROUND_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const MAX_PROMPT_CHARS: usize = 100_000;
const MAX_OUTPUT_TOKENS: u32 = 32_768;

struct AppState {
    core: Arc<VertexAiCore>,
    audit: Mutex<PersistentAuditLog>,
    memory_runtime: Arc<ManagedPostgresRuntime>,
    developer: Arc<DeveloperCoordinator>,
    ard: Arc<ArdCoordinator>,
}

struct CoreAgentModel {
    core: Arc<VertexAiCore>,
    model: ModelRef,
    name: String,
}

#[async_trait]
impl AgentModel for CoreAgentModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    async fn complete(&self, system: &str, prompt: &str) -> Result<String, DeveloperError> {
        let response = self
            .core
            .execute(Command::Generate {
                model: Some(self.model.clone()),
                messages: vec![
                    Message {
                        role: MessageRole::System,
                        content: system.to_owned(),
                    },
                    Message::user(prompt),
                ],
                context: Box::new(PreparedContext::local(VertexContext::default())),
                parameters: GenerationParameters {
                    temperature: Some(0.1),
                    max_output_tokens: Some(2_048),
                    ..GenerationParameters::default()
                },
            })
            .await
            .map_err(|error| DeveloperError::Model(error.to_string()))?;
        match response {
            CommandResponse::Generated(result) => Ok(result.text),
            other => Err(DeveloperError::Model(format!(
                "unexpected developer model response: {other:?}"
            ))),
        }
    }
}

#[derive(Debug, Serialize)]
struct LocalProviderStatus {
    health: ProviderHealth,
    models: Vec<ModelDescriptor>,
}

#[derive(Debug, Deserialize)]
struct LocalGenerationInput {
    model_id: String,
    prompt: String,
    temperature: Option<f32>,
    max_output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct StoreSystemMemoryInput {
    content: String,
    category: Option<String>,
    priority: Option<f32>,
    confidence: Option<f32>,
}

#[tauri::command]
#[allow(clippy::result_large_err)]
async fn register_developer_workspace(
    state: State<'_, AppState>,
    name: String,
    root: String,
) -> Result<Workspace, ErrorEnvelope> {
    state
        .developer
        .register_workspace(name, root)
        .await
        .map_err(|error| developer_error("register_developer_workspace", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn list_developer_workspaces(state: State<'_, AppState>) -> Result<Vec<Workspace>, ErrorEnvelope> {
    state
        .developer
        .list_workspaces()
        .map_err(|error| developer_error("list_developer_workspaces", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
async fn start_developer_task(
    state: State<'_, AppState>,
    input: StartDeveloperTask,
) -> Result<DeveloperTask, ErrorEnvelope> {
    if input.provider_id != "ollama" {
        return Err(error_envelope(
            "start_developer_task",
            "developer_provider_not_available",
            "Phase 1 currently supports the Ollama provider adapter".to_owned(),
            false,
        ));
    }
    let model_id = ModelId::new(input.model_id.clone()).map_err(|_| {
        error_envelope(
            "start_developer_task",
            "invalid_model_id",
            "model id cannot be blank".to_owned(),
            false,
        )
    })?;
    let model_ref = ModelRef::new(ollama_provider_id(), model_id);
    let available = state
        .core
        .execute(Command::GetModels {
            provider_id: Some(ollama_provider_id()),
            refresh: true,
        })
        .await
        .map_err(|error| {
            error_envelope(
                "start_developer_task",
                "developer_model_discovery_failed",
                error.to_string(),
                true,
            )
        })?;
    let CommandResponse::Models(models) = available else {
        return Err(error_envelope(
            "start_developer_task",
            "unexpected_core_response",
            format!("unexpected response: {available:?}"),
            false,
        ));
    };
    if !models.iter().any(|model| model.reference == model_ref) {
        return Err(error_envelope(
            "start_developer_task",
            "developer_model_not_available",
            format!("Ollama model is not available: {}", input.model_id),
            false,
        ));
    }
    let model: Arc<dyn AgentModel> = Arc::new(CoreAgentModel {
        core: state.core.clone(),
        model: model_ref,
        name: format!("ollama/{}", input.model_id),
    });
    state
        .developer
        .start_task(input, Arc::new(JsonDeveloperEngine::new(model)))
        .await
        .map_err(|error| developer_error("start_developer_task", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
async fn get_developer_task(
    state: State<'_, AppState>,
    task_id: DeveloperTaskId,
) -> Result<DeveloperTask, ErrorEnvelope> {
    state
        .developer
        .get_task(task_id)
        .await
        .map_err(|error| developer_error("get_developer_task", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
async fn list_developer_tasks(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<DeveloperTask>, ErrorEnvelope> {
    state
        .developer
        .list_tasks(limit.unwrap_or(50).clamp(1, 500))
        .await
        .map_err(|error| developer_error("list_developer_tasks", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
async fn cancel_developer_task(
    state: State<'_, AppState>,
    task_id: DeveloperTaskId,
) -> Result<bool, ErrorEnvelope> {
    state
        .developer
        .cancel_task(task_id)
        .await
        .map_err(|error| developer_error("cancel_developer_task", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
async fn rollback_developer_task(
    state: State<'_, AppState>,
    task_id: DeveloperTaskId,
) -> Result<DeveloperTask, ErrorEnvelope> {
    state
        .developer
        .rollback_task(task_id)
        .await
        .map_err(|error| developer_error("rollback_developer_task", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn create_ard_team(
    state: State<'_, AppState>,
    input: CreateArdTeam,
) -> Result<ArdTeam, ErrorEnvelope> {
    state
        .ard
        .create_team(input)
        .map_err(|error| developer_error("create_ard_team", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn list_ard_teams(state: State<'_, AppState>) -> Result<Vec<ArdTeam>, ErrorEnvelope> {
    state
        .ard
        .list_teams()
        .map_err(|error| developer_error("list_ard_teams", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn create_ard_workflow(
    state: State<'_, AppState>,
    team_id: ArdTeamId,
    name: String,
) -> Result<ArdWorkflow, ErrorEnvelope> {
    state
        .ard
        .create_relay_workflow(team_id, name)
        .map_err(|error| developer_error("create_ard_workflow", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn list_ard_workflows(
    state: State<'_, AppState>,
    team_id: ArdTeamId,
) -> Result<Vec<ArdWorkflow>, ErrorEnvelope> {
    state
        .ard
        .list_workflows(team_id)
        .map_err(|error| developer_error("list_ard_workflows", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn start_ard_session(
    state: State<'_, AppState>,
    workflow_id: ArdWorkflowId,
    goal: String,
) -> Result<ArdSession, ErrorEnvelope> {
    state
        .ard
        .start_session(workflow_id, goal)
        .map_err(|error| developer_error("start_ard_session", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn get_ard_session(
    state: State<'_, AppState>,
    session_id: ArdSessionId,
) -> Result<ArdSession, ErrorEnvelope> {
    state
        .ard
        .get_session(session_id)
        .map_err(|error| developer_error("get_ard_session", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn list_ard_sessions(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<ArdSession>, ErrorEnvelope> {
    state
        .ard
        .list_sessions(limit.unwrap_or(50))
        .map_err(|error| developer_error("list_ard_sessions", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn get_ard_assignment(
    state: State<'_, AppState>,
    session_id: ArdSessionId,
) -> Result<ArdAssignment, ErrorEnvelope> {
    state
        .ard
        .current_assignment(session_id)
        .map_err(|error| developer_error("get_ard_assignment", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn complete_ard_stage(
    state: State<'_, AppState>,
    session_id: ArdSessionId,
    input: CompleteArdStage,
) -> Result<ArdSession, ErrorEnvelope> {
    state
        .ard
        .complete_stage(session_id, input)
        .map_err(|error| developer_error("complete_ard_stage", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn pause_ard_session(
    state: State<'_, AppState>,
    session_id: ArdSessionId,
) -> Result<ArdSession, ErrorEnvelope> {
    state
        .ard
        .pause(session_id)
        .map_err(|error| developer_error("pause_ard_session", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn resume_ard_session(
    state: State<'_, AppState>,
    session_id: ArdSessionId,
) -> Result<ArdSession, ErrorEnvelope> {
    state
        .ard
        .resume(session_id)
        .map_err(|error| developer_error("resume_ard_session", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn cancel_ard_session(
    state: State<'_, AppState>,
    session_id: ArdSessionId,
) -> Result<ArdSession, ErrorEnvelope> {
    state
        .ard
        .cancel(session_id)
        .map_err(|error| developer_error("cancel_ard_session", error))
}

#[tauri::command]
#[allow(clippy::result_large_err)]
fn intervene_ard_session(
    state: State<'_, AppState>,
    session_id: ArdSessionId,
    instruction: String,
) -> Result<ArdSession, ErrorEnvelope> {
    state
        .ard
        .intervene(session_id, instruction)
        .map_err(|error| developer_error("intervene_ard_session", error))
}

#[tauri::command]
async fn get_memory_core_status(
    state: State<'_, AppState>,
) -> Result<ManagedRuntimeSnapshot, ErrorEnvelope> {
    state
        .memory_runtime
        .inspect_managed()
        .await
        .map_err(|error| memory_error("get_memory_core_status", "memory_status_failed", error))
}

#[tauri::command]
async fn start_memory_core(
    state: State<'_, AppState>,
) -> Result<ManagedRuntimeSnapshot, ErrorEnvelope> {
    let result = start_and_connect_memory_core(state.inner()).await;
    record_result_audit(
        state.inner(),
        "start_memory_core",
        &["vertex-memory-core".to_owned()],
        &result,
        BTreeMap::new(),
    );
    result
}

#[tauri::command]
async fn stop_memory_core(
    state: State<'_, AppState>,
) -> Result<ManagedRuntimeSnapshot, ErrorEnvelope> {
    let result = state
        .memory_runtime
        .stop()
        .await
        .map_err(|error| memory_error("stop_memory_core", "memory_stop_failed", error));
    record_result_audit(
        state.inner(),
        "stop_memory_core",
        &["vertex-memory-core".to_owned()],
        &result,
        BTreeMap::new(),
    );
    result
}

#[tauri::command]
async fn restart_memory_core(
    state: State<'_, AppState>,
) -> Result<ManagedRuntimeSnapshot, ErrorEnvelope> {
    let result =
        async {
            state.memory_runtime.restart().await.map_err(|error| {
                memory_error("restart_memory_core", "memory_restart_failed", error)
            })?;
            connect_memory_repository(state.inner()).await?;
            state
                .memory_runtime
                .inspect_managed()
                .await
                .map_err(|error| memory_error("restart_memory_core", "memory_status_failed", error))
        }
        .await;
    record_result_audit(
        state.inner(),
        "restart_memory_core",
        &["vertex-memory-core".to_owned()],
        &result,
        BTreeMap::new(),
    );
    result
}

#[tauri::command]
async fn diagnose_memory_core(
    state: State<'_, AppState>,
) -> Result<Vec<RuntimeDiagnosis>, ErrorEnvelope> {
    state
        .memory_runtime
        .diagnose()
        .await
        .map_err(|error| memory_error("diagnose_memory_core", "memory_diagnosis_failed", error))
}

#[tauri::command]
async fn search_system_memories(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<MemoryRecord>, ErrorEnvelope> {
    match state
        .core
        .execute(Command::RecallMemory {
            query: Box::new(MemoryQuery {
                scope: MemoryScope::system(),
                text: query.and_then(|value| {
                    let value = value.trim().to_owned();
                    (!value.is_empty()).then_some(value)
                }),
                category: None,
                include_expired: false,
                limit: limit.unwrap_or(50).clamp(1, 100),
            }),
        })
        .await
    {
        Ok(CommandResponse::Memories(memories)) => Ok(memories),
        Ok(other) => Err(error_envelope(
            "search_system_memories",
            "unexpected_core_response",
            format!("unexpected response: {other:?}"),
            false,
        )),
        Err(error) => Err(error_envelope(
            "search_system_memories",
            "memory_search_failed",
            error.to_string(),
            true,
        )),
    }
}

#[tauri::command]
async fn store_system_memory(
    state: State<'_, AppState>,
    input: StoreSystemMemoryInput,
) -> Result<MemoryRecord, ErrorEnvelope> {
    let content = input.content.trim().to_owned();
    if content.is_empty() || content.chars().count() > MAX_PROMPT_CHARS {
        return Err(error_envelope(
            "store_system_memory",
            "invalid_memory_content",
            "memory content must not be blank or oversized".to_owned(),
            false,
        ));
    }
    let category = parse_memory_category(input.category.as_deref())?;
    let scope = MemoryScope::system();
    let result = match state
        .core
        .execute(Command::ProposeMemory {
            proposal: Box::new(MemoryProposal {
                candidate: CreateMemory {
                    category,
                    scope: scope.clone(),
                    owner_id: None,
                    content,
                    structured_content: json!({}),
                    priority: input.priority.unwrap_or(0.5),
                    confidence: input.confidence.unwrap_or(1.0),
                    source: "desktop-user".to_owned(),
                    expires_at: None,
                    privacy: MemoryPrivacy {
                        local_only: true,
                        cloud_allowed: false,
                        sensitive: false,
                        share_scope: None,
                    },
                    metadata: json!({"managed_by": "vertex-memory-core"}),
                },
            }),
            permit: Box::new(MemoryWritePermit {
                actor_id: None,
                scope,
                allow_sensitive: false,
            }),
        })
        .await
    {
        Ok(CommandResponse::MemoryStored(memory)) => Ok(memory),
        Ok(other) => Err(error_envelope(
            "store_system_memory",
            "unexpected_core_response",
            format!("unexpected response: {other:?}"),
            false,
        )),
        Err(error) => Err(error_envelope(
            "store_system_memory",
            "memory_store_failed",
            error.to_string(),
            true,
        )),
    };
    record_result_audit(
        state.inner(),
        "store_system_memory",
        &["vertex-memory-core".to_owned()],
        &result,
        BTreeMap::new(),
    );
    result
}

#[tauri::command]
async fn scan_environment(
    state: State<'_, AppState>,
) -> Result<IndexedEnvironmentSnapshot, ErrorEnvelope> {
    let result = match state
        .core
        .execute(Command::ScanEnvironment {
            path_override: None,
        })
        .await
    {
        Ok(CommandResponse::EnvironmentScanned(snapshot)) => Ok(snapshot),
        Ok(other) => Err(error_envelope(
            "scan_environment",
            "unexpected_core_response",
            format!("unexpected response: {other:?}"),
            false,
        )),
        Err(error) => Err(error_envelope(
            "scan_environment",
            "environment_scan_failed",
            error.to_string(),
            true,
        )),
    };
    record_result_audit(
        state.inner(),
        "scan_environment",
        &[],
        &result,
        BTreeMap::new(),
    );
    result
}

#[tauri::command]
async fn get_environment_snapshot(
    state: State<'_, AppState>,
) -> Result<Option<EnvironmentSnapshot>, ErrorEnvelope> {
    match state.core.execute(Command::GetEnvironmentSnapshot).await {
        Ok(CommandResponse::EnvironmentSnapshot(snapshot)) => Ok(snapshot),
        Ok(other) => Err(error_envelope(
            "get_environment_snapshot",
            "unexpected_core_response",
            format!("unexpected response: {other:?}"),
            false,
        )),
        Err(error) => Err(error_envelope(
            "get_environment_snapshot",
            "environment_cache_read_failed",
            error.to_string(),
            true,
        )),
    }
}

#[tauri::command]
async fn get_local_provider_status(
    state: State<'_, AppState>,
) -> Result<LocalProviderStatus, ErrorEnvelope> {
    let provider_id = ollama_provider_id();
    let health = match state
        .core
        .execute(Command::GetProviderHealth {
            provider_id: provider_id.clone(),
        })
        .await
    {
        Ok(CommandResponse::ProviderHealth(health)) => health,
        Ok(other) => {
            return Err(error_envelope(
                "get_local_provider_status",
                "unexpected_core_response",
                format!("unexpected response: {other:?}"),
                false,
            ));
        }
        Err(error) => {
            return Err(error_envelope(
                "get_local_provider_status",
                "provider_health_failed",
                error.to_string(),
                true,
            ));
        }
    };
    if !matches!(health, ProviderHealth::Healthy) {
        return Ok(LocalProviderStatus {
            health,
            models: Vec::new(),
        });
    }
    let models = match state
        .core
        .execute(Command::GetModels {
            provider_id: Some(provider_id),
            refresh: true,
        })
        .await
    {
        Ok(CommandResponse::Models(models)) => models,
        Ok(other) => {
            return Err(error_envelope(
                "get_local_provider_status",
                "unexpected_core_response",
                format!("unexpected response: {other:?}"),
                false,
            ));
        }
        Err(error) => {
            return Err(error_envelope(
                "get_local_provider_status",
                "model_discovery_failed",
                error.to_string(),
                true,
            ));
        }
    };
    Ok(LocalProviderStatus { health, models })
}

#[tauri::command]
async fn get_ai_environment(
    state: State<'_, AppState>,
) -> Result<AiEnvironmentSummary, ErrorEnvelope> {
    match state.core.execute(Command::GetAiEnvironment).await {
        Ok(CommandResponse::AiEnvironment(summary)) => Ok(summary),
        Ok(other) => Err(error_envelope(
            "get_ai_environment",
            "unexpected_core_response",
            format!("unexpected response: {other:?}"),
            false,
        )),
        Err(error) => Err(error_envelope(
            "get_ai_environment",
            "ai_environment_inspection_failed",
            error.to_string(),
            true,
        )),
    }
}

#[tauri::command]
async fn unload_local_model(
    state: State<'_, AppState>,
    provider_id: String,
    model_id: String,
) -> Result<LocalRuntimeSnapshot, ErrorEnvelope> {
    let provider_id = ProviderId::new(provider_id).map_err(|_| {
        error_envelope(
            "unload_local_model",
            "invalid_provider_id",
            "provider id cannot be blank".to_owned(),
            false,
        )
    })?;
    let model_id = ModelId::new(model_id).map_err(|_| {
        error_envelope(
            "unload_local_model",
            "invalid_model_id",
            "model id cannot be blank".to_owned(),
            false,
        )
    })?;
    let target = format!("{provider_id}/{model_id}");
    let result = match state
        .core
        .execute(Command::UnloadLocalModel {
            provider_id,
            model_id,
        })
        .await
    {
        Ok(CommandResponse::LocalModelUnloaded(snapshot)) => Ok(snapshot),
        Ok(other) => Err(error_envelope(
            "unload_local_model",
            "unexpected_core_response",
            format!("unexpected response: {other:?}"),
            false,
        )),
        Err(error) => Err(error_envelope(
            "unload_local_model",
            "model_unload_failed",
            error.to_string(),
            true,
        )),
    };
    record_result_audit(
        state.inner(),
        "unload_local_model",
        &[target],
        &result,
        BTreeMap::new(),
    );
    result
}

#[tauri::command]
async fn start_model_download(
    state: State<'_, AppState>,
    provider_id: String,
    model_id: String,
) -> Result<ModelDownloadJob, ErrorEnvelope> {
    let provider_id = ProviderId::new(provider_id).map_err(|_| {
        error_envelope(
            "start_model_download",
            "invalid_provider_id",
            "provider id cannot be blank".to_owned(),
            false,
        )
    })?;
    let model_id = ModelId::new(model_id).map_err(|_| {
        error_envelope(
            "start_model_download",
            "invalid_model_id",
            "model id cannot be blank".to_owned(),
            false,
        )
    })?;
    let target = format!("{provider_id}/{model_id}");
    let result = match state
        .core
        .execute(Command::StartModelDownload {
            provider_id,
            model_id,
        })
        .await
    {
        Ok(CommandResponse::ModelDownloadStarted(job)) => Ok(job),
        Ok(other) => Err(error_envelope(
            "start_model_download",
            "unexpected_core_response",
            format!("unexpected response: {other:?}"),
            false,
        )),
        Err(error) => Err(error_envelope(
            "start_model_download",
            "model_download_start_failed",
            error.to_string(),
            true,
        )),
    };
    record_result_audit(
        state.inner(),
        "start_model_download",
        &[target],
        &result,
        BTreeMap::new(),
    );
    result
}

#[tauri::command]
async fn list_model_downloads(
    state: State<'_, AppState>,
) -> Result<Vec<ModelDownloadJob>, ErrorEnvelope> {
    match state.core.execute(Command::ListModelDownloads).await {
        Ok(CommandResponse::ModelDownloads(jobs)) => Ok(jobs),
        Ok(other) => Err(error_envelope(
            "list_model_downloads",
            "unexpected_core_response",
            format!("unexpected response: {other:?}"),
            false,
        )),
        Err(error) => Err(error_envelope(
            "list_model_downloads",
            "model_download_list_failed",
            error.to_string(),
            true,
        )),
    }
}

#[tauri::command]
async fn cancel_model_download(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<ModelDownloadJob, ErrorEnvelope> {
    let job_id = RuntimeJobId::new(job_id).map_err(|_| {
        error_envelope(
            "cancel_model_download",
            "invalid_job_id",
            "job id cannot be blank".to_owned(),
            false,
        )
    })?;
    let target = job_id.to_string();
    let result = match state
        .core
        .execute(Command::CancelModelDownload { job_id })
        .await
    {
        Ok(CommandResponse::ModelDownloadCancelled(job)) => Ok(job),
        Ok(other) => Err(error_envelope(
            "cancel_model_download",
            "unexpected_core_response",
            format!("unexpected response: {other:?}"),
            false,
        )),
        Err(error) => Err(error_envelope(
            "cancel_model_download",
            "model_download_cancel_failed",
            error.to_string(),
            true,
        )),
    };
    record_result_audit(
        state.inner(),
        "cancel_model_download",
        &[target],
        &result,
        BTreeMap::new(),
    );
    result
}

#[tauri::command]
async fn generate_local(
    state: State<'_, AppState>,
    input: LocalGenerationInput,
) -> Result<GenerationResponse, ErrorEnvelope> {
    let requested_model = input.model_id.trim().to_owned();
    let result = generate_local_inner(state.core.as_ref(), input).await;
    let mut details = BTreeMap::new();
    if !requested_model.is_empty() {
        details.insert(
            "model_id".to_owned(),
            Value::String(requested_model.clone()),
        );
    }
    if let Ok(response) = &result {
        details.insert("response_id".to_owned(), json!(response.response_id));
        details.insert(
            "input_tokens".to_owned(),
            json!(response.usage.input_tokens),
        );
        details.insert(
            "output_tokens".to_owned(),
            json!(response.usage.output_tokens),
        );
    }
    let targets = if requested_model.is_empty() {
        Vec::new()
    } else {
        vec![format!("ollama/{requested_model}")]
    };
    record_result_audit(state.inner(), "generate_local", &targets, &result, details);
    result
}

async fn generate_local_inner(
    core: &VertexAiCore,
    input: LocalGenerationInput,
) -> Result<GenerationResponse, ErrorEnvelope> {
    let input = validate_generation_input(input)?;
    let provider_id = ollama_provider_id();
    let model_id = ModelId::new(input.model_id.clone()).map_err(|_| {
        error_envelope(
            "generate_local",
            "invalid_model_id",
            "model id cannot be blank".to_owned(),
            false,
        )
    })?;
    let model = ModelRef::new(provider_id.clone(), model_id);

    let discovered = core
        .execute(Command::GetModels {
            provider_id: Some(provider_id),
            refresh: true,
        })
        .await
        .map_err(|error| {
            error_envelope(
                "generate_local",
                "model_discovery_failed",
                error.to_string(),
                true,
            )
        })?;
    let CommandResponse::Models(models) = discovered else {
        return Err(error_envelope(
            "generate_local",
            "unexpected_core_response",
            format!("unexpected response: {discovered:?}"),
            false,
        ));
    };
    if !models
        .iter()
        .any(|descriptor| descriptor.reference == model)
    {
        return Err(error_envelope(
            "generate_local",
            "model_not_available",
            format!("Ollama model is not available: {}", input.model_id),
            false,
        ));
    }

    let context = PreparedContext::local(VertexContext::default());
    match core
        .execute(Command::Generate {
            model: Some(model.clone()),
            messages: vec![Message::user(input.prompt)],
            context: Box::new(context),
            parameters: GenerationParameters {
                temperature: input.temperature,
                max_output_tokens: input.max_output_tokens,
                ..GenerationParameters::default()
            },
        })
        .await
    {
        Ok(CommandResponse::Generated(response)) => Ok(response),
        Ok(other) => Err(error_envelope(
            "generate_local",
            "unexpected_core_response",
            format!("unexpected response: {other:?}"),
            false,
        )),
        Err(error) => Err(error_envelope(
            "generate_local",
            "local_generation_failed",
            error.to_string(),
            true,
        )),
    }
}

#[tauri::command]
#[allow(clippy::result_large_err)] // Tauri serializes the shared structured error contract directly.
fn get_audit_events(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<AuditEvent>, ErrorEnvelope> {
    state
        .audit
        .lock()
        .map_err(|_| {
            error_envelope(
                "get_audit_events",
                "audit_lock_failed",
                "audit log lock is poisoned".to_owned(),
                true,
            )
        })?
        .recent(limit.unwrap_or(100))
        .map_err(|error| {
            error_envelope(
                "get_audit_events",
                "audit_read_failed",
                error.to_string(),
                true,
            )
        })
}

#[allow(clippy::result_large_err)] // Validation must preserve the shared IPC error contract.
fn validate_generation_input(
    mut input: LocalGenerationInput,
) -> Result<LocalGenerationInput, ErrorEnvelope> {
    input.prompt = input.prompt.trim().to_owned();
    input.model_id = input.model_id.trim().to_owned();
    if input.prompt.is_empty() || input.prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(error_envelope(
            "generate_local",
            "invalid_prompt",
            format!("prompt must contain 1 to {MAX_PROMPT_CHARS} characters"),
            false,
        ));
    }
    if let Some(temperature) = input.temperature
        && (!temperature.is_finite() || !(0.0..=2.0).contains(&temperature))
    {
        return Err(error_envelope(
            "generate_local",
            "invalid_temperature",
            "temperature must be between 0 and 2".to_owned(),
            false,
        ));
    }
    if let Some(tokens) = input.max_output_tokens
        && !(1..=MAX_OUTPUT_TOKENS).contains(&tokens)
    {
        return Err(error_envelope(
            "generate_local",
            "invalid_output_budget",
            format!("max output tokens must be between 1 and {MAX_OUTPUT_TOKENS}"),
            false,
        ));
    }
    Ok(input)
}

fn record_result_audit<T>(
    state: &AppState,
    operation: &str,
    target_ids: &[String],
    result: &Result<T, ErrorEnvelope>,
    mut details: BTreeMap<String, Value>,
) {
    let outcome = if result.is_ok() {
        AuditOutcome::Succeeded
    } else {
        AuditOutcome::Failed
    };
    if let Err(error) = result {
        details.insert(
            "error_code".to_owned(),
            Value::String(error.machine_readable_code.clone()),
        );
    }
    let event = AuditEvent {
        id: AuditEventId::new(format!("audit:{}", Uuid::new_v4()))
            .expect("generated audit id is non-empty"),
        occurred_at: Utc::now(),
        actor: "local-user".to_owned(),
        operation: operation.to_owned(),
        target_ids: target_ids.to_vec(),
        outcome,
        elevated: false,
        details,
    };
    match state.audit.lock() {
        Ok(mut audit) => {
            if let Err(error) = audit.append(&event) {
                error!(operation, error = %error, "failed to persist audit event");
            }
        }
        Err(_) => error!(operation, "audit log lock is poisoned"),
    }
}

fn ollama_provider_id() -> ProviderId {
    ProviderId::new("ollama").expect("static provider id is valid")
}

fn error_envelope(
    operation: &str,
    code: &str,
    technical_message: String,
    retryable: bool,
) -> ErrorEnvelope {
    let human_fallback_message = match operation {
        "generate_local" => "ローカルAIの実行に失敗しました。",
        "get_local_provider_status" => "ローカルAIの状態を取得できませんでした。",
        "get_ai_environment" => "AI環境の状態を取得できませんでした。",
        "unload_local_model" => "モデルをメモリから解放できませんでした。",
        "start_model_download" => "モデルのダウンロードを開始できませんでした。",
        "list_model_downloads" => "ダウンロード状況を取得できませんでした。",
        "cancel_model_download" => "モデルのダウンロードを中止できませんでした。",
        "get_audit_events" => "監査ログを取得できませんでした。",
        "get_memory_core_status" => "Vertex Memory Coreの状態を取得できませんでした。",
        "start_memory_core" => "Vertex Memory Coreを起動できませんでした。",
        "stop_memory_core" => "Vertex Memory Coreを停止できませんでした。",
        "restart_memory_core" => "Vertex Memory Coreを再起動できませんでした。",
        "diagnose_memory_core" => "Vertex Memory Coreを診断できませんでした。",
        "search_system_memories" => "AI Memoryを検索できませんでした。",
        "store_system_memory" => "AI Memoryを保存できませんでした。",
        "register_developer_workspace" => "Workspaceを登録できませんでした。",
        "list_developer_workspaces" => "Workspace一覧を取得できませんでした。",
        "start_developer_task" => "Developer Agentを開始できませんでした。",
        "get_developer_task" | "list_developer_tasks" => {
            "Developer Agentの実行状況を取得できませんでした。"
        }
        "cancel_developer_task" => "Developer Agentをキャンセルできませんでした。",
        "rollback_developer_task" => "Developer Agentの変更を戻せませんでした。",
        _ => "環境情報を取得できませんでした。",
    };
    ErrorEnvelope {
        error_id: ErrorId::new(format!("error:{}", Uuid::new_v4()))
            .expect("generated error id is non-empty"),
        timestamp: Utc::now(),
        component: "vertex-ai-desktop".to_owned(),
        operation: operation.to_owned(),
        severity: Severity::Error,
        machine_readable_code: code.to_owned(),
        human_fallback_message: human_fallback_message.to_owned(),
        technical_message,
        causes: Vec::new(),
        evidence_refs: Vec::new(),
        suggested_check_ids: Vec::new(),
        recoverable: true,
        retryable,
    }
}

fn memory_error(operation: &str, code: &str, error: impl std::fmt::Display) -> ErrorEnvelope {
    error_envelope(operation, code, error.to_string(), true)
}

fn developer_error(operation: &str, error: DeveloperError) -> ErrorEnvelope {
    let retryable = matches!(
        error,
        DeveloperError::Terminal(_) | DeveloperError::Model(_) | DeveloperError::Store(_)
    );
    error_envelope(
        operation,
        "developer_agent_failed",
        error.to_string(),
        retryable,
    )
}

#[allow(clippy::result_large_err)] // IPC uses the shared structured error contract.
fn parse_memory_category(value: Option<&str>) -> Result<MemoryCategory, ErrorEnvelope> {
    match value.unwrap_or("knowledge") {
        "working" => Ok(MemoryCategory::Working),
        "conversation" => Ok(MemoryCategory::Conversation),
        "long_term" => Ok(MemoryCategory::LongTerm),
        "project" => Ok(MemoryCategory::Project),
        "knowledge" => Ok(MemoryCategory::Knowledge),
        "decision" => Ok(MemoryCategory::Decision),
        "preference" => Ok(MemoryCategory::Preference),
        "experience" => Ok(MemoryCategory::Experience),
        "success" => Ok(MemoryCategory::Success),
        "failure" => Ok(MemoryCategory::Failure),
        "system" => Ok(MemoryCategory::System),
        "vxn_knowledge" => Ok(MemoryCategory::VxnKnowledge),
        _ => Err(error_envelope(
            "store_system_memory",
            "invalid_memory_category",
            "unsupported memory category".to_owned(),
            false,
        )),
    }
}

async fn connect_memory_repository(state: &AppState) -> Result<(), ErrorEnvelope> {
    let options = state
        .memory_runtime
        .application_connect_options()
        .await
        .map_err(|error| memory_error("start_memory_core", "memory_credential_failed", error))?;
    let repository = Arc::new(
        PostgresMemoryRepository::connect_with_options(options, 8)
            .await
            .map_err(|error| memory_error("start_memory_core", "memory_connect_failed", error))?,
    );
    repository
        .migrate()
        .await
        .map_err(|error| memory_error("start_memory_core", "memory_migration_failed", error))?;
    state.core.configure_memory_repository(repository).await;
    Ok(())
}

async fn start_and_connect_memory_core(
    state: &AppState,
) -> Result<ManagedRuntimeSnapshot, ErrorEnvelope> {
    state
        .memory_runtime
        .start()
        .await
        .map_err(|error| memory_error("start_memory_core", "memory_start_failed", error))?;
    connect_memory_repository(state).await?;
    state
        .memory_runtime
        .inspect_managed()
        .await
        .map_err(|error| memory_error("start_memory_core", "memory_status_failed", error))
}

fn start_background_refresh(core: Arc<VertexAiCore>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(BACKGROUND_REFRESH_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = core
                .execute(Command::ScanEnvironment {
                    path_override: None,
                })
                .await
            {
                warn!(error = %error, "background environment refresh failed");
            }
            if let Err(error) = core
                .execute(Command::GetModels {
                    provider_id: Some(ollama_provider_id()),
                    refresh: true,
                })
                .await
            {
                warn!(error = %error, "background Ollama model refresh failed");
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let application = tauri::Builder::default()
        .setup(|app| {
            let config = CoreConfig::from_env();
            let _ = init_logging(&config.log_filter);
            let secret_store: Arc<dyn SecretStore> =
                Arc::new(WindowsCredentialStore::new("vertex-ai")?);
            let app_data_dir = app.path().app_data_dir()?;
            let index =
                PersistentEnvironmentIndex::open(app_data_dir.join("environment-index-v1.json"))?;
            let audit = PersistentAuditLog::open(app_data_dir.join("audit-v1.jsonl"))?;
            let downloads =
                ModelDownloadCoordinator::open(app_data_dir.join("runtime-jobs-v1.json"))?;
            let packaged_runtime = app
                .path()
                .resource_dir()?
                .join("runtime")
                .join("postgresql")
                .join(MANAGED_POSTGRES_VERSION)
                .join("pgsql");
            let development_runtime = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("runtime")
                .join("postgresql")
                .join(MANAGED_POSTGRES_VERSION)
                .join("pgsql");
            let runtime_root = if packaged_runtime.join("bin/postgres.exe").is_file() {
                packaged_runtime
            } else {
                development_runtime
            };
            let memory_runtime = Arc::new(ManagedPostgresRuntime::new(
                ManagedPostgresPaths {
                    runtime_root,
                    data_root: app_data_dir.join("Memory").join("PostgreSQL"),
                },
                secret_store.clone(),
            ));
            let memory_repository = tauri::async_runtime::block_on(async {
                memory_runtime
                    .ensure_ready()
                    .await
                    .map_err(|error| error.to_string())?;
                let options = memory_runtime
                    .application_connect_options()
                    .await
                    .map_err(|error| error.to_string())?;
                let repository = Arc::new(
                    PostgresMemoryRepository::connect_with_options(options, 8)
                        .await
                        .map_err(|error| error.to_string())?,
                );
                repository
                    .migrate()
                    .await
                    .map_err(|error| error.to_string())?;
                Ok::<_, String>(repository)
            });
            let developer_store: Arc<dyn DeveloperStore> = match &memory_repository {
                Ok(repository) => Arc::new(PostgresDeveloperStore::new(repository.pool().clone())),
                Err(_) => Arc::new(JsonDeveloperStore::open(
                    app_data_dir.join("developer-agent-v1.json"),
                )?),
            };
            let workspace_registry = Arc::new(WorkspaceRegistry::open(
                app_data_dir.join("developer-workspaces-v1.json"),
            )?);
            let developer = Arc::new(DeveloperCoordinator::new(
                workspace_registry,
                developer_store,
            ));
            let ard = Arc::new(ArdCoordinator::open(
                app_data_dir.join("ard-orchestrator-v1.json"),
            )?);
            if let Some(vertex_root) = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(4)
                .filter(|path| path.join("ProgramSource/Cargo.toml").is_file())
                && let Err(error) = tauri::async_runtime::block_on(
                    developer.register_workspace("Vertex AI", vertex_root),
                )
            {
                warn!(error = %error, "failed to register the built-in Vertex AI workspace");
            }
            let core_builder = VertexAiCore::new(config, secret_store)
                .with_environment_index(index)
                .with_download_coordinator(downloads);
            let core = Arc::new(match memory_repository {
                Ok(repository) => core_builder.with_memory_repository(repository),
                Err(error) => {
                    warn!(error = %error, "Vertex Memory Core started in degraded mode");
                    core_builder
                }
            });
            let ollama = Arc::new(OllamaProvider::new(OllamaProviderConfig::default())?);
            tauri::async_runtime::block_on(core.register_provider(ollama.clone()))?;
            tauri::async_runtime::block_on(core.register_runtime(ollama))?;
            start_background_refresh(core.clone());
            app.manage(AppState {
                core,
                audit: Mutex::new(audit),
                memory_runtime,
                developer,
                ard,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_environment,
            get_environment_snapshot,
            get_local_provider_status,
            get_ai_environment,
            unload_local_model,
            start_model_download,
            list_model_downloads,
            cancel_model_download,
            generate_local,
            get_audit_events,
            get_memory_core_status,
            start_memory_core,
            stop_memory_core,
            restart_memory_core,
            diagnose_memory_core,
            search_system_memories,
            store_system_memory,
            register_developer_workspace,
            list_developer_workspaces,
            start_developer_task,
            get_developer_task,
            list_developer_tasks,
            cancel_developer_task,
            rollback_developer_task,
            create_ard_team,
            list_ard_teams,
            create_ard_workflow,
            list_ard_workflows,
            start_ard_session,
            get_ard_session,
            list_ard_sessions,
            get_ard_assignment,
            complete_ard_stage,
            pause_ard_session,
            resume_ard_session,
            cancel_ard_session,
            intervene_ard_session,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Vertex AI desktop application");
    application.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            let runtime = app_handle.state::<AppState>().memory_runtime.clone();
            if let Err(error) = tauri::async_runtime::block_on(runtime.stop()) {
                warn!(error = %error, "failed to stop Vertex Memory Core during shutdown");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires a running Ollama instance with qwen3:8b"]
    async fn actual_ollama_read_only_developer_agent_acceptance() {
        let secret_store: Arc<dyn SecretStore> =
            Arc::new(WindowsCredentialStore::new("vertex-ai-developer-acceptance").unwrap());
        let core = Arc::new(VertexAiCore::new(CoreConfig::from_env(), secret_store));
        let ollama = Arc::new(OllamaProvider::new(OllamaProviderConfig::default()).unwrap());
        core.register_provider(ollama.clone()).await.unwrap();
        core.register_runtime(ollama).await.unwrap();
        core.execute(Command::GetModels {
            provider_id: Some(ollama_provider_id()),
            refresh: true,
        })
        .await
        .unwrap();

        let temporary = tempfile::tempdir().unwrap();
        let registry =
            Arc::new(WorkspaceRegistry::open(temporary.path().join("workspaces.json")).unwrap());
        let store: Arc<dyn DeveloperStore> =
            Arc::new(JsonDeveloperStore::open(temporary.path().join("tasks.json")).unwrap());
        let developer = DeveloperCoordinator::new(registry, store);
        let vertex_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .unwrap();
        let workspace = developer
            .register_workspace("Vertex AI", vertex_root)
            .await
            .unwrap();
        let model_ref = ModelRef::new(ollama_provider_id(), ModelId::new("qwen3:8b").unwrap());
        let model: Arc<dyn AgentModel> = Arc::new(CoreAgentModel {
            core,
            model: model_ref,
            name: "ollama/qwen3:8b".to_owned(),
        });
        let task = developer
            .start_task(
                StartDeveloperTask {
                    workspace_id: workspace.id,
                    request: "ProgramSource/crates/vertex-ai-runtime/src/lib.rsを読み、Runtime Managerの構造を変更せず日本語で報告してください。".to_owned(),
                    mode: vertex_ai_developer::DeveloperMode::ReadOnly,
                    provider_id: "ollama".to_owned(),
                    model_id: "qwen3:8b".to_owned(),
                    limits: Some(vertex_ai_developer::AgentLimits {
                        max_steps: 12,
                        max_tool_calls: 10,
                        max_runtime_seconds: 180,
                        max_failed_attempts: 5,
                        max_consecutive_errors: 3,
                        max_context_chars: 60_000,
                    }),
                },
                Arc::new(JsonDeveloperEngine::new(model)),
            )
            .await
            .unwrap();

        for _ in 0..720 {
            let current = developer.get_task(task.id).await.unwrap();
            if current.state.is_terminal()
                || current.state == vertex_ai_developer::DeveloperTaskState::WaitingApproval
            {
                assert_eq!(
                    current.state,
                    vertex_ai_developer::DeveloperTaskState::Completed,
                    "{current:#?}"
                );
                assert!(current.files_changed.is_empty());
                assert!(current.result_summary.is_some());
                return;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        panic!("actual Ollama Developer Agent acceptance timed out");
    }

    #[test]
    fn ipc_errors_preserve_machine_and_human_information() {
        let error = error_envelope(
            "scan_environment",
            "scan_failed",
            "raw failure".to_owned(),
            true,
        );
        assert_eq!(error.machine_readable_code, "scan_failed");
        assert_eq!(error.technical_message, "raw failure");
        assert!(error.retryable);
    }

    #[test]
    fn local_generation_rejects_blank_and_unbounded_input() {
        let blank = validate_generation_input(LocalGenerationInput {
            model_id: "qwen3:8b".to_owned(),
            prompt: "  ".to_owned(),
            temperature: None,
            max_output_tokens: None,
        });
        assert!(blank.is_err());
        let oversized = validate_generation_input(LocalGenerationInput {
            model_id: "qwen3:8b".to_owned(),
            prompt: "hello".to_owned(),
            temperature: Some(3.0),
            max_output_tokens: Some(MAX_OUTPUT_TOKENS + 1),
        });
        assert!(oversized.is_err());
    }
}
