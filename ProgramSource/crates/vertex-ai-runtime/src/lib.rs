//! Provider-neutral local AI runtime inspection and durable background model jobs.

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock, mpsc, watch};
use uuid::Uuid;
use vertex_ai_types::{
    LocalRuntimeSnapshot, ModelDownloadJob, ModelDownloadProgress, ModelDownloadState, ModelId,
    ProviderId, RuntimeJobId,
};

const JOB_SCHEMA_VERSION: u32 = 1;

/// Lifecycle state shared by infrastructure runtimes such as Vertex Memory Core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagedRuntimeState {
    Initializing,
    Ready,
    Stopped,
    Degraded,
    Error,
    RepairRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDiagnosis {
    pub code: String,
    pub summary: String,
    pub detail: String,
    pub repairable: bool,
    pub destructive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedRuntimeSnapshot {
    pub id: String,
    pub display_name: String,
    pub state: ManagedRuntimeState,
    pub version: Option<String>,
    pub runtime_location: String,
    pub data_location: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub schema_version: Option<String>,
    pub database_size_bytes: Option<u64>,
    pub connection_count: Option<u32>,
    pub backup_state: String,
    pub last_successful_start: Option<chrono::DateTime<Utc>>,
    pub last_error: Option<String>,
    pub observed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBackupSnapshot {
    pub state: String,
    pub last_backup: Option<chrono::DateTime<Utc>>,
    pub location: Option<String>,
    pub last_result: Option<String>,
}

/// Boundary reserved for pre-migration, upgrade, and repair backups. Concrete
/// runtimes may add implementation without coupling UI code to database tools.
#[async_trait]
pub trait ManagedRuntimeBackup: Send + Sync {
    async fn inspect_backup(&self) -> Result<RuntimeBackupSnapshot, RuntimeError>;
    async fn create_backup(&self) -> Result<RuntimeBackupSnapshot, RuntimeError>;
}

#[async_trait]
pub trait ManagedServiceRuntime: Send + Sync {
    fn id(&self) -> &str;
    async fn inspect_managed(&self) -> Result<ManagedRuntimeSnapshot, RuntimeError>;
    async fn start(&self) -> Result<ManagedRuntimeSnapshot, RuntimeError>;
    async fn stop(&self) -> Result<ManagedRuntimeSnapshot, RuntimeError>;
    async fn restart(&self) -> Result<ManagedRuntimeSnapshot, RuntimeError> {
        self.stop().await?;
        self.start().await
    }
    async fn diagnose(&self) -> Result<Vec<RuntimeDiagnosis>, RuntimeError>;
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime is unavailable: {0}")]
    Unavailable(String),
    #[error("runtime request is invalid: {0}")]
    InvalidRequest(String),
    #[error("runtime operation was cancelled")]
    Cancelled,
    #[error("runtime operation failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeModelStateKind {
    Loaded,
    Loading,
    Unloading,
    Unloaded,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeModelState {
    pub runtime_id: ProviderId,
    pub model_id: ModelId,
    pub state: RuntimeModelStateKind,
    pub observed: bool,
    pub detail: String,
    pub observed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOperationControl {
    Continue,
    Pause,
    Cancel,
}

/// Runtime-specific model residency control used by ARD rotation. Implementations
/// must verify actual runtime state instead of trusting persisted ARD state.
#[async_trait]
pub trait ModelRuntimeAdapter: Send + Sync {
    fn runtime_id(&self) -> &ProviderId;
    async fn model_state(&self, model_id: &ModelId) -> Result<RuntimeModelState, RuntimeError>;
    async fn load_model(
        &self,
        model_id: &ModelId,
        control: watch::Receiver<RuntimeOperationControl>,
    ) -> Result<RuntimeModelState, RuntimeError>;
    async fn release_model(
        &self,
        model_id: &ModelId,
        control: watch::Receiver<RuntimeOperationControl>,
    ) -> Result<RuntimeModelState, RuntimeError>;
}

#[async_trait]
pub trait LocalRuntimeManager: Send + Sync {
    fn id(&self) -> &ProviderId;
    async fn inspect(&self) -> Result<LocalRuntimeSnapshot, RuntimeError>;
    async fn unload_model(&self, model_id: &ModelId) -> Result<(), RuntimeError>;
    async fn download_model(
        &self,
        model_id: &ModelId,
        progress: mpsc::UnboundedSender<ModelDownloadProgress>,
        cancellation: watch::Receiver<bool>,
    ) -> Result<(), RuntimeError>;
}

#[derive(Debug, Error)]
pub enum RuntimeRegistryError {
    #[error("runtime is already registered: {0}")]
    DuplicateRuntime(ProviderId),
    #[error("runtime is not registered: {0}")]
    RuntimeNotFound(ProviderId),
}

#[derive(Default)]
pub struct RuntimeRegistry {
    runtimes: RwLock<BTreeMap<ProviderId, Arc<dyn LocalRuntimeManager>>>,
}

impl RuntimeRegistry {
    pub async fn register(
        &self,
        runtime: Arc<dyn LocalRuntimeManager>,
    ) -> Result<(), RuntimeRegistryError> {
        let id = runtime.id().clone();
        let mut runtimes = self.runtimes.write().await;
        if runtimes.contains_key(&id) {
            return Err(RuntimeRegistryError::DuplicateRuntime(id));
        }
        runtimes.insert(id, runtime);
        Ok(())
    }

    pub async fn get(
        &self,
        id: &ProviderId,
    ) -> Result<Arc<dyn LocalRuntimeManager>, RuntimeRegistryError> {
        self.runtimes
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| RuntimeRegistryError::RuntimeNotFound(id.clone()))
    }

    pub async fn list(&self) -> Vec<Arc<dyn LocalRuntimeManager>> {
        self.runtimes.read().await.values().cloned().collect()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JobDocument {
    schema_version: u32,
    jobs: Vec<ModelDownloadJob>,
}

#[derive(Clone)]
pub struct ModelDownloadCoordinator {
    path: PathBuf,
    jobs: Arc<RwLock<BTreeMap<RuntimeJobId, ModelDownloadJob>>>,
    cancellations: Arc<Mutex<BTreeMap<RuntimeJobId, watch::Sender<bool>>>>,
    persist_lock: Arc<Mutex<()>>,
}

impl ModelDownloadCoordinator {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, RuntimeError> {
        let path = path.into();
        let mut jobs = if path.exists() {
            let document: JobDocument = serde_json::from_slice(
                &fs::read(&path).map_err(|error| RuntimeError::Failed(error.to_string()))?,
            )
            .map_err(|error| RuntimeError::Failed(error.to_string()))?;
            if document.schema_version != JOB_SCHEMA_VERSION {
                return Err(RuntimeError::Failed(format!(
                    "unsupported runtime job schema {}",
                    document.schema_version
                )));
            }
            document
                .jobs
                .into_iter()
                .map(|job| (job.id.clone(), job))
                .collect::<BTreeMap<_, _>>()
        } else {
            BTreeMap::new()
        };
        let now = Utc::now();
        for job in jobs.values_mut() {
            if matches!(
                job.state,
                ModelDownloadState::Queued
                    | ModelDownloadState::Running
                    | ModelDownloadState::Cancelling
            ) {
                job.state = ModelDownloadState::Interrupted;
                job.status = "application_restarted".to_owned();
                job.updated_at = now;
            }
        }
        persist_jobs(&path, jobs.values().cloned().collect())?;
        Ok(Self {
            path,
            jobs: Arc::new(RwLock::new(jobs)),
            cancellations: Arc::new(Mutex::new(BTreeMap::new())),
            persist_lock: Arc::new(Mutex::new(())),
        })
    }

    pub async fn start(
        &self,
        runtime: Arc<dyn LocalRuntimeManager>,
        model_id: ModelId,
    ) -> Result<ModelDownloadJob, RuntimeError> {
        let provider_id = runtime.id().clone();
        if self.jobs.read().await.values().any(|job| {
            job.provider_id == provider_id
                && job.model_id == model_id
                && matches!(
                    job.state,
                    ModelDownloadState::Queued
                        | ModelDownloadState::Running
                        | ModelDownloadState::Cancelling
                )
        }) {
            return Err(RuntimeError::InvalidRequest(
                "a download for this model is already active".to_owned(),
            ));
        }
        let now = Utc::now();
        let job = ModelDownloadJob {
            id: RuntimeJobId::new(format!("runtime-job:{}", Uuid::new_v4()))
                .expect("generated job id is valid"),
            provider_id,
            model_id,
            state: ModelDownloadState::Queued,
            status: "queued".to_owned(),
            completed_bytes: 0,
            total_bytes: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        self.jobs.write().await.insert(job.id.clone(), job.clone());
        self.persist().await?;
        let (cancel_tx, cancel_rx) = watch::channel(false);
        self.cancellations
            .lock()
            .await
            .insert(job.id.clone(), cancel_tx);
        let coordinator = self.clone();
        let job_id = job.id.clone();
        tokio::spawn(async move {
            coordinator.run_download(job_id, runtime, cancel_rx).await;
        });
        Ok(job)
    }

    pub async fn cancel(&self, id: &RuntimeJobId) -> Result<ModelDownloadJob, RuntimeError> {
        let sender = self
            .cancellations
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| RuntimeError::InvalidRequest("download job is not active".to_owned()))?;
        self.update(id, |job| {
            job.state = ModelDownloadState::Cancelling;
            job.status = "cancelling".to_owned();
        })
        .await?;
        sender
            .send(true)
            .map_err(|_| RuntimeError::Failed("download worker is unavailable".to_owned()))?;
        self.get(id).await
    }

    pub async fn list(&self) -> Vec<ModelDownloadJob> {
        let mut jobs = self.jobs.read().await.values().cloned().collect::<Vec<_>>();
        jobs.sort_by_key(|job| std::cmp::Reverse(job.created_at));
        jobs
    }

    pub async fn get(&self, id: &RuntimeJobId) -> Result<ModelDownloadJob, RuntimeError> {
        self.jobs
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| RuntimeError::InvalidRequest("download job was not found".to_owned()))
    }

    async fn run_download(
        &self,
        id: RuntimeJobId,
        runtime: Arc<dyn LocalRuntimeManager>,
        cancellation: watch::Receiver<bool>,
    ) {
        if *cancellation.borrow() {
            let _ = self
                .update(&id, |job| {
                    job.state = ModelDownloadState::Cancelled;
                    job.status = "cancelled".to_owned();
                })
                .await;
            self.cancellations.lock().await.remove(&id);
            return;
        }
        let _ = self
            .update(&id, |job| {
                if job.state == ModelDownloadState::Queued {
                    job.state = ModelDownloadState::Running;
                    job.status = "starting".to_owned();
                }
            })
            .await;
        let model_id = match self.get(&id).await {
            Ok(job) => job.model_id,
            Err(_) => return,
        };
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
        let download = runtime.download_model(&model_id, progress_tx, cancellation);
        tokio::pin!(download);
        let result = loop {
            tokio::select! {
                progress = progress_rx.recv() => {
                    if let Some(progress) = progress {
                        let _ = self.update(&id, |job| {
                            job.status = progress.status;
                            job.completed_bytes = progress.completed_bytes;
                            job.total_bytes = progress.total_bytes.or(job.total_bytes);
                        }).await;
                    }
                }
                result = &mut download => break result,
            }
        };
        // A very fast runtime can complete in the same scheduler turn as its
        // final progress event. Drain already-queued progress before marking
        // the job terminal so persisted byte counts are deterministic.
        while let Ok(progress) = progress_rx.try_recv() {
            let _ = self
                .update(&id, |job| {
                    job.status = progress.status;
                    job.completed_bytes = progress.completed_bytes;
                    job.total_bytes = progress.total_bytes.or(job.total_bytes);
                })
                .await;
        }
        let _ = self
            .update(&id, |job| match &result {
                Ok(()) => {
                    job.state = ModelDownloadState::Succeeded;
                    job.status = "success".to_owned();
                    if let Some(total) = job.total_bytes {
                        job.completed_bytes = total;
                    }
                }
                Err(RuntimeError::Cancelled) => {
                    job.state = ModelDownloadState::Cancelled;
                    job.status = "cancelled".to_owned();
                }
                Err(error) => {
                    job.state = ModelDownloadState::Failed;
                    job.status = "failed".to_owned();
                    job.error_message = Some(error.to_string());
                }
            })
            .await;
        self.cancellations.lock().await.remove(&id);
    }

    async fn update(
        &self,
        id: &RuntimeJobId,
        update: impl FnOnce(&mut ModelDownloadJob),
    ) -> Result<(), RuntimeError> {
        let mut jobs = self.jobs.write().await;
        let job = jobs
            .get_mut(id)
            .ok_or_else(|| RuntimeError::InvalidRequest("download job was not found".to_owned()))?;
        update(job);
        job.updated_at = Utc::now();
        drop(jobs);
        self.persist().await
    }

    async fn persist(&self) -> Result<(), RuntimeError> {
        let _guard = self.persist_lock.lock().await;
        let jobs = self.jobs.read().await.values().cloned().collect();
        persist_jobs(&self.path, jobs)
    }
}

fn persist_jobs(path: &Path, jobs: Vec<ModelDownloadJob>) -> Result<(), RuntimeError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| RuntimeError::Failed(error.to_string()))?;
    }
    let document = JobDocument {
        schema_version: JOB_SCHEMA_VERSION,
        jobs,
    };
    let next = path.with_extension("next");
    fs::write(
        &next,
        serde_json::to_vec_pretty(&document)
            .map_err(|error| RuntimeError::Failed(error.to_string()))?,
    )
    .map_err(|error| RuntimeError::Failed(error.to_string()))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| RuntimeError::Failed(error.to_string()))?;
    }
    fs::rename(next, path).map_err(|error| RuntimeError::Failed(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CompletingRuntime {
        id: ProviderId,
    }

    #[async_trait]
    impl LocalRuntimeManager for CompletingRuntime {
        fn id(&self) -> &ProviderId {
            &self.id
        }

        async fn inspect(&self) -> Result<LocalRuntimeSnapshot, RuntimeError> {
            unreachable!("not required by the coordinator test")
        }

        async fn unload_model(&self, _model_id: &ModelId) -> Result<(), RuntimeError> {
            Ok(())
        }

        async fn download_model(
            &self,
            _model_id: &ModelId,
            progress: mpsc::UnboundedSender<ModelDownloadProgress>,
            _cancellation: watch::Receiver<bool>,
        ) -> Result<(), RuntimeError> {
            progress
                .send(ModelDownloadProgress {
                    status: "pulling_manifest".to_owned(),
                    completed_bytes: 50,
                    total_bytes: Some(100),
                })
                .expect("coordinator receives progress");
            tokio::task::yield_now().await;
            Ok(())
        }
    }

    struct CancellableRuntime {
        id: ProviderId,
    }

    #[async_trait]
    impl LocalRuntimeManager for CancellableRuntime {
        fn id(&self) -> &ProviderId {
            &self.id
        }

        async fn inspect(&self) -> Result<LocalRuntimeSnapshot, RuntimeError> {
            unreachable!("not required by the coordinator test")
        }

        async fn unload_model(&self, _model_id: &ModelId) -> Result<(), RuntimeError> {
            Ok(())
        }

        async fn download_model(
            &self,
            _model_id: &ModelId,
            _progress: mpsc::UnboundedSender<ModelDownloadProgress>,
            mut cancellation: watch::Receiver<bool>,
        ) -> Result<(), RuntimeError> {
            if *cancellation.borrow() {
                return Err(RuntimeError::Cancelled);
            }
            loop {
                cancellation
                    .changed()
                    .await
                    .map_err(|_| RuntimeError::Cancelled)?;
                if *cancellation.borrow() {
                    return Err(RuntimeError::Cancelled);
                }
            }
        }
    }

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("vertex-ai-runtime-tests")
            .join(format!("{name}-{}.json", Uuid::new_v4()))
    }

    async fn await_terminal(
        coordinator: &ModelDownloadCoordinator,
        id: &RuntimeJobId,
    ) -> ModelDownloadJob {
        for _ in 0..1_000 {
            let job = coordinator.get(id).await.expect("job remains indexed");
            if !matches!(
                job.state,
                ModelDownloadState::Queued
                    | ModelDownloadState::Running
                    | ModelDownloadState::Cancelling
            ) {
                return job;
            }
            tokio::task::yield_now().await;
        }
        panic!("job did not reach a terminal state")
    }

    #[tokio::test]
    async fn completed_jobs_persist_progress_and_survive_reopen() {
        let path = test_path("complete");
        let coordinator = ModelDownloadCoordinator::open(&path).expect("coordinator opens");
        let runtime = Arc::new(CompletingRuntime {
            id: ProviderId::new("test-runtime").expect("valid id"),
        });
        let started = coordinator
            .start(runtime, ModelId::new("test-model").expect("valid id"))
            .await
            .expect("download starts");
        let completed = await_terminal(&coordinator, &started.id).await;
        assert_eq!(completed.state, ModelDownloadState::Succeeded);
        assert_eq!(completed.completed_bytes, 100);
        assert_eq!(completed.total_bytes, Some(100));

        let reopened = ModelDownloadCoordinator::open(&path).expect("coordinator reopens");
        assert_eq!(
            reopened
                .get(&started.id)
                .await
                .expect("job persisted")
                .state,
            ModelDownloadState::Succeeded
        );
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn cancellation_reaches_a_terminal_state_even_immediately_after_start() {
        let path = test_path("cancel");
        let coordinator = ModelDownloadCoordinator::open(&path).expect("coordinator opens");
        let runtime = Arc::new(CancellableRuntime {
            id: ProviderId::new("test-runtime").expect("valid id"),
        });
        let started = coordinator
            .start(runtime, ModelId::new("test-model").expect("valid id"))
            .await
            .expect("download starts");
        coordinator
            .cancel(&started.id)
            .await
            .expect("cancel accepted");
        let cancelled = await_terminal(&coordinator, &started.id).await;
        assert_eq!(cancelled.state, ModelDownloadState::Cancelled);
        let _ = fs::remove_file(path);
    }
}
