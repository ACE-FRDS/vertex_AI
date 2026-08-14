//! Deterministic, read-only discovery for the Vertex Environment Explorer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;
use vertex_ai_types::{
    AssetCategory, AssetKind, EvidenceRef, HealthState, RelationshipKind, SystemAsset,
    SystemAssetId, SystemRelationship,
};

#[derive(Debug, Error)]
pub enum EnvironmentError {
    #[error("environment path contains no searchable directories")]
    EmptySearchPath,
    #[error("environment index I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("environment index serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("unsupported environment index schema version {0}")]
    UnsupportedSchema(u32),
}

pub const ENVIRONMENT_INDEX_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDetector {
    pub name: &'static str,
    pub category: AssetCategory,
    pub kind: AssetKind,
    pub executable_names: &'static [&'static str],
    pub capabilities: &'static [&'static str],
}

impl ToolDetector {
    pub const fn new(
        name: &'static str,
        category: AssetCategory,
        kind: AssetKind,
        executable_names: &'static [&'static str],
        capabilities: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            category,
            kind,
            executable_names,
            capabilities,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    pub scanned_at: DateTime<Utc>,
    pub roots_scanned: Vec<String>,
    pub assets: Vec<SystemAsset>,
    pub relationships: Vec<SystemRelationship>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EnvironmentDelta {
    pub added: Vec<SystemAssetId>,
    pub updated: Vec<SystemAssetId>,
    pub removed: Vec<SystemAssetId>,
    pub relationships_changed: bool,
}

impl EnvironmentDelta {
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty()
            || !self.updated.is_empty()
            || !self.removed.is_empty()
            || self.relationships_changed
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedEnvironmentSnapshot {
    pub snapshot: EnvironmentSnapshot,
    pub delta: Option<EnvironmentDelta>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct EnvironmentIndexDocument {
    schema_version: u32,
    snapshot: Option<EnvironmentSnapshot>,
}

#[derive(Debug)]
pub struct PersistentEnvironmentIndex {
    path: PathBuf,
    document: EnvironmentIndexDocument,
}

impl PersistentEnvironmentIndex {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, EnvironmentError> {
        let path = path.into();
        recover_interrupted_write(&path)?;
        let document = if path.exists() {
            let document: EnvironmentIndexDocument = serde_json::from_slice(&fs::read(&path)?)?;
            if document.schema_version != ENVIRONMENT_INDEX_SCHEMA_VERSION {
                return Err(EnvironmentError::UnsupportedSchema(document.schema_version));
            }
            document
        } else {
            EnvironmentIndexDocument {
                schema_version: ENVIRONMENT_INDEX_SCHEMA_VERSION,
                snapshot: None,
            }
        };
        Ok(Self { path, document })
    }

    pub fn current(&self) -> Option<&EnvironmentSnapshot> {
        self.document.snapshot.as_ref()
    }

    pub fn update(
        &mut self,
        snapshot: EnvironmentSnapshot,
    ) -> Result<EnvironmentDelta, EnvironmentError> {
        let delta = diff_snapshots(self.document.snapshot.as_ref(), &snapshot);
        let next = EnvironmentIndexDocument {
            schema_version: ENVIRONMENT_INDEX_SCHEMA_VERSION,
            snapshot: Some(snapshot),
        };
        persist_atomically(&self.path, &serde_json::to_vec_pretty(&next)?)?;
        self.document = next;
        Ok(delta)
    }
}

impl EnvironmentSnapshot {
    pub fn search(&self, query: &str) -> Vec<&SystemAsset> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return self.assets.iter().collect();
        }
        self.assets
            .iter()
            .filter(|asset| {
                asset.name.to_lowercase().contains(&query)
                    || asset
                        .location
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&query)
                    || asset
                        .capabilities
                        .iter()
                        .any(|capability| capability.to_lowercase().contains(&query))
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct EnvironmentScanner {
    detectors: Vec<ToolDetector>,
}

impl Default for EnvironmentScanner {
    fn default() -> Self {
        Self::new(default_detectors())
    }
}

impl EnvironmentScanner {
    pub fn new(detectors: Vec<ToolDetector>) -> Self {
        Self { detectors }
    }

    /// Performs a bounded PATH scan only. It never executes discovered programs or mutates the host.
    pub fn scan_path(
        &self,
        path_override: Option<&OsStr>,
    ) -> Result<EnvironmentSnapshot, EnvironmentError> {
        let path_value = path_override
            .map(ToOwned::to_owned)
            .or_else(|| env::var_os("PATH"));
        let Some(path_value) = path_value else {
            return Err(EnvironmentError::EmptySearchPath);
        };
        let roots: Vec<PathBuf> = env::split_paths(&path_value)
            .filter(|path| !path.as_os_str().is_empty())
            .collect();
        if roots.is_empty() {
            return Err(EnvironmentError::EmptySearchPath);
        }

        let observed_at = Utc::now();
        let mut seen = BTreeSet::new();
        let mut assets = Vec::new();
        let mut relationships = Vec::new();

        for detector in &self.detectors {
            for root in &roots {
                for executable_name in detector.executable_names {
                    let candidate = root.join(executable_name);
                    if !candidate.is_file() {
                        continue;
                    }
                    let resolved = fs::canonicalize(&candidate).unwrap_or(candidate);
                    let normalized = normalize_path(&resolved);
                    if !seen.insert(normalized.clone()) {
                        continue;
                    }
                    let id = SystemAssetId::new(format!("executable:{normalized}"))
                        .expect("normalized executable path is non-empty");
                    let capabilities: Vec<String> = detector
                        .capabilities
                        .iter()
                        .map(|value| (*value).to_owned())
                        .collect();
                    for capability in &capabilities {
                        relationships.push(SystemRelationship {
                            source_id: id.to_string(),
                            target_id: format!("capability:{capability}"),
                            kind: RelationshipKind::Provides,
                            verified: true,
                        });
                    }
                    assets.push(SystemAsset {
                        id,
                        name: detector.name.to_owned(),
                        category: detector.category,
                        kind: detector.kind,
                        location: Some(resolved.to_string_lossy().into_owned()),
                        version: None,
                        architecture: None,
                        health: HealthState::Ready,
                        capabilities,
                        evidence: vec![EvidenceRef {
                            source: "path_scan".to_owned(),
                            locator: resolved.to_string_lossy().into_owned(),
                            observed_at,
                            content_hash: None,
                            metadata: BTreeMap::new(),
                        }],
                        observed_at,
                        metadata: BTreeMap::new(),
                    });
                }
            }
        }

        assets.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(EnvironmentSnapshot {
            scanned_at: observed_at,
            roots_scanned: roots
                .iter()
                .map(|root| root.to_string_lossy().into_owned())
                .collect(),
            assets,
            relationships,
        })
    }
}

fn normalize_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

fn diff_snapshots(
    previous: Option<&EnvironmentSnapshot>,
    next: &EnvironmentSnapshot,
) -> EnvironmentDelta {
    let previous_assets: BTreeMap<&SystemAssetId, &SystemAsset> = previous
        .map(|snapshot| {
            snapshot
                .assets
                .iter()
                .map(|asset| (&asset.id, asset))
                .collect()
        })
        .unwrap_or_default();
    let next_assets: BTreeMap<&SystemAssetId, &SystemAsset> =
        next.assets.iter().map(|asset| (&asset.id, asset)).collect();
    let added = next_assets
        .keys()
        .filter(|id| !previous_assets.contains_key(*id))
        .map(|id| (*id).clone())
        .collect();
    let removed = previous_assets
        .keys()
        .filter(|id| !next_assets.contains_key(*id))
        .map(|id| (*id).clone())
        .collect();
    let updated = next_assets
        .iter()
        .filter_map(|(id, asset)| {
            previous_assets
                .get(id)
                .filter(|previous| !assets_materially_equal(previous, asset))
                .map(|_| (*id).clone())
        })
        .collect();
    EnvironmentDelta {
        added,
        updated,
        removed,
        relationships_changed: previous
            .map(|snapshot| snapshot.relationships != next.relationships)
            .unwrap_or(!next.relationships.is_empty()),
    }
}

fn assets_materially_equal(left: &SystemAsset, right: &SystemAsset) -> bool {
    left.id == right.id
        && left.name == right.name
        && left.category == right.category
        && left.kind == right.kind
        && left.location == right.location
        && left.version == right.version
        && left.architecture == right.architecture
        && left.health == right.health
        && left.capabilities == right.capabilities
        && left.metadata == right.metadata
}

fn next_path(path: &Path) -> PathBuf {
    path.with_extension("next")
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("backup")
}

fn recover_interrupted_write(path: &Path) -> Result<(), std::io::Error> {
    let backup = backup_path(path);
    if !path.exists() && backup.exists() {
        fs::rename(backup, path)?;
    }
    Ok(())
}

fn persist_atomically(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let next = next_path(path);
    let backup = backup_path(path);
    if next.exists() {
        fs::remove_file(&next)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&next)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);

    let had_current = path.exists();
    if had_current {
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        fs::rename(path, &backup)?;
    }
    if let Err(error) = fs::rename(&next, path) {
        if had_current && backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(error);
    }
    if backup.exists() {
        fs::remove_file(backup)?;
    }
    Ok(())
}

fn default_detectors() -> Vec<ToolDetector> {
    vec![
        ToolDetector::new(
            "Python",
            AssetCategory::Developer,
            AssetKind::Runtime,
            &["python.exe", "python3.exe", "python", "python3"],
            &["development.python", "runtime.python"],
        ),
        ToolDetector::new(
            "Node.js",
            AssetCategory::Developer,
            AssetKind::Runtime,
            &["node.exe", "node"],
            &["development.javascript", "runtime.nodejs"],
        ),
        ToolDetector::new(
            "Rust",
            AssetCategory::Developer,
            AssetKind::Sdk,
            &["rustc.exe", "rustc"],
            &["development.rust"],
        ),
        ToolDetector::new(
            "Cargo",
            AssetCategory::Developer,
            AssetKind::Executable,
            &["cargo.exe", "cargo"],
            &["development.rust.package_management"],
        ),
        ToolDetector::new(
            "Git",
            AssetCategory::Developer,
            AssetKind::Executable,
            &["git.exe", "git"],
            &["development.version_control"],
        ),
        ToolDetector::new(
            "Docker",
            AssetCategory::Runtime,
            AssetKind::Runtime,
            &["docker.exe", "docker"],
            &["runtime.containers"],
        ),
        ToolDetector::new(
            "Ollama",
            AssetCategory::Ai,
            AssetKind::Runtime,
            &["ollama.exe", "ollama"],
            &["ai.local_inference"],
        ),
        ToolDetector::new(
            "Visual Studio Code",
            AssetCategory::Developer,
            AssetKind::Application,
            &["code.exe", "code"],
            &["development.source_editing"],
        ),
        ToolDetector::new(
            "ffmpeg",
            AssetCategory::Creator,
            AssetKind::Executable,
            &["ffmpeg.exe", "ffmpeg"],
            &["creator.media_conversion", "creator.transcoding"],
        ),
        ToolDetector::new(
            "Blender",
            AssetCategory::Creator,
            AssetKind::Application,
            &["blender.exe", "blender"],
            &["creator.3d_modeling", "creator.rendering"],
        ),
        ToolDetector::new(
            "PostgreSQL Client",
            AssetCategory::Database,
            AssetKind::Executable,
            &["psql.exe", "psql"],
            &["database.postgresql.client"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root() -> PathBuf {
        std::env::temp_dir().join(format!("vertex-environment-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn path_scan_produces_verified_asset_and_capability_relationship() {
        let root = temporary_root();
        fs::create_dir_all(&root).expect("creates test root");
        let executable = root.join("vertex-probe.exe");
        fs::write(&executable, b"test").expect("creates test executable");
        let detector = ToolDetector::new(
            "Vertex Probe",
            AssetCategory::Developer,
            AssetKind::Executable,
            &["vertex-probe.exe"],
            &["development.vertex_probe"],
        );
        let scanner = EnvironmentScanner::new(vec![detector]);
        let snapshot = scanner
            .scan_path(Some(root.as_os_str()))
            .expect("scan succeeds");
        assert_eq!(snapshot.assets.len(), 1);
        assert_eq!(snapshot.assets[0].health, HealthState::Ready);
        assert_eq!(snapshot.assets[0].evidence[0].source, "path_scan");
        assert_eq!(snapshot.relationships.len(), 1);
        assert!(snapshot.relationships[0].verified);
        fs::remove_dir_all(&root).expect("removes test root");
    }

    #[test]
    fn semantic_metadata_search_matches_capabilities() {
        let root = temporary_root();
        fs::create_dir_all(&root).expect("creates test root");
        fs::write(root.join("media-tool.exe"), b"test").expect("creates test executable");
        let scanner = EnvironmentScanner::new(vec![ToolDetector::new(
            "Media Tool",
            AssetCategory::Creator,
            AssetKind::Executable,
            &["media-tool.exe"],
            &["creator.transcoding"],
        )]);
        let snapshot = scanner
            .scan_path(Some(root.as_os_str()))
            .expect("scan succeeds");
        assert_eq!(snapshot.search("transcoding").len(), 1);
        assert!(snapshot.search("database").is_empty());
        fs::remove_dir_all(&root).expect("removes test root");
    }

    #[test]
    fn empty_search_path_is_rejected() {
        let scanner = EnvironmentScanner::new(Vec::new());
        assert!(matches!(
            scanner.scan_path(Some(OsStr::new(""))),
            Err(EnvironmentError::EmptySearchPath)
        ));
    }

    #[test]
    fn persistent_index_round_trips_and_ignores_observation_time_only_changes() {
        let root = temporary_root();
        fs::create_dir_all(&root).expect("creates test root");
        fs::write(root.join("vertex-probe.exe"), b"test").expect("creates test executable");
        let scanner = EnvironmentScanner::new(vec![ToolDetector::new(
            "Vertex Probe",
            AssetCategory::Developer,
            AssetKind::Executable,
            &["vertex-probe.exe"],
            &["development.vertex_probe"],
        )]);
        let index_path = root.join("index").join("environment.json");
        let first = scanner
            .scan_path(Some(root.as_os_str()))
            .expect("first scan");
        let mut index = PersistentEnvironmentIndex::open(&index_path).expect("opens index");
        let first_delta = index.update(first).expect("stores first snapshot");
        assert_eq!(first_delta.added.len(), 1);

        let second = scanner
            .scan_path(Some(root.as_os_str()))
            .expect("second scan");
        let second_delta = index.update(second).expect("stores second snapshot");
        assert!(!second_delta.has_changes());

        let reopened = PersistentEnvironmentIndex::open(&index_path).expect("reopens index");
        assert_eq!(reopened.current().expect("cached snapshot").assets.len(), 1);
        fs::remove_dir_all(&root).expect("removes test root");
    }

    #[test]
    fn persistent_index_reports_removed_assets() {
        let root = temporary_root();
        fs::create_dir_all(&root).expect("creates test root");
        let executable = root.join("vertex-probe.exe");
        fs::write(&executable, b"test").expect("creates test executable");
        let scanner = EnvironmentScanner::new(vec![ToolDetector::new(
            "Vertex Probe",
            AssetCategory::Developer,
            AssetKind::Executable,
            &["vertex-probe.exe"],
            &["development.vertex_probe"],
        )]);
        let mut index =
            PersistentEnvironmentIndex::open(root.join("environment.json")).expect("opens index");
        index
            .update(
                scanner
                    .scan_path(Some(root.as_os_str()))
                    .expect("first scan"),
            )
            .expect("stores first snapshot");
        fs::remove_file(executable).expect("removes test executable");
        let delta = index
            .update(
                scanner
                    .scan_path(Some(root.as_os_str()))
                    .expect("second scan"),
            )
            .expect("stores second snapshot");
        assert_eq!(delta.removed.len(), 1);
        fs::remove_dir_all(&root).expect("removes test root");
    }
}
