//! Vertex-managed private PostgreSQL runtime for Vertex Memory Core.
//!
//! Runtime binaries are immutable installer resources. Cluster data and the
//! non-secret runtime manifest live under the application's durable data root.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Connection, PgConnection, postgres::PgConnectOptions};
use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use tokio::{net::TcpListener, process::Command, sync::RwLock, time::sleep};
use tracing::{info, warn};
use vertex_ai_runtime::{
    ManagedRuntimeSnapshot, ManagedRuntimeState, ManagedServiceRuntime, RuntimeDiagnosis,
    RuntimeError,
};
use vertex_ai_secrets::{SecretId, SecretStore, SecretValue};

pub const MANAGED_POSTGRES_VERSION: &str = "18.4";
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const DATABASE_NAME: &str = "vertex_ai";
pub const APPLICATION_ROLE: &str = "vertex_ai_app";
const BOOTSTRAP_ROLE: &str = "vertex_ai_bootstrap";
const MAX_START_ATTEMPTS: usize = 2;

#[derive(Debug, Error)]
pub enum ManagedPostgresError {
    #[error("managed PostgreSQL runtime is missing: {0}")]
    RuntimeMissing(String),
    #[error("managed PostgreSQL runtime I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("managed PostgreSQL manifest is invalid: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("managed PostgreSQL credential failed: {0}")]
    Credential(String),
    #[error("managed PostgreSQL command failed: {0}")]
    Command(String),
    #[error("managed PostgreSQL database failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("managed PostgreSQL requires repair: {0}")]
    RepairRequired(String),
}

#[derive(Debug, Clone)]
pub struct ManagedPostgresPaths {
    pub runtime_root: PathBuf,
    pub data_root: PathBuf,
}

impl ManagedPostgresPaths {
    pub fn cluster_dir(&self) -> PathBuf {
        self.data_root.join("Cluster")
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.data_root.join("runtime-v1.json")
    }

    fn logs_dir(&self) -> PathBuf {
        self.data_root.join("Logs")
    }

    fn postgres_exe(&self) -> PathBuf {
        self.runtime_root.join("bin").join("postgres.exe")
    }

    fn initdb_exe(&self) -> PathBuf {
        self.runtime_root.join("bin").join("initdb.exe")
    }

    fn pg_ctl_exe(&self) -> PathBuf {
        self.runtime_root.join("bin").join("pg_ctl.exe")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeManifest {
    schema_version: u32,
    cluster_id: String,
    runtime_version: String,
    host: String,
    port: u16,
    database: String,
    application_role: String,
    cluster_initialized: bool,
    database_initialized: bool,
    last_successful_start: Option<DateTime<Utc>>,
    last_error: Option<String>,
    backup_state: String,
}

impl RuntimeManifest {
    fn fresh(port: u16) -> Self {
        Self {
            schema_version: MANIFEST_SCHEMA_VERSION,
            cluster_id: format!("vertex-memory-{}", Utc::now().timestamp_millis()),
            runtime_version: MANAGED_POSTGRES_VERSION.to_owned(),
            host: Ipv4Addr::LOCALHOST.to_string(),
            port,
            database: DATABASE_NAME.to_owned(),
            application_role: APPLICATION_ROLE.to_owned(),
            cluster_initialized: false,
            database_initialized: false,
            last_successful_start: None,
            last_error: None,
            backup_state: "not_configured".to_owned(),
        }
    }
}

#[derive(Clone)]
pub struct ManagedPostgresRuntime {
    paths: ManagedPostgresPaths,
    secrets: Arc<dyn SecretStore>,
    manifest: Arc<RwLock<Option<RuntimeManifest>>>,
}

impl ManagedPostgresRuntime {
    pub fn new(paths: ManagedPostgresPaths, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            paths,
            secrets,
            manifest: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn ensure_ready(&self) -> Result<ManagedRuntimeSnapshot, ManagedPostgresError> {
        self.verify_runtime()?;
        fs::create_dir_all(&self.paths.data_root)?;
        fs::create_dir_all(self.paths.logs_dir())?;
        let mut manifest = self.load_or_create_manifest().await?;

        if !self.paths.cluster_dir().join("PG_VERSION").is_file() {
            manifest.cluster_initialized = false;
            manifest.database_initialized = false;
            self.persist_manifest(&manifest)?;
            self.initialize_cluster(&manifest).await?;
            manifest.cluster_initialized = true;
            self.persist_manifest(&manifest)?;
        } else {
            let cluster_version = fs::read_to_string(self.paths.cluster_dir().join("PG_VERSION"))?;
            let cluster_major = cluster_version.trim().split('.').next().unwrap_or_default();
            let runtime_major = MANAGED_POSTGRES_VERSION
                .split('.')
                .next()
                .unwrap_or_default();
            if cluster_major != runtime_major {
                return Err(ManagedPostgresError::RepairRequired(format!(
                    "cluster major version {cluster_major} is incompatible with runtime major version {runtime_major}; backup and pg_upgrade are required"
                )));
            }
            // Minor runtime replacement is safe with a stopped compatible
            // cluster. The manifest tracks the active binary version without
            // rewriting cluster data.
            manifest.runtime_version = MANAGED_POSTGRES_VERSION.to_owned();
            self.persist_manifest(&manifest)?;
        }

        // A persisted port can later be claimed by another application. Keep
        // the current port when it is our healthy server; otherwise select and
        // persist a new loopback-only port before starting PostgreSQL.
        if self.health_check(&manifest).await.is_err() && !port_is_available(manifest.port).await {
            manifest.port = select_available_port().await?;
            append_private_configuration(&self.paths.cluster_dir(), manifest.port)?;
            self.persist_manifest(&manifest)?;
        }

        let mut last_error = None;
        for attempt in 1..=MAX_START_ATTEMPTS {
            match self.start_server(&manifest).await {
                Ok(()) => {
                    last_error = None;
                    break;
                }
                Err(error) => {
                    last_error = Some(error.to_string());
                    warn!(attempt, error = %error, "Vertex Memory Core start attempt failed");
                    if attempt < MAX_START_ATTEMPTS {
                        sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        }
        if let Some(error) = last_error {
            manifest.last_error = Some(redact_error(&error));
            self.persist_manifest(&manifest)?;
            *self.manifest.write().await = Some(manifest);
            return Err(ManagedPostgresError::RepairRequired(error));
        }

        if !manifest.database_initialized {
            self.initialize_database(&manifest).await?;
            manifest.database_initialized = true;
        }
        self.health_check(&manifest).await?;
        manifest.last_successful_start = Some(Utc::now());
        manifest.last_error = None;
        self.persist_manifest(&manifest)?;
        *self.manifest.write().await = Some(manifest.clone());
        info!(port = manifest.port, "Vertex Memory Core is ready");
        self.snapshot(&manifest, ManagedRuntimeState::Ready).await
    }

    pub async fn application_connect_options(
        &self,
    ) -> Result<PgConnectOptions, ManagedPostgresError> {
        let manifest = self.current_manifest().await?;
        let password = self
            .secrets
            .get(&SecretId::memory_database_password())
            .await
            .map_err(|error| ManagedPostgresError::Credential(error.to_string()))?
            .ok_or_else(|| {
                ManagedPostgresError::Credential("Memory Core credential is missing".to_owned())
            })?;
        Ok(PgConnectOptions::new()
            .host(&manifest.host)
            .port(manifest.port)
            .username(&manifest.application_role)
            .password(password.expose())
            .database(&manifest.database))
    }

    async fn load_or_create_manifest(&self) -> Result<RuntimeManifest, ManagedPostgresError> {
        let path = self.paths.manifest_path();
        let manifest = if path.is_file() {
            let value: RuntimeManifest = serde_json::from_slice(&fs::read(&path)?)?;
            if value.schema_version != MANIFEST_SCHEMA_VERSION {
                return Err(ManagedPostgresError::RepairRequired(format!(
                    "unsupported runtime manifest schema {}",
                    value.schema_version
                )));
            }
            value
        } else {
            RuntimeManifest::fresh(select_available_port().await?)
        };
        self.persist_manifest(&manifest)?;
        *self.manifest.write().await = Some(manifest.clone());
        Ok(manifest)
    }

    async fn current_manifest(&self) -> Result<RuntimeManifest, ManagedPostgresError> {
        if let Some(manifest) = self.manifest.read().await.clone() {
            return Ok(manifest);
        }
        self.load_or_create_manifest().await
    }

    fn persist_manifest(&self, manifest: &RuntimeManifest) -> Result<(), ManagedPostgresError> {
        let path = self.paths.manifest_path();
        let next = path.with_extension("next");
        fs::create_dir_all(&self.paths.data_root)?;
        fs::write(&next, serde_json::to_vec_pretty(manifest)?)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        fs::rename(next, path)?;
        Ok(())
    }

    fn verify_runtime(&self) -> Result<(), ManagedPostgresError> {
        for required in [
            self.paths.postgres_exe(),
            self.paths.initdb_exe(),
            self.paths.pg_ctl_exe(),
            self.paths.runtime_root.join("share"),
        ] {
            if !required.exists() {
                return Err(ManagedPostgresError::RuntimeMissing(
                    required.to_string_lossy().into_owned(),
                ));
            }
        }
        Ok(())
    }

    async fn initialize_cluster(
        &self,
        manifest: &RuntimeManifest,
    ) -> Result<(), ManagedPostgresError> {
        let bootstrap = self
            .get_or_create_secret(SecretId::memory_bootstrap_password())
            .await?;
        let password_file = self.paths.data_root.join(".bootstrap-pw.tmp");
        fs::write(&password_file, bootstrap.expose().as_bytes())?;
        let result = run_checked(
            self.paths.initdb_exe(),
            &[
                "--pgdata".to_owned(),
                self.paths.cluster_dir().to_string_lossy().into_owned(),
                "--username".to_owned(),
                BOOTSTRAP_ROLE.to_owned(),
                "--pwfile".to_owned(),
                password_file.to_string_lossy().into_owned(),
                "--auth-host=scram-sha-256".to_owned(),
                "--auth-local=scram-sha-256".to_owned(),
                "--encoding=UTF8".to_owned(),
                "--locale=C".to_owned(),
            ],
            None,
        )
        .await;
        let _ = fs::remove_file(password_file);
        result?;
        append_private_configuration(&self.paths.cluster_dir(), manifest.port)?;
        Ok(())
    }

    async fn start_server(&self, manifest: &RuntimeManifest) -> Result<(), ManagedPostgresError> {
        if self.health_check(manifest).await.is_ok() {
            return Ok(());
        }
        run_detached_checked(
            self.paths.pg_ctl_exe(),
            &[
                "--pgdata".to_owned(),
                self.paths.cluster_dir().to_string_lossy().into_owned(),
                "--log".to_owned(),
                self.paths
                    .logs_dir()
                    .join("postgresql.log")
                    .to_string_lossy()
                    .into_owned(),
                "--wait".to_owned(),
                "--timeout=15".to_owned(),
                "start".to_owned(),
            ],
        )
        .await
    }

    async fn initialize_database(
        &self,
        manifest: &RuntimeManifest,
    ) -> Result<(), ManagedPostgresError> {
        let bootstrap = self
            .secrets
            .get(&SecretId::memory_bootstrap_password())
            .await
            .map_err(|error| ManagedPostgresError::Credential(error.to_string()))?
            .ok_or_else(|| {
                ManagedPostgresError::Credential("bootstrap credential missing".to_owned())
            })?;
        let application = self
            .get_or_create_secret(SecretId::memory_database_password())
            .await?;
        let options = PgConnectOptions::new()
            .host(&manifest.host)
            .port(manifest.port)
            .username(BOOTSTRAP_ROLE)
            .password(bootstrap.expose())
            .database("postgres");
        let mut connection = PgConnection::connect_with(&options).await?;
        let escaped = application.expose().replace('\'', "''");
        let create_role = format!(
            "DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{APPLICATION_ROLE}') THEN CREATE ROLE {APPLICATION_ROLE} LOGIN PASSWORD '{escaped}'; ELSE ALTER ROLE {APPLICATION_ROLE} PASSWORD '{escaped}'; END IF; END $$"
        );
        sqlx::query(&create_role).execute(&mut connection).await?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
                .bind(DATABASE_NAME)
                .fetch_one(&mut connection)
                .await?;
        if !exists {
            sqlx::query(&format!(
                "CREATE DATABASE {DATABASE_NAME} OWNER {APPLICATION_ROLE}"
            ))
            .execute(&mut connection)
            .await?;
        }
        Ok(())
    }

    async fn get_or_create_secret(
        &self,
        id: SecretId,
    ) -> Result<SecretValue, ManagedPostgresError> {
        if let Some(value) = self
            .secrets
            .get(&id)
            .await
            .map_err(|error| ManagedPostgresError::Credential(error.to_string()))?
        {
            return Ok(value);
        }
        let value = SecretValue::new(generate_password())
            .map_err(|error| ManagedPostgresError::Credential(error.to_string()))?;
        self.secrets
            .put(&id, value.clone())
            .await
            .map_err(|error| ManagedPostgresError::Credential(error.to_string()))?;
        Ok(value)
    }

    async fn health_check(&self, manifest: &RuntimeManifest) -> Result<(), ManagedPostgresError> {
        let bootstrap = self
            .secrets
            .get(&SecretId::memory_bootstrap_password())
            .await
            .map_err(|error| ManagedPostgresError::Credential(error.to_string()))?
            .ok_or_else(|| {
                ManagedPostgresError::Credential("bootstrap credential missing".to_owned())
            })?;
        let options = PgConnectOptions::new()
            .host(&manifest.host)
            .port(manifest.port)
            .username(BOOTSTRAP_ROLE)
            .password(bootstrap.expose())
            .database("postgres");
        let mut connection = PgConnection::connect_with(&options).await?;
        sqlx::query("SELECT 1").execute(&mut connection).await?;
        Ok(())
    }

    async fn stop_server(&self) -> Result<(), ManagedPostgresError> {
        if !self.paths.cluster_dir().join("PG_VERSION").is_file() {
            return Ok(());
        }
        run_checked(
            self.paths.pg_ctl_exe(),
            &[
                "--pgdata".to_owned(),
                self.paths.cluster_dir().to_string_lossy().into_owned(),
                "--wait".to_owned(),
                "--timeout=15".to_owned(),
                "stop".to_owned(),
                "--mode=fast".to_owned(),
            ],
            None,
        )
        .await
    }

    async fn snapshot(
        &self,
        manifest: &RuntimeManifest,
        state: ManagedRuntimeState,
    ) -> Result<ManagedRuntimeSnapshot, ManagedPostgresError> {
        let (database_size_bytes, connection_count, schema_version) = if state
            == ManagedRuntimeState::Ready
            && manifest.database_initialized
        {
            match self.application_connect_options().await {
                Ok(options) => match PgConnection::connect_with(&options).await {
                    Ok(mut connection) => {
                        let size = sqlx::query_scalar::<_, i64>(
                            "SELECT pg_database_size(current_database())",
                        )
                        .fetch_one(&mut connection)
                        .await
                        .ok()
                        .and_then(|value| u64::try_from(value).ok());
                        let count = sqlx::query_scalar::<_, i64>(
                                "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database()",
                            )
                            .fetch_one(&mut connection)
                            .await
                            .ok()
                            .and_then(|value| u32::try_from(value).ok());
                        let schema = sqlx::query_scalar::<_, i64>(
                            "SELECT COALESCE(max(version), 0) FROM _sqlx_migrations WHERE success",
                        )
                        .fetch_optional(&mut connection)
                        .await
                        .ok()
                        .flatten()
                        .map(|value| value.to_string());
                        (size, count, schema)
                    }
                    Err(_) => (None, None, None),
                },
                Err(_) => (None, None, None),
            }
        } else {
            (None, None, None)
        };
        Ok(ManagedRuntimeSnapshot {
            id: "vertex-memory-core".to_owned(),
            display_name: "Vertex Memory Core".to_owned(),
            state,
            version: Some(manifest.runtime_version.clone()),
            runtime_location: self.paths.runtime_root.to_string_lossy().into_owned(),
            data_location: self.paths.data_root.to_string_lossy().into_owned(),
            host: Some(manifest.host.clone()),
            port: Some(manifest.port),
            database: Some(manifest.database.clone()),
            schema_version,
            database_size_bytes,
            connection_count,
            backup_state: manifest.backup_state.clone(),
            last_successful_start: manifest.last_successful_start,
            last_error: manifest.last_error.clone(),
            observed_at: Utc::now(),
        })
    }
}

#[async_trait]
impl ManagedServiceRuntime for ManagedPostgresRuntime {
    fn id(&self) -> &str {
        "vertex-memory-core"
    }

    async fn inspect_managed(&self) -> Result<ManagedRuntimeSnapshot, RuntimeError> {
        let manifest = self.current_manifest().await.map_err(to_runtime_error)?;
        let state = if self.health_check(&manifest).await.is_ok() {
            ManagedRuntimeState::Ready
        } else if self.paths.cluster_dir().join("PG_VERSION").is_file() {
            ManagedRuntimeState::Stopped
        } else {
            ManagedRuntimeState::RepairRequired
        };
        self.snapshot(&manifest, state)
            .await
            .map_err(to_runtime_error)
    }

    async fn start(&self) -> Result<ManagedRuntimeSnapshot, RuntimeError> {
        self.ensure_ready().await.map_err(to_runtime_error)
    }

    async fn stop(&self) -> Result<ManagedRuntimeSnapshot, RuntimeError> {
        let manifest = self.current_manifest().await.map_err(to_runtime_error)?;
        self.stop_server().await.map_err(to_runtime_error)?;
        self.snapshot(&manifest, ManagedRuntimeState::Stopped)
            .await
            .map_err(to_runtime_error)
    }

    async fn diagnose(&self) -> Result<Vec<RuntimeDiagnosis>, RuntimeError> {
        let mut findings = Vec::new();
        if let Err(error) = self.verify_runtime() {
            findings.push(diagnosis("runtime_missing", error.to_string(), true, false));
            return Ok(findings);
        }
        if !self.paths.cluster_dir().join("PG_VERSION").is_file() {
            findings.push(diagnosis(
                "cluster_missing",
                "Vertex Memory cluster has not been initialized".to_owned(),
                true,
                false,
            ));
            return Ok(findings);
        }
        let manifest = match self.current_manifest().await {
            Ok(value) => value,
            Err(error) => {
                findings.push(diagnosis(
                    "manifest_invalid",
                    error.to_string(),
                    true,
                    false,
                ));
                return Ok(findings);
            }
        };
        if TcpListener::bind(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            manifest.port,
        ))
        .await
        .is_err()
            && self.health_check(&manifest).await.is_err()
        {
            findings.push(diagnosis(
                "port_conflict",
                format!(
                    "Managed port {} is occupied by another process",
                    manifest.port
                ),
                true,
                false,
            ));
        }
        if let Err(error) = self.health_check(&manifest).await {
            findings.push(diagnosis(
                "connection_failure",
                error.to_string(),
                true,
                false,
            ));
        }
        Ok(findings)
    }
}

fn diagnosis(code: &str, detail: String, repairable: bool, destructive: bool) -> RuntimeDiagnosis {
    RuntimeDiagnosis {
        code: code.to_owned(),
        summary: code.replace('_', " "),
        detail: redact_error(&detail),
        repairable,
        destructive,
    }
}

fn to_runtime_error(error: ManagedPostgresError) -> RuntimeError {
    match error {
        ManagedPostgresError::RuntimeMissing(message) => RuntimeError::Unavailable(message),
        other => RuntimeError::Failed(other.to_string()),
    }
}

async fn select_available_port() -> Result<u16, ManagedPostgresError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    Ok(listener.local_addr()?.port())
}

async fn port_is_available(port: u16) -> bool {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await.is_ok()
}

fn generate_password() -> String {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).expect("operating system random source must be available");
    let mut value = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut value, "{byte:02x}");
    }
    value
}

fn append_private_configuration(cluster: &Path, port: u16) -> Result<(), ManagedPostgresError> {
    let configuration = format!(
        "\n# Vertex AI managed configuration\nlisten_addresses = '127.0.0.1'\nport = {port}\npassword_encryption = 'scram-sha-256'\nmax_connections = 24\n"
    );
    use std::io::Write;
    fs::OpenOptions::new()
        .append(true)
        .open(cluster.join("postgresql.conf"))?
        .write_all(configuration.as_bytes())?;
    Ok(())
}

async fn run_checked(
    executable: PathBuf,
    arguments: &[String],
    environment: Option<(&str, &str)>,
) -> Result<(), ManagedPostgresError> {
    let mut command = Command::new(&executable);
    command.args(arguments).kill_on_drop(true);
    if let Some((name, value)) = environment {
        command.env(name, value);
    }
    let output = command.output().await?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(ManagedPostgresError::Command(format!(
        "{} exited with {}: {}",
        executable.display(),
        output.status,
        redact_error(stderr.trim())
    )))
}

/// `pg_ctl start` launches a long-lived child process. Capturing its standard
/// handles can keep the parent pipe open forever on Windows, so startup uses
/// null handles and relies on PostgreSQL's dedicated log file for details.
async fn run_detached_checked(
    executable: PathBuf,
    arguments: &[String],
) -> Result<(), ManagedPostgresError> {
    let status = Command::new(&executable)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .status()
        .await?;
    if status.success() {
        Ok(())
    } else {
        Err(ManagedPostgresError::Command(format!(
            "{} exited with {status}; inspect the managed PostgreSQL log",
            executable.display()
        )))
    }
}

fn redact_error(message: &str) -> String {
    let mut value = message.to_owned();
    for marker in ["password=", "PGPASSWORD="] {
        if let Some(index) = value.find(marker) {
            let tail = &value[index + marker.len()..];
            let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
            value.replace_range(
                index + marker.len()..index + marker.len() + end,
                "[REDACTED]",
            );
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use vertex_ai_secrets::InMemorySecretStore;

    #[test]
    fn generated_password_is_strong_and_log_safe() {
        let password = generate_password();
        assert_eq!(password.len(), 64);
        assert!(
            password
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        );
        assert_eq!(
            redact_error(&format!("password={password} next")),
            "password=[REDACTED] next"
        );
    }

    #[tokio::test]
    async fn missing_runtime_is_structurally_diagnosed() {
        let root = std::env::temp_dir().join(format!("vertex-pg-test-{}", uuid::Uuid::new_v4()));
        let runtime = ManagedPostgresRuntime::new(
            ManagedPostgresPaths {
                runtime_root: root.join("runtime"),
                data_root: root.join("data"),
            },
            InMemorySecretStore::shared(),
        );
        let findings = runtime.diagnose().await.expect("diagnosis succeeds");
        assert_eq!(findings[0].code, "runtime_missing");
        assert!(findings[0].repairable);
    }

    #[tokio::test]
    async fn selected_port_is_local_and_available() {
        let port = select_available_port().await.expect("port selected");
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .await
            .expect("selected port was released");
        drop(listener);
    }

    #[tokio::test]
    #[ignore = "requires VERTEX_AI_TEST_MANAGED_POSTGRES_RUNTIME"]
    async fn fresh_cluster_migrates_restarts_and_keeps_data() {
        use vertex_ai_memory::{
            CreateMemory, MemoryCategory, MemoryPrivacy, MemoryRepository, MemoryScope,
            PostgresMemoryRepository,
        };
        let runtime_root = std::env::var("VERTEX_AI_TEST_MANAGED_POSTGRES_RUNTIME")
            .expect("managed PostgreSQL runtime path must be configured");
        let data_root = std::env::temp_dir().join(format!(
            "vertex-managed-postgres-integration-{}",
            uuid::Uuid::new_v4()
        ));
        let runtime = ManagedPostgresRuntime::new(
            ManagedPostgresPaths {
                runtime_root: runtime_root.into(),
                data_root: data_root.clone(),
            },
            InMemorySecretStore::shared(),
        );
        let first = runtime.ensure_ready().await.expect("fresh runtime starts");
        assert_eq!(first.state, ManagedRuntimeState::Ready);
        let repository = PostgresMemoryRepository::connect_with_options(
            runtime.application_connect_options().await.unwrap(),
            2,
        )
        .await
        .expect("Memory repository connects");
        repository
            .migrate()
            .await
            .expect("schema migration succeeds");
        let stored = repository
            .create(CreateMemory {
                category: MemoryCategory::Knowledge,
                scope: MemoryScope::system(),
                owner_id: None,
                content: "managed runtime persistence probe".to_owned(),
                structured_content: serde_json::json!({}),
                priority: 0.5,
                confidence: 1.0,
                source: "integration-test".to_owned(),
                expires_at: None,
                privacy: MemoryPrivacy::default(),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("memory is stored");
        drop(repository);
        runtime.stop().await.expect("runtime stops cleanly");
        runtime.ensure_ready().await.expect("runtime restarts");
        let repository = PostgresMemoryRepository::connect_with_options(
            runtime.application_connect_options().await.unwrap(),
            2,
        )
        .await
        .expect("Memory repository reconnects");
        let loaded = repository
            .get(stored.memory_id, &MemoryScope::system())
            .await
            .expect("memory query succeeds");
        assert!(loaded.is_some(), "memory survives runtime restart");
        drop(repository);
        runtime.stop().await.expect("runtime stops after test");
        fs::remove_dir_all(data_root).expect("test data is removed");
    }
}
