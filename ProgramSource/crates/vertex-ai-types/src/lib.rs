//! Provider-neutral domain types shared across Vertex AI boundaries.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

pub const VERTEX_CONTEXT_VERSION: &str = "0.1";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{kind} cannot be empty")]
pub struct IdentifierError {
    kind: &'static str,
}

macro_rules! identifier {
    ($name:ident, $kind:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(IdentifierError { kind: $kind });
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier!(ProviderId, "provider id");
identifier!(ModelId, "model id");
identifier!(SystemAssetId, "system asset id");
identifier!(FindingId, "finding id");
identifier!(HealthCheckId, "health check id");
identifier!(ErrorId, "error id");
identifier!(RepairPlanId, "repair plan id");
identifier!(AuditEventId, "audit event id");
identifier!(RuntimeJobId, "runtime job id");

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ModelRef {
    pub provider_id: ProviderId,
    pub model_id: ModelId,
}

impl ModelRef {
    pub fn new(provider_id: ProviderId, model_id: ModelId) -> Self {
        Self {
            provider_id,
            model_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModelCapabilities {
    pub tools: bool,
    pub vision: bool,
    pub structured_output: bool,
    pub streaming: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub reference: ModelRef,
    pub display_name: String,
    pub capabilities: ModelCapabilities,
    pub context_size: Option<u32>,
    pub local: bool,
    pub input_cost_per_million: Option<f64>,
    pub output_cost_per_million: Option<f64>,
    pub available: bool,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PrivacyPolicy {
    pub local_only: bool,
    pub cloud_allowed: bool,
    pub sensitive: bool,
    pub share_scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VertexContext {
    pub vertex_context: String,
    pub task: Value,
    pub application: Value,
    pub project: Value,
    pub user_context: Value,
    pub memories: Vec<Value>,
    pub decisions: Vec<Value>,
    pub constraints: Vec<Value>,
    pub schema: Value,
    pub tools: Vec<Value>,
    pub permissions: Value,
    pub runtime: Value,
    pub privacy_policy: PrivacyPolicy,
}

impl Default for VertexContext {
    fn default() -> Self {
        Self {
            vertex_context: VERTEX_CONTEXT_VERSION.to_owned(),
            task: Value::Object(Map::new()),
            application: Value::Object(Map::new()),
            project: Value::Object(Map::new()),
            user_context: Value::Object(Map::new()),
            memories: Vec::new(),
            decisions: Vec::new(),
            constraints: Vec::new(),
            schema: Value::Object(Map::new()),
            tools: Vec::new(),
            permissions: Value::Object(Map::new()),
            runtime: Value::Object(Map::new()),
            privacy_policy: PrivacyPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GenerationParameters {
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stop: Vec<String>,
    pub structured_output_schema: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub context: VertexContext,
    pub parameters: GenerationParameters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationResponse {
    pub response_id: Uuid,
    pub model: ModelRef,
    pub text: String,
    pub usage: TokenUsage,
    pub finish_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamChunk {
    pub text: String,
    pub finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSelectionMode {
    Manual,
    Auto,
    Council,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetCategory {
    Ai,
    Developer,
    Creator,
    Runtime,
    Database,
    Server,
    System,
    Hardware,
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Application,
    Executable,
    Runtime,
    Service,
    Process,
    Dependency,
    Sdk,
    Driver,
    StorageDevice,
    Configuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Ready,
    Warning,
    Offline,
    Misconfigured,
    MissingDependency,
    ConflictDetected,
    OrphanDetected,
    RepairAvailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub source: String,
    pub locator: String,
    pub observed_at: DateTime<Utc>,
    pub content_hash: Option<String>,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemAsset {
    pub id: SystemAssetId,
    pub name: String,
    pub category: AssetCategory,
    pub kind: AssetKind,
    pub location: Option<String>,
    pub version: Option<String>,
    pub architecture: Option<String>,
    pub health: HealthState,
    pub capabilities: Vec<String>,
    pub evidence: Vec<EvidenceRef>,
    pub observed_at: DateTime<Utc>,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    InstalledAt,
    Executes,
    DependsOn,
    Provides,
    References,
    Stores,
    Launches,
    ListensOn,
    CompatibleWith,
    Duplicates,
    Supersedes,
    OrphanedFrom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemRelationship {
    pub source_id: String,
    pub target_id: String,
    pub kind: RelationshipKind,
    pub verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingConfidence {
    Verified,
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentFinding {
    pub id: FindingId,
    pub title: String,
    pub observed_fact: String,
    pub inference: Option<String>,
    pub recommendation: Option<String>,
    pub confidence: FindingConfidence,
    pub severity: Severity,
    pub evidence: Vec<EvidenceRef>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthCheck {
    pub id: HealthCheckId,
    pub subject_id: String,
    pub state: HealthState,
    pub summary: String,
    pub evidence: Vec<EvidenceRef>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error_id: ErrorId,
    pub timestamp: DateTime<Utc>,
    pub component: String,
    pub operation: String,
    pub severity: Severity,
    pub machine_readable_code: String,
    pub human_fallback_message: String,
    pub technical_message: String,
    pub causes: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub suggested_check_ids: Vec<HealthCheckId>,
    pub recoverable: bool,
    pub retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairRisk {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum RepairAction {
    SetEnvironmentVariable {
        name: String,
        value: String,
    },
    RemoveEnvironmentVariable {
        name: String,
    },
    UpdateProviderEndpoint {
        provider_id: ProviderId,
        endpoint: String,
    },
    MovePath {
        source: String,
        destination: String,
        expected_hash: Option<String>,
    },
    DeletePath {
        path: String,
        expected_hash: String,
    },
    StartService {
        service_name: String,
    },
    StopService {
        service_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepairPlan {
    pub repair_plan_id: RepairPlanId,
    pub finding_ids: Vec<FindingId>,
    pub summary: String,
    pub risk: RepairRisk,
    pub requires_elevation: bool,
    pub requires_restart: bool,
    pub reversible: bool,
    pub backup_strategy: Option<String>,
    pub actions: Vec<RepairAction>,
    pub verification_steps: Vec<String>,
    pub rollback_actions: Vec<RepairAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Succeeded,
    Failed,
    Rejected,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: AuditEventId,
    pub occurred_at: DateTime<Utc>,
    pub actor: String,
    pub operation: String,
    pub target_ids: Vec<String>,
    pub outcome: AuditOutcome,
    pub elevated: bool,
    pub details: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledLocalModel {
    pub reference: ModelRef,
    pub display_name: String,
    pub size_bytes: u64,
    pub digest: Option<String>,
    pub format: Option<String>,
    pub family: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
    pub context_length: Option<u64>,
    pub modified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadedLocalModel {
    pub reference: ModelRef,
    pub size_bytes: u64,
    pub size_vram_bytes: u64,
    pub context_length: Option<u64>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalRuntimeSnapshot {
    pub provider_id: ProviderId,
    pub display_name: String,
    pub endpoint: String,
    pub health: HealthState,
    pub version: Option<String>,
    pub executable_path: Option<String>,
    pub model_storage_path: Option<String>,
    pub storage_total_bytes: Option<u64>,
    pub storage_available_bytes: Option<u64>,
    pub installed_models: Vec<InstalledLocalModel>,
    pub loaded_models: Vec<LoadedLocalModel>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelDownloadState {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelDownloadProgress {
    pub status: String,
    pub completed_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelDownloadJob {
    pub id: RuntimeJobId,
    pub provider_id: ProviderId,
    pub model_id: ModelId,
    pub state: ModelDownloadState,
    pub status: String,
    pub completed_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiEnvironmentSummary {
    pub runtimes: Vec<LocalRuntimeSnapshot>,
    pub runtime_count: usize,
    pub ready_runtime_count: usize,
    pub installed_model_count: usize,
    pub loaded_model_count: usize,
    pub total_model_bytes: u64,
    pub total_vram_bytes: u64,
    pub local_inference_ready: bool,
    pub observed_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_uses_protocol_version() {
        let context = VertexContext::default();
        let json = serde_json::to_value(context).expect("context serializes");
        assert_eq!(json["vertex_context"], VERTEX_CONTEXT_VERSION);
    }

    #[test]
    fn identifiers_reject_blank_values() {
        assert!(ProviderId::new("  ").is_err());
        assert!(ModelId::new("").is_err());
    }

    #[test]
    fn repair_actions_are_typed_and_never_arbitrary_shell() {
        let action = RepairAction::MovePath {
            source: "C:/models/a.gguf".to_owned(),
            destination: "D:/models/a.gguf".to_owned(),
            expected_hash: Some("sha256:abc".to_owned()),
        };
        let json = serde_json::to_value(action).expect("repair action serializes");
        assert_eq!(json["operation"], "move_path");
        assert!(json.get("command").is_none());
    }

    #[test]
    fn system_assets_keep_observed_evidence_separate() {
        let observed_at = Utc::now();
        let asset = SystemAsset {
            id: SystemAssetId::new("executable:python").expect("valid id"),
            name: "Python".to_owned(),
            category: AssetCategory::Developer,
            kind: AssetKind::Runtime,
            location: Some("C:/Python/python.exe".to_owned()),
            version: None,
            architecture: None,
            health: HealthState::Ready,
            capabilities: vec!["development.python".to_owned()],
            evidence: vec![EvidenceRef {
                source: "path_scan".to_owned(),
                locator: "C:/Python/python.exe".to_owned(),
                observed_at,
                content_hash: None,
                metadata: BTreeMap::new(),
            }],
            observed_at,
            metadata: BTreeMap::new(),
        };
        assert_eq!(asset.evidence[0].source, "path_scan");
        assert_eq!(asset.health, HealthState::Ready);
    }
}
