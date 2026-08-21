//! Runtime-neutral model inventory, storage, discovery, and compatibility foundation.

use chrono::{DateTime, Utc};
use fs2::{available_space, total_space};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use thiserror::Error;
use uuid::Uuid;
use vertex_ai_types::{HealthState, InstalledLocalModel, LocalRuntimeSnapshot};
use walkdir::WalkDir;

const SCHEMA_VERSION: u32 = 1;
const MAX_DISCOVERY_DEPTH: usize = 12;

#[derive(Debug, Error)]
pub enum ModelManagementError {
    #[error("model management I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("model management serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("unsupported model registry schema version {0}")]
    UnsupportedSchema(u32),
    #[error("invalid storage location: {0}")]
    InvalidStorage(String),
    #[error("storage location was not found: {0}")]
    StorageNotFound(String),
    #[error("model was not found: {0}")]
    ModelNotFound(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    Coding,
    Reasoning,
    Review,
    General,
    ToolUse,
    StructuredOutput,
    LongContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelHealth {
    Ready,
    Unavailable,
    Missing,
    Invalid,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTrust {
    Discovered,
    Registered,
    Verified,
    Trusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    LocalStorage,
    Ollama,
    LmStudio,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCompatibilityState {
    Available,
    Compatible,
    Planned,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCompatibility {
    pub runtime_id: String,
    pub state: RuntimeCompatibilityState,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRecord {
    pub id: String,
    pub display_name: String,
    pub family: Option<String>,
    pub format: Option<String>,
    pub quantization: Option<String>,
    pub parameter_size: Option<String>,
    pub file_size: Option<u64>,
    pub storage_location_id: Option<String>,
    pub storage_path: Option<String>,
    pub runtime_compatibility: Vec<RuntimeCompatibility>,
    pub capabilities: BTreeSet<ModelCapability>,
    pub context_length: Option<u64>,
    pub local: bool,
    pub installed: bool,
    pub health: ModelHealth,
    pub trust: ModelTrust,
    pub source: ModelSource,
    pub source_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageAvailability {
    Available,
    Unavailable,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelStorageLocation {
    pub id: String,
    pub display_name: String,
    pub path: String,
    pub is_default: bool,
    pub availability: StorageAvailability,
    pub writable: bool,
    pub total_space: Option<u64>,
    pub free_space: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateModelCandidate {
    pub model_ids: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityState {
    Compatible,
    CompatibleWithOffload,
    ResourceConstrained,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityAssessment {
    pub model_id: String,
    pub state: CompatibilityState,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareSnapshot {
    pub system_ram_total: Option<u64>,
    pub system_ram_available: Option<u64>,
    pub gpu_vram_total: Option<u64>,
    pub gpu_vram_available: Option<u64>,
    pub gpu_vram_in_use: u64,
    pub storage_locations: Vec<ModelStorageLocation>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSelectionRequest {
    pub required_capabilities: BTreeSet<ModelCapability>,
    pub allowed_runtime_ids: BTreeSet<String>,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCandidate {
    pub model: ModelRecord,
    pub compatibility: CompatibilityAssessment,
    pub score: u32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMovePlan {
    pub model_id: String,
    pub source_path: String,
    pub destination_path: String,
    pub required_bytes: u64,
    pub verification: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelManagementSnapshot {
    pub models: Vec<ModelRecord>,
    pub storage_locations: Vec<ModelStorageLocation>,
    pub duplicates: Vec<DuplicateModelCandidate>,
    pub hardware: HardwareSnapshot,
    pub compatibility: Vec<CompatibilityAssessment>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryDocument {
    schema_version: u32,
    models: BTreeMap<String, ModelRecord>,
    storage_locations: BTreeMap<String, ModelStorageLocation>,
}

impl Default for RegistryDocument {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            models: BTreeMap::new(),
            storage_locations: BTreeMap::new(),
        }
    }
}

pub struct ModelManager {
    path: PathBuf,
    document: Mutex<RegistryDocument>,
}

impl ModelManager {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, ModelManagementError> {
        let path = path.into();
        recover_interrupted_write(&path)?;
        let mut document = if path.exists() {
            let document: RegistryDocument = serde_json::from_slice(&fs::read(&path)?)?;
            if document.schema_version != SCHEMA_VERSION {
                return Err(ModelManagementError::UnsupportedSchema(
                    document.schema_version,
                ));
            }
            document
        } else {
            RegistryDocument::default()
        };
        refresh_storage_records(&mut document);
        persist_document(&path, &document)?;
        Ok(Self {
            path,
            document: Mutex::new(document),
        })
    }

    pub fn list_models(&self) -> Vec<ModelRecord> {
        self.document
            .lock()
            .expect("model registry lock poisoned")
            .models
            .values()
            .cloned()
            .collect()
    }

    pub fn get_model(&self, id: &str) -> Result<ModelRecord, ModelManagementError> {
        self.document
            .lock()
            .expect("model registry lock poisoned")
            .models
            .get(id)
            .cloned()
            .ok_or_else(|| ModelManagementError::ModelNotFound(id.to_owned()))
    }

    pub fn upsert_model(
        &self,
        mut model: ModelRecord,
    ) -> Result<ModelRecord, ModelManagementError> {
        let mut document = self.document.lock().expect("model registry lock poisoned");
        let now = Utc::now();
        if let Some(existing) = document.models.get(&model.id) {
            model.created_at = existing.created_at;
        }
        model.updated_at = now;
        document.models.insert(model.id.clone(), model.clone());
        persist_document(&self.path, &document)?;
        Ok(model)
    }

    pub fn remove_model(&self, id: &str) -> Result<ModelRecord, ModelManagementError> {
        let mut document = self.document.lock().expect("model registry lock poisoned");
        let model = document
            .models
            .remove(id)
            .ok_or_else(|| ModelManagementError::ModelNotFound(id.to_owned()))?;
        persist_document(&self.path, &document)?;
        Ok(model)
    }

    pub fn storage_locations(&self) -> Vec<ModelStorageLocation> {
        self.document
            .lock()
            .expect("model registry lock poisoned")
            .storage_locations
            .values()
            .cloned()
            .collect()
    }

    pub fn add_storage_location(
        &self,
        display_name: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<ModelStorageLocation, ModelManagementError> {
        let canonical = validate_storage_path(path.as_ref())?;
        let normalized = normalized_path(&canonical);
        let mut document = self.document.lock().expect("model registry lock poisoned");
        for existing in document.storage_locations.values() {
            let existing_path = PathBuf::from(&existing.path);
            if normalized_path(&existing_path) == normalized {
                return Err(ModelManagementError::InvalidStorage(
                    "同じ保存先は既に登録されています".to_owned(),
                ));
            }
            if paths_overlap(&existing_path, &canonical) {
                return Err(ModelManagementError::InvalidStorage(format!(
                    "親子関係にある保存先は重複登録できません: {}",
                    existing.path
                )));
            }
        }
        let now = Utc::now();
        let is_default = document.storage_locations.is_empty();
        let location = ModelStorageLocation {
            id: format!("storage:{}", Uuid::new_v4()),
            display_name: non_blank_or(display_name.into(), "モデル保存先"),
            path: canonical.to_string_lossy().into_owned(),
            is_default,
            availability: StorageAvailability::Available,
            writable: true,
            total_space: total_space(&canonical).ok(),
            free_space: available_space(&canonical).ok(),
            created_at: now,
            updated_at: now,
        };
        document
            .storage_locations
            .insert(location.id.clone(), location.clone());
        persist_document(&self.path, &document)?;
        Ok(location)
    }

    pub fn set_default_storage(
        &self,
        id: &str,
    ) -> Result<ModelStorageLocation, ModelManagementError> {
        let mut document = self.document.lock().expect("model registry lock poisoned");
        if !document.storage_locations.contains_key(id) {
            return Err(ModelManagementError::StorageNotFound(id.to_owned()));
        }
        for location in document.storage_locations.values_mut() {
            location.is_default = location.id == id;
            location.updated_at = Utc::now();
        }
        let selected = document.storage_locations[id].clone();
        persist_document(&self.path, &document)?;
        Ok(selected)
    }

    pub fn refresh_storage_locations(
        &self,
    ) -> Result<Vec<ModelStorageLocation>, ModelManagementError> {
        let mut document = self.document.lock().expect("model registry lock poisoned");
        refresh_storage_records(&mut document);
        persist_document(&self.path, &document)?;
        Ok(document.storage_locations.values().cloned().collect())
    }

    pub fn discover_all(&self) -> Result<Vec<ModelRecord>, ModelManagementError> {
        let storage_ids = self
            .storage_locations()
            .into_iter()
            .filter(|location| location.availability == StorageAvailability::Available)
            .map(|location| location.id)
            .collect::<Vec<_>>();
        let mut discovered = Vec::new();
        for storage_id in storage_ids {
            discovered.extend(self.discover_storage(&storage_id)?);
        }
        Ok(discovered)
    }

    pub fn discover_storage(
        &self,
        storage_id: &str,
    ) -> Result<Vec<ModelRecord>, ModelManagementError> {
        let location = self
            .storage_locations()
            .into_iter()
            .find(|location| location.id == storage_id)
            .ok_or_else(|| ModelManagementError::StorageNotFound(storage_id.to_owned()))?;
        if location.availability != StorageAvailability::Available {
            return Err(ModelManagementError::InvalidStorage(
                "保存先は現在利用できません".to_owned(),
            ));
        }
        let root = PathBuf::from(&location.path);
        let mut discovered = Vec::new();
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .max_depth(MAX_DISCOVERY_DEPTH)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.path();
            if !path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("gguf"))
            {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            discovered.push(local_model_record(&location, path, metadata.len()));
        }
        let mut document = self.document.lock().expect("model registry lock poisoned");
        document.models.retain(|_, model| {
            model.source != ModelSource::LocalStorage
                || model.storage_location_id.as_deref() != Some(storage_id)
        });
        for model in &discovered {
            document.models.insert(model.id.clone(), model.clone());
        }
        persist_document(&self.path, &document)?;
        Ok(discovered)
    }

    pub fn ingest_runtime_snapshot(
        &self,
        snapshot: &LocalRuntimeSnapshot,
    ) -> Result<Vec<ModelRecord>, ModelManagementError> {
        let runtime_id = snapshot.provider_id.as_str();
        let source = if runtime_id.eq_ignore_ascii_case("ollama") {
            ModelSource::Ollama
        } else if runtime_id.eq_ignore_ascii_case("lm-studio") {
            ModelSource::LmStudio
        } else {
            ModelSource::Remote
        };
        let now = Utc::now();
        let models = snapshot
            .installed_models
            .iter()
            .map(|model| runtime_model_record(snapshot, model, source, now))
            .collect::<Vec<_>>();
        let mut document = self.document.lock().expect("model registry lock poisoned");
        let prefix = format!("{runtime_id}:");
        if snapshot.health != HealthState::Ready {
            for model in document
                .models
                .values_mut()
                .filter(|model| model.source == source && model.source_key.starts_with(&prefix))
            {
                model.health = ModelHealth::Unavailable;
                model.updated_at = now;
            }
            persist_document(&self.path, &document)?;
            return Ok(document
                .models
                .values()
                .filter(|model| model.source == source && model.source_key.starts_with(&prefix))
                .cloned()
                .collect());
        }
        document
            .models
            .retain(|_, model| model.source != source || !model.source_key.starts_with(&prefix));
        for model in &models {
            document.models.insert(model.id.clone(), model.clone());
        }
        persist_document(&self.path, &document)?;
        Ok(models)
    }

    pub fn duplicates(&self) -> Vec<DuplicateModelCandidate> {
        let models = self.list_models();
        let mut groups: BTreeMap<(String, Option<u64>), Vec<&ModelRecord>> = BTreeMap::new();
        for model in &models {
            let filename = model
                .storage_path
                .as_deref()
                .and_then(|path| Path::new(path).file_name())
                .and_then(|name| name.to_str())
                .unwrap_or(&model.display_name)
                .to_lowercase();
            groups
                .entry((filename, model.file_size))
                .or_default()
                .push(model);
        }
        groups
            .into_iter()
            .filter(|(_, models)| models.len() > 1)
            .map(|((filename, size), models)| DuplicateModelCandidate {
                model_ids: models.iter().map(|model| model.id.clone()).collect(),
                evidence: vec![
                    format!("filename={filename}"),
                    format!(
                        "file_size={}",
                        size.map_or_else(|| "unknown".to_owned(), |v| v.to_string())
                    ),
                ],
            })
            .collect()
    }

    pub fn hardware_snapshot(&self, runtimes: &[LocalRuntimeSnapshot]) -> HardwareSnapshot {
        let (system_ram_total, system_ram_available) = system_memory();
        let gpu_vram_in_use = runtimes
            .iter()
            .flat_map(|runtime| &runtime.loaded_models)
            .map(|model| model.size_vram_bytes)
            .sum();
        HardwareSnapshot {
            system_ram_total,
            system_ram_available,
            gpu_vram_total: None,
            gpu_vram_available: None,
            gpu_vram_in_use,
            storage_locations: self.storage_locations(),
            observed_at: Utc::now(),
        }
    }

    pub fn assess(
        &self,
        model: &ModelRecord,
        hardware: &HardwareSnapshot,
    ) -> CompatibilityAssessment {
        let mut reasons = Vec::new();
        let state = if !model.installed || model.health == ModelHealth::Missing {
            reasons.push("モデルが利用可能な保存先またはRuntimeに存在しません".to_owned());
            CompatibilityState::Unsupported
        } else if model.health != ModelHealth::Ready {
            reasons.push("モデルの状態がReadyではありません".to_owned());
            CompatibilityState::Unknown
        } else if let (Some(size), Some(available)) = (model.file_size, hardware.gpu_vram_available)
        {
            if size <= available {
                reasons.push(format!("モデル {size} bytes は利用可能VRAM内です"));
                CompatibilityState::Compatible
            } else if hardware.system_ram_available.is_some_and(|ram| size <= ram) {
                reasons.push("VRAM単独では不足しますがRAM Offload候補です".to_owned());
                CompatibilityState::CompatibleWithOffload
            } else {
                reasons.push("推定必要量が利用可能VRAM/RAMを超えます".to_owned());
                CompatibilityState::ResourceConstrained
            }
        } else if let (Some(size), Some(ram)) = (model.file_size, hardware.system_ram_available) {
            if size <= ram {
                reasons.push("GPU総量は未検出ですがSystem RAMには収まります".to_owned());
                CompatibilityState::CompatibleWithOffload
            } else {
                reasons.push("モデルサイズが利用可能System RAMを超えます".to_owned());
                CompatibilityState::ResourceConstrained
            }
        } else {
            reasons.push("判定に必要なモデルサイズまたはHardware情報が不足しています".to_owned());
            CompatibilityState::Unknown
        };
        CompatibilityAssessment {
            model_id: model.id.clone(),
            state,
            reasons,
        }
    }

    pub fn rank_candidates(
        &self,
        request: &ModelSelectionRequest,
        hardware: &HardwareSnapshot,
    ) -> Vec<ModelCandidate> {
        let mut candidates = self
            .list_models()
            .into_iter()
            .filter(|model| !request.local_only || model.local)
            .filter(|model| request.required_capabilities.is_subset(&model.capabilities))
            .filter(|model| {
                request.allowed_runtime_ids.is_empty()
                    || model.runtime_compatibility.iter().any(|runtime| {
                        request.allowed_runtime_ids.contains(&runtime.runtime_id)
                            && matches!(
                                runtime.state,
                                RuntimeCompatibilityState::Available
                                    | RuntimeCompatibilityState::Compatible
                            )
                    })
            })
            .filter_map(|model| {
                let compatibility = self.assess(&model, hardware);
                if compatibility.state == CompatibilityState::Unsupported {
                    return None;
                }
                let capability_score = model.capabilities.len() as u32 * 10;
                let compatibility_score = match compatibility.state {
                    CompatibilityState::Compatible => 50,
                    CompatibilityState::CompatibleWithOffload => 35,
                    CompatibilityState::Unknown => 15,
                    CompatibilityState::ResourceConstrained => 5,
                    CompatibilityState::Unsupported => 0,
                };
                Some(ModelCandidate {
                    reasons: compatibility.reasons.clone(),
                    score: capability_score + compatibility_score,
                    model,
                    compatibility,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.model.id.cmp(&right.model.id))
        });
        candidates
    }

    pub fn plan_move(
        &self,
        model_id: &str,
        destination_storage_id: &str,
    ) -> Result<ModelMovePlan, ModelManagementError> {
        let model = self.get_model(model_id)?;
        let source_path = model.storage_path.ok_or_else(|| {
            ModelManagementError::InvalidStorage("モデルの実ファイル位置が不明です".to_owned())
        })?;
        let destination = self
            .storage_locations()
            .into_iter()
            .find(|location| location.id == destination_storage_id)
            .ok_or_else(|| {
                ModelManagementError::StorageNotFound(destination_storage_id.to_owned())
            })?;
        let required_bytes = model.file_size.unwrap_or_default();
        if destination.availability != StorageAvailability::Available
            || destination
                .free_space
                .is_some_and(|space| space < required_bytes)
        {
            return Err(ModelManagementError::InvalidStorage(
                "移動先が利用不能または空き容量不足です".to_owned(),
            ));
        }
        let file_name = Path::new(&source_path)
            .file_name()
            .map(ToOwned::to_owned)
            .ok_or_else(|| ModelManagementError::InvalidStorage("モデル名が無効です".to_owned()))?;
        Ok(ModelMovePlan {
            model_id: model_id.to_owned(),
            source_path,
            destination_path: Path::new(&destination.path)
                .join(file_name)
                .to_string_lossy()
                .into_owned(),
            required_bytes,
            verification: vec![
                "コピー完了後にFile Sizeを照合".to_owned(),
                "Registry更新後に旧ファイルを削除".to_owned(),
                "失敗時はRegistryを変更しない".to_owned(),
            ],
        })
    }

    pub fn snapshot(&self, runtimes: &[LocalRuntimeSnapshot]) -> ModelManagementSnapshot {
        let hardware = self.hardware_snapshot(runtimes);
        let models = self.list_models();
        let compatibility = models
            .iter()
            .map(|model| self.assess(model, &hardware))
            .collect();
        ModelManagementSnapshot {
            models,
            storage_locations: self.storage_locations(),
            duplicates: self.duplicates(),
            hardware,
            compatibility,
            observed_at: Utc::now(),
        }
    }
}

fn local_model_record(location: &ModelStorageLocation, path: &Path, size: u64) -> ModelRecord {
    let now = Utc::now();
    let filename = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("GGUF Model");
    let normalized = normalized_path(path);
    let (family, parameter_size, quantization) = infer_filename_metadata(filename);
    ModelRecord {
        id: format!("local:{normalized}"),
        display_name: filename.replace(['_', '-'], " "),
        family,
        format: Some("GGUF".to_owned()),
        quantization,
        parameter_size,
        file_size: Some(size),
        storage_location_id: Some(location.id.clone()),
        storage_path: Some(path.to_string_lossy().into_owned()),
        runtime_compatibility: vec![
            RuntimeCompatibility {
                runtime_id: "vertex-built-in".to_owned(),
                state: RuntimeCompatibilityState::Planned,
                reason: "Built-in Runtimeは次工程で実装予定".to_owned(),
            },
            RuntimeCompatibility {
                runtime_id: "lm-studio".to_owned(),
                state: RuntimeCompatibilityState::Unknown,
                reason: "GGUF互換性はRuntime側確認が必要".to_owned(),
            },
        ],
        capabilities: infer_capabilities(filename, None),
        context_length: None,
        local: true,
        installed: true,
        health: ModelHealth::Ready,
        trust: ModelTrust::Discovered,
        source: ModelSource::LocalStorage,
        source_key: normalized,
        created_at: now,
        updated_at: now,
    }
}

fn runtime_model_record(
    runtime: &LocalRuntimeSnapshot,
    model: &InstalledLocalModel,
    source: ModelSource,
    now: DateTime<Utc>,
) -> ModelRecord {
    let runtime_id = runtime.provider_id.as_str();
    let source_key = format!("{runtime_id}:{}", model.reference.model_id.as_str());
    let mut capabilities = infer_capabilities(&model.display_name, model.family.as_deref());
    if model
        .context_length
        .is_some_and(|context| context >= 32_768)
    {
        capabilities.insert(ModelCapability::LongContext);
    }
    ModelRecord {
        id: source_key.clone(),
        display_name: model.display_name.clone(),
        family: model.family.clone(),
        format: model.format.clone(),
        quantization: model.quantization_level.clone(),
        parameter_size: model.parameter_size.clone(),
        file_size: Some(model.size_bytes),
        storage_location_id: None,
        storage_path: runtime.model_storage_path.clone(),
        runtime_compatibility: vec![
            RuntimeCompatibility {
                runtime_id: runtime_id.to_owned(),
                state: if runtime.health == HealthState::Ready {
                    RuntimeCompatibilityState::Available
                } else {
                    RuntimeCompatibilityState::Unknown
                },
                reason: format!("{} Discovery Adapterで確認", runtime.display_name),
            },
            RuntimeCompatibility {
                runtime_id: "vertex-built-in".to_owned(),
                state: RuntimeCompatibilityState::Planned,
                reason: "Built-in Runtimeは未実装".to_owned(),
            },
        ],
        capabilities,
        context_length: model.context_length,
        local: true,
        installed: true,
        health: if runtime.health == HealthState::Ready {
            ModelHealth::Ready
        } else {
            ModelHealth::Unavailable
        },
        trust: if model.digest.is_some() {
            ModelTrust::Verified
        } else {
            ModelTrust::Registered
        },
        source,
        source_key,
        created_at: now,
        updated_at: now,
    }
}

fn infer_capabilities(name: &str, family: Option<&str>) -> BTreeSet<ModelCapability> {
    let text = format!("{} {}", name, family.unwrap_or_default()).to_lowercase();
    let mut capabilities = BTreeSet::from([ModelCapability::General]);
    if ["coder", "code", "qwen", "deepseek", "codestral"]
        .iter()
        .any(|needle| text.contains(needle))
    {
        capabilities.extend([
            ModelCapability::Coding,
            ModelCapability::Reasoning,
            ModelCapability::Review,
        ]);
    }
    if ["qwen", "llama", "mistral", "deepseek", "gemma"]
        .iter()
        .any(|needle| text.contains(needle))
    {
        capabilities.extend([ModelCapability::ToolUse, ModelCapability::StructuredOutput]);
    }
    capabilities
}

fn infer_filename_metadata(name: &str) -> (Option<String>, Option<String>, Option<String>) {
    let lower = name.to_lowercase();
    let family = ["qwen", "llama", "mistral", "deepseek", "gemma", "phi"]
        .iter()
        .find(|family| lower.contains(**family))
        .map(|family| (*family).to_owned());
    let parameter_size = lower
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '.')
        .find(|part| {
            part.ends_with('b') && part[..part.len().saturating_sub(1)].parse::<f32>().is_ok()
        })
        .map(str::to_uppercase);
    let quantization = ["q2_k", "q3_k", "q4_k", "q5_k", "q6_k", "q8_0", "f16"]
        .iter()
        .find(|quantization| lower.contains(**quantization))
        .map(|quantization| quantization.to_uppercase());
    (family, parameter_size, quantization)
}

fn validate_storage_path(path: &Path) -> Result<PathBuf, ModelManagementError> {
    if path.as_os_str().is_empty() {
        return Err(ModelManagementError::InvalidStorage(
            "パスが空です".to_owned(),
        ));
    }
    if !path.exists() || !path.is_dir() {
        return Err(ModelManagementError::InvalidStorage(
            "存在するフォルダを指定してください".to_owned(),
        ));
    }
    let canonical = fs::canonicalize(path)?;
    let probe = canonical.join(format!(".vertex-write-probe-{}", Uuid::new_v4()));
    let result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .and_then(|mut file| file.write_all(b"vertex-model-storage-probe"));
    let _ = fs::remove_file(&probe);
    result
        .map_err(|_| ModelManagementError::InvalidStorage("保存先へ書き込めません".to_owned()))?;
    Ok(canonical)
}

fn refresh_storage_records(document: &mut RegistryDocument) {
    let now = Utc::now();
    for location in document.storage_locations.values_mut() {
        let path = Path::new(&location.path);
        if !path.exists() {
            location.availability = StorageAvailability::Missing;
            location.writable = false;
            location.total_space = None;
            location.free_space = None;
        } else if !path.is_dir() {
            location.availability = StorageAvailability::Unavailable;
            location.writable = false;
            location.total_space = None;
            location.free_space = None;
        } else {
            location.availability = StorageAvailability::Available;
            location.writable = true;
            location.total_space = total_space(path).ok();
            location.free_space = available_space(path).ok();
        }
        location.updated_at = now;
    }
    let availability = document
        .storage_locations
        .iter()
        .map(|(id, location)| (id.clone(), location.availability))
        .collect::<BTreeMap<_, _>>();
    for model in document.models.values_mut() {
        if let Some(storage_id) = &model.storage_location_id
            && availability.get(storage_id) != Some(&StorageAvailability::Available)
        {
            model.health = ModelHealth::Missing;
            model.updated_at = now;
        }
    }
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    let left = normalized_path(left);
    let right = normalized_path(right);
    let separator = std::path::MAIN_SEPARATOR;
    left.starts_with(&format!("{right}{separator}"))
        || right.starts_with(&format!("{left}{separator}"))
}

fn normalized_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        value.trim_end_matches('\\').to_lowercase()
    } else {
        value.trim_end_matches('\\').to_owned()
    }
}

fn non_blank_or(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_owned()
    } else {
        value.trim().to_owned()
    }
}

#[cfg(windows)]
fn system_memory() -> (Option<u64>, Option<u64>) {
    use std::mem::size_of;
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    // SAFETY: Windows requires a valid writable MEMORYSTATUSEX with dwLength initialized.
    if unsafe { GlobalMemoryStatusEx(&mut status) } != 0 {
        (Some(status.ullTotalPhys), Some(status.ullAvailPhys))
    } else {
        (None, None)
    }
}

#[cfg(not(windows))]
fn system_memory() -> (Option<u64>, Option<u64>) {
    (None, None)
}

fn persist_document(path: &Path, document: &RegistryDocument) -> Result<(), ModelManagementError> {
    let bytes = serde_json::to_vec_pretty(document)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let next = path.with_extension("json.next");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&next)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(next, path)?;
    Ok(())
}

fn recover_interrupted_write(path: &Path) -> Result<(), ModelManagementError> {
    let next = path.with_extension("json.next");
    if !path.exists() && next.exists() {
        fs::rename(next, path)?;
    } else if next.exists() {
        fs::remove_file(next)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vertex_ai_types::{ModelId, ModelRef, ProviderId};

    fn open_manager(root: &Path) -> ModelManager {
        ModelManager::open(root.join("registry.json")).unwrap()
    }

    fn runtime_model(name: &str, size: u64) -> InstalledLocalModel {
        InstalledLocalModel {
            reference: ModelRef::new(
                ProviderId::new("ollama").unwrap(),
                ModelId::new(name).unwrap(),
            ),
            display_name: name.to_owned(),
            size_bytes: size,
            digest: Some("sha256:test".to_owned()),
            format: Some("gguf".to_owned()),
            family: Some("qwen3".to_owned()),
            parameter_size: Some("8B".to_owned()),
            quantization_level: Some("Q4_K_M".to_owned()),
            context_length: Some(32_768),
            modified_at: None,
        }
    }

    fn runtime(models: Vec<InstalledLocalModel>) -> LocalRuntimeSnapshot {
        LocalRuntimeSnapshot {
            provider_id: ProviderId::new("ollama").unwrap(),
            display_name: "Ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            health: HealthState::Ready,
            version: Some("test".to_owned()),
            executable_path: None,
            model_storage_path: Some("D:/Ollama/models".to_owned()),
            storage_total_bytes: None,
            storage_available_bytes: None,
            installed_models: models,
            loaded_models: Vec::new(),
            checked_at: Utc::now(),
        }
    }

    #[test]
    fn storage_crud_default_validation_and_persistence() {
        let root = tempdir().unwrap();
        let first_dir = root.path().join("models-a");
        let second_dir = root.path().join("models-b");
        fs::create_dir_all(&first_dir).unwrap();
        fs::create_dir_all(&second_dir).unwrap();
        let manager = open_manager(root.path());
        let first = manager.add_storage_location("A", &first_dir).unwrap();
        let second = manager.add_storage_location("B", &second_dir).unwrap();
        assert!(first.is_default);
        assert!(!second.is_default);
        assert!(
            manager
                .add_storage_location("duplicate", &first_dir)
                .is_err()
        );
        let selected = manager.set_default_storage(&second.id).unwrap();
        assert!(selected.is_default);
        drop(manager);
        let reopened = open_manager(root.path());
        assert_eq!(reopened.storage_locations().len(), 2);
        assert!(
            reopened
                .storage_locations()
                .iter()
                .any(|item| item.id == second.id && item.is_default)
        );
    }

    #[test]
    fn invalid_and_overlapping_storage_is_rejected() {
        let root = tempdir().unwrap();
        let manager = open_manager(root.path());
        assert!(
            manager
                .add_storage_location("missing", root.path().join("missing"))
                .is_err()
        );
        let parent = root.path().join("models");
        let child = parent.join("nested");
        fs::create_dir_all(&child).unwrap();
        manager.add_storage_location("parent", &parent).unwrap();
        assert!(manager.add_storage_location("child", &child).is_err());
    }

    #[test]
    fn unavailable_storage_and_models_survive_recovery() {
        let root = tempdir().unwrap();
        let storage = root.path().join("external");
        fs::create_dir_all(&storage).unwrap();
        fs::write(storage.join("qwen-8b-q4_k.gguf"), b"gguf").unwrap();
        let manager = open_manager(root.path());
        manager.add_storage_location("External", &storage).unwrap();
        assert_eq!(manager.discover_all().unwrap().len(), 1);
        fs::remove_dir_all(&storage).unwrap();
        drop(manager);
        let reopened = open_manager(root.path());
        assert_eq!(reopened.list_models().len(), 1);
        assert_eq!(reopened.list_models()[0].health, ModelHealth::Missing);
        assert_eq!(
            reopened.storage_locations()[0].availability,
            StorageAvailability::Missing
        );
    }

    #[test]
    fn gguf_discovery_metadata_and_duplicate_detection_are_deterministic() {
        let root = tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        fs::write(first.join("qwen-8b-q4_k.gguf"), b"same").unwrap();
        fs::write(second.join("qwen-8b-q4_k.gguf"), b"same").unwrap();
        fs::write(first.join("ignore.bin"), b"same").unwrap();
        let manager = open_manager(root.path());
        manager.add_storage_location("First", &first).unwrap();
        manager.add_storage_location("Second", &second).unwrap();
        let models = manager.discover_all().unwrap();
        assert_eq!(models.len(), 2);
        assert!(
            models
                .iter()
                .all(|model| model.format.as_deref() == Some("GGUF"))
        );
        assert!(
            models
                .iter()
                .all(|model| model.capabilities.contains(&ModelCapability::Coding))
        );
        assert_eq!(manager.duplicates().len(), 1);
    }

    #[test]
    fn ollama_adapter_reconciles_inventory_and_preserves_typed_metadata() {
        let root = tempdir().unwrap();
        let manager = open_manager(root.path());
        let models = manager
            .ingest_runtime_snapshot(&runtime(vec![runtime_model("qwen3:8b", 8_000)]))
            .unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].source, ModelSource::Ollama);
        assert!(
            models[0]
                .capabilities
                .contains(&ModelCapability::StructuredOutput)
        );
        manager
            .ingest_runtime_snapshot(&runtime(Vec::new()))
            .unwrap();
        assert!(manager.list_models().is_empty());
    }

    #[test]
    fn model_crud_compatibility_and_auto_candidate_boundary_work() {
        let root = tempdir().unwrap();
        let manager = open_manager(root.path());
        let model = manager
            .ingest_runtime_snapshot(&runtime(vec![runtime_model("qwen3:8b", 8_000)]))
            .unwrap()
            .remove(0);
        let hardware = HardwareSnapshot {
            system_ram_total: Some(32_000),
            system_ram_available: Some(16_000),
            gpu_vram_total: Some(12_000),
            gpu_vram_available: Some(10_000),
            gpu_vram_in_use: 2_000,
            storage_locations: Vec::new(),
            observed_at: Utc::now(),
        };
        assert_eq!(
            manager.assess(&model, &hardware).state,
            CompatibilityState::Compatible
        );
        let request = ModelSelectionRequest {
            required_capabilities: BTreeSet::from([ModelCapability::Coding]),
            allowed_runtime_ids: BTreeSet::from(["ollama".to_owned()]),
            local_only: true,
        };
        assert_eq!(manager.rank_candidates(&request, &hardware).len(), 1);
        let removed = manager.remove_model(&model.id).unwrap();
        assert_eq!(removed.id, model.id);
        assert!(manager.get_model(&model.id).is_err());
    }

    #[test]
    fn move_foundation_preflights_without_mutating_files() {
        let root = tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&destination).unwrap();
        let file = source.join("model.gguf");
        fs::write(&file, b"model").unwrap();
        let manager = open_manager(root.path());
        manager.add_storage_location("Source", &source).unwrap();
        let destination = manager
            .add_storage_location("Destination", &destination)
            .unwrap();
        let model = manager.discover_all().unwrap().remove(0);
        let plan = manager.plan_move(&model.id, &destination.id).unwrap();
        assert_eq!(plan.required_bytes, 5);
        assert!(file.exists());
        assert!(!Path::new(&plan.destination_path).exists());
    }
}
