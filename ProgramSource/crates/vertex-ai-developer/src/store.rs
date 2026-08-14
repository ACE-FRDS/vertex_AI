use crate::{DeveloperError, DeveloperTask, DeveloperTaskId, Workspace};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use uuid::Uuid;

#[async_trait]
pub trait DeveloperStore: Send + Sync {
    async fn save_workspace(&self, workspace: &Workspace) -> Result<(), DeveloperError>;
    async fn save_task(&self, task: &DeveloperTask) -> Result<(), DeveloperError>;
    async fn append_event(
        &self,
        task_id: DeveloperTaskId,
        sequence: u64,
        event_type: &str,
        payload: Value,
    ) -> Result<(), DeveloperError>;
    async fn load_task(
        &self,
        task_id: DeveloperTaskId,
    ) -> Result<Option<DeveloperTask>, DeveloperError>;
    async fn list_tasks(&self, limit: usize) -> Result<Vec<DeveloperTask>, DeveloperError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredEvent {
    event_id: Uuid,
    task_id: DeveloperTaskId,
    sequence: u64,
    event_type: String,
    payload: Value,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct JsonDocument {
    workspaces: BTreeMap<Uuid, Workspace>,
    tasks: BTreeMap<DeveloperTaskId, DeveloperTask>,
    events: Vec<StoredEvent>,
}

pub struct JsonDeveloperStore {
    path: PathBuf,
    document: Mutex<JsonDocument>,
}

impl JsonDeveloperStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, DeveloperError> {
        let path = path.into();
        let document = if path.is_file() {
            serde_json::from_slice(&fs::read(&path)?)?
        } else {
            JsonDocument::default()
        };
        Ok(Self {
            path,
            document: Mutex::new(document),
        })
    }

    fn update(&self, update: impl FnOnce(&mut JsonDocument)) -> Result<(), DeveloperError> {
        let mut document = self
            .document
            .lock()
            .map_err(|_| DeveloperError::Store("developer JSON store lock failed".to_owned()))?;
        update(&mut document);
        persist_atomically(&self.path, &serde_json::to_vec_pretty(&*document)?)
    }
}

#[async_trait]
impl DeveloperStore for JsonDeveloperStore {
    async fn save_workspace(&self, workspace: &Workspace) -> Result<(), DeveloperError> {
        self.update(|document| {
            document.workspaces.insert(workspace.id, workspace.clone());
        })
    }

    async fn save_task(&self, task: &DeveloperTask) -> Result<(), DeveloperError> {
        self.update(|document| {
            document.tasks.insert(task.id, task.clone());
        })
    }

    async fn append_event(
        &self,
        task_id: DeveloperTaskId,
        sequence: u64,
        event_type: &str,
        payload: Value,
    ) -> Result<(), DeveloperError> {
        let event_type = event_type.to_owned();
        self.update(|document| {
            document
                .events
                .retain(|event| !(event.task_id == task_id && event.sequence == sequence));
            document.events.push(StoredEvent {
                event_id: Uuid::new_v4(),
                task_id,
                sequence,
                event_type,
                payload,
            });
        })
    }

    async fn load_task(
        &self,
        task_id: DeveloperTaskId,
    ) -> Result<Option<DeveloperTask>, DeveloperError> {
        Ok(self
            .document
            .lock()
            .map_err(|_| DeveloperError::Store("developer JSON store lock failed".to_owned()))?
            .tasks
            .get(&task_id)
            .cloned())
    }

    async fn list_tasks(&self, limit: usize) -> Result<Vec<DeveloperTask>, DeveloperError> {
        let mut tasks = self
            .document
            .lock()
            .map_err(|_| DeveloperError::Store("developer JSON store lock failed".to_owned()))?
            .tasks
            .values()
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by_key(|task| std::cmp::Reverse(task.updated_at));
        tasks.truncate(limit.min(500));
        Ok(tasks)
    }
}

#[derive(Clone)]
pub struct PostgresDeveloperStore {
    pool: PgPool,
}

impl PostgresDeveloperStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DeveloperStore for PostgresDeveloperStore {
    async fn save_workspace(&self, workspace: &Workspace) -> Result<(), DeveloperError> {
        sqlx::query(
            r#"
            INSERT INTO vertex_ai_memory.developer_workspaces (
                workspace_id, name, root, git_enabled, branch, document, registered_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, now())
            ON CONFLICT (workspace_id) DO UPDATE SET
                name = EXCLUDED.name, root = EXCLUDED.root,
                git_enabled = EXCLUDED.git_enabled, branch = EXCLUDED.branch,
                document = EXCLUDED.document, updated_at = now()
            "#,
        )
        .bind(workspace.id)
        .bind(&workspace.name)
        .bind(&workspace.root)
        .bind(workspace.git_enabled)
        .bind(&workspace.branch)
        .bind(serde_json::to_value(workspace)?)
        .bind(workspace.registered_at)
        .execute(&self.pool)
        .await
        .map_err(|error| DeveloperError::Store(error.to_string()))?;
        Ok(())
    }

    async fn save_task(&self, task: &DeveloperTask) -> Result<(), DeveloperError> {
        sqlx::query(
            r#"
            INSERT INTO vertex_ai_memory.development_tasks (
                task_id, workspace_id, request, mode, state, model, risk,
                confidence, document, created_at, updated_at, completed_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (task_id) DO UPDATE SET
                state = EXCLUDED.state, risk = EXCLUDED.risk,
                confidence = EXCLUDED.confidence, document = EXCLUDED.document,
                updated_at = EXCLUDED.updated_at, completed_at = EXCLUDED.completed_at
            "#,
        )
        .bind(task.id)
        .bind(task.workspace_id)
        .bind(&task.request)
        .bind(format!("{:?}", task.mode).to_ascii_uppercase())
        .bind(format!("{:?}", task.state).to_ascii_uppercase())
        .bind(&task.model)
        .bind(format!("{:?}", task.risk).to_ascii_uppercase())
        .bind(task.confidence)
        .bind(serde_json::to_value(task)?)
        .bind(task.created_at)
        .bind(task.updated_at)
        .bind(task.completed_at)
        .execute(&self.pool)
        .await
        .map_err(|error| DeveloperError::Store(error.to_string()))?;
        Ok(())
    }

    async fn append_event(
        &self,
        task_id: DeveloperTaskId,
        sequence: u64,
        event_type: &str,
        payload: Value,
    ) -> Result<(), DeveloperError> {
        let sequence = i64::try_from(sequence)
            .map_err(|_| DeveloperError::Store("event sequence overflow".to_owned()))?;
        sqlx::query(
            r#"
            INSERT INTO vertex_ai_memory.development_events (
                event_id, task_id, sequence, event_type, payload
            ) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (task_id, sequence) DO UPDATE SET
                event_type = EXCLUDED.event_type, payload = EXCLUDED.payload
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(task_id)
        .bind(sequence)
        .bind(event_type)
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(|error| DeveloperError::Store(error.to_string()))?;
        Ok(())
    }

    async fn load_task(
        &self,
        task_id: DeveloperTaskId,
    ) -> Result<Option<DeveloperTask>, DeveloperError> {
        let document = sqlx::query_scalar::<_, Value>(
            "SELECT document FROM vertex_ai_memory.development_tasks WHERE task_id = $1",
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| DeveloperError::Store(error.to_string()))?;
        document
            .map(serde_json::from_value)
            .transpose()
            .map_err(Into::into)
    }

    async fn list_tasks(&self, limit: usize) -> Result<Vec<DeveloperTask>, DeveloperError> {
        let limit = i64::try_from(limit.clamp(1, 500)).unwrap_or(100);
        let documents = sqlx::query_scalar::<_, Value>(
            "SELECT document FROM vertex_ai_memory.development_tasks ORDER BY updated_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| DeveloperError::Store(error.to_string()))?;
        documents
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn persist_atomically(path: &Path, bytes: &[u8]) -> Result<(), DeveloperError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let next = path.with_extension("next");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&next)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(next, path)?;
    Ok(())
}
