use vertex_ai_context::{BuiltContext, ContextBuildReport, ContextBuildRequest, PreparedContext};
use vertex_ai_environment::{EnvironmentSnapshot, IndexedEnvironmentSnapshot};
use vertex_ai_memory::{MemoryProposal, MemoryQuery, MemoryRecord, MemoryWritePermit};
use vertex_ai_provider::ProviderHealth;
use vertex_ai_secrets::SecretValue;
use vertex_ai_types::{
    AiEnvironmentSummary, GenerationParameters, GenerationResponse, LocalRuntimeSnapshot, Message,
    ModelDescriptor, ModelDownloadJob, ModelId, ModelRef, ProviderId, RuntimeJobId,
};

/// Internal transport-neutral command model. Transport adapters map their DTOs into these values.
#[derive(Debug)]
pub enum Command {
    ScanEnvironment {
        path_override: Option<String>,
    },
    GetEnvironmentSnapshot,
    GetModels {
        provider_id: Option<ProviderId>,
        refresh: bool,
    },
    GetProviderHealth {
        provider_id: ProviderId,
    },
    GetAiEnvironment,
    UnloadLocalModel {
        provider_id: ProviderId,
        model_id: ModelId,
    },
    StartModelDownload {
        provider_id: ProviderId,
        model_id: ModelId,
    },
    CancelModelDownload {
        job_id: RuntimeJobId,
    },
    ListModelDownloads,
    SelectModel {
        model: ModelRef,
    },
    Generate {
        model: Option<ModelRef>,
        messages: Vec<Message>,
        context: Box<PreparedContext>,
        parameters: GenerationParameters,
    },
    StoreProviderSecret {
        provider_id: ProviderId,
        value: SecretValue,
    },
    DeleteProviderSecret {
        provider_id: ProviderId,
    },
    ProposeMemory {
        proposal: Box<MemoryProposal>,
        permit: Box<MemoryWritePermit>,
    },
    RecallMemory {
        query: Box<MemoryQuery>,
    },
    BuildContext {
        request: Box<ContextBuildRequest>,
    },
    Reason {
        model: Option<ModelRef>,
        messages: Vec<Message>,
        context_request: Box<ContextBuildRequest>,
        parameters: GenerationParameters,
    },
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ScanEnvironment { .. } => "scan_environment",
            Self::GetEnvironmentSnapshot => "get_environment_snapshot",
            Self::GetModels { .. } => "get_models",
            Self::GetProviderHealth { .. } => "get_provider_health",
            Self::GetAiEnvironment => "get_ai_environment",
            Self::UnloadLocalModel { .. } => "unload_local_model",
            Self::StartModelDownload { .. } => "start_model_download",
            Self::CancelModelDownload { .. } => "cancel_model_download",
            Self::ListModelDownloads => "list_model_downloads",
            Self::SelectModel { .. } => "select_model",
            Self::Generate { .. } => "generate",
            Self::StoreProviderSecret { .. } => "store_provider_secret",
            Self::DeleteProviderSecret { .. } => "delete_provider_secret",
            Self::ProposeMemory { .. } => "propose_memory",
            Self::RecallMemory { .. } => "recall_memory",
            Self::BuildContext { .. } => "build_context",
            Self::Reason { .. } => "reason",
        }
    }
}

#[derive(Debug)]
pub enum CommandResponse {
    EnvironmentScanned(IndexedEnvironmentSnapshot),
    EnvironmentSnapshot(Option<EnvironmentSnapshot>),
    Models(Vec<ModelDescriptor>),
    ProviderHealth(ProviderHealth),
    AiEnvironment(AiEnvironmentSummary),
    LocalModelUnloaded(LocalRuntimeSnapshot),
    ModelDownloadStarted(ModelDownloadJob),
    ModelDownloadCancelled(ModelDownloadJob),
    ModelDownloads(Vec<ModelDownloadJob>),
    ModelSelected(ModelRef),
    Generated(GenerationResponse),
    SecretStored,
    SecretDeleted(bool),
    MemoryStored(MemoryRecord),
    Memories(Vec<MemoryRecord>),
    ContextBuilt(BuiltContext),
    Reasoned {
        response: GenerationResponse,
        context_report: ContextBuildReport,
    },
}
