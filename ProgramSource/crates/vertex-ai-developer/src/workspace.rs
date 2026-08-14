use crate::{DeveloperError, FileChange, FileChangeKind, TextReplacement, Workspace, WorkspaceId};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use similar::TextDiff;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};
use uuid::Uuid;
use walkdir::WalkDir;

const REGISTRY_SCHEMA_VERSION: u32 = 1;
const MAX_READ_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SEARCH_FILES: usize = 20_000;
const MAX_SEARCH_RESULTS: usize = 250;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryDocument {
    schema_version: u32,
    workspaces: Vec<Workspace>,
}

#[derive(Debug)]
pub struct WorkspaceRegistry {
    path: PathBuf,
    workspaces: RwLock<BTreeMap<WorkspaceId, Workspace>>,
}

impl WorkspaceRegistry {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, DeveloperError> {
        let path = path.into();
        let workspaces = if path.is_file() {
            let document: RegistryDocument = serde_json::from_slice(&fs::read(&path)?)?;
            if document.schema_version != REGISTRY_SCHEMA_VERSION {
                return Err(DeveloperError::Invalid(format!(
                    "unsupported workspace registry schema {}",
                    document.schema_version
                )));
            }
            document
                .workspaces
                .into_iter()
                .map(|workspace| (workspace.id, workspace))
                .collect()
        } else {
            BTreeMap::new()
        };
        Ok(Self {
            path,
            workspaces: RwLock::new(workspaces),
        })
    }

    pub fn register(
        &self,
        name: impl Into<String>,
        root: impl AsRef<Path>,
    ) -> Result<Workspace, DeveloperError> {
        let name = name.into().trim().to_owned();
        if name.is_empty() {
            return Err(DeveloperError::Invalid(
                "workspace name is blank".to_owned(),
            ));
        }
        let canonical = fs::canonicalize(root.as_ref())?;
        if !canonical.is_dir() {
            return Err(DeveloperError::Invalid(
                "workspace root is not a directory".to_owned(),
            ));
        }
        let normalized = normalize(&canonical);
        let mut values = self
            .workspaces
            .write()
            .map_err(|_| DeveloperError::Store("workspace registry lock failed".to_owned()))?;
        if let Some(existing) = values.values_mut().find(|value| value.root == normalized) {
            existing.name = name;
            existing.last_opened_at = Utc::now();
            let result = existing.clone();
            drop(values);
            self.persist()?;
            return Ok(result);
        }
        let now = Utc::now();
        let git_enabled = canonical.join(".git").exists();
        let workspace = Workspace {
            id: Uuid::new_v4(),
            name,
            root: normalized,
            git_enabled,
            branch: git_enabled.then(|| read_git_branch(&canonical)).flatten(),
            registered_at: now,
            last_opened_at: now,
        };
        values.insert(workspace.id, workspace.clone());
        drop(values);
        self.persist()?;
        Ok(workspace)
    }

    pub fn get(&self, id: WorkspaceId) -> Result<Workspace, DeveloperError> {
        self.workspaces
            .read()
            .map_err(|_| DeveloperError::Store("workspace registry lock failed".to_owned()))?
            .get(&id)
            .cloned()
            .ok_or_else(|| DeveloperError::NotFound(format!("workspace {id}")))
    }

    pub fn list(&self) -> Result<Vec<Workspace>, DeveloperError> {
        Ok(self
            .workspaces
            .read()
            .map_err(|_| DeveloperError::Store("workspace registry lock failed".to_owned()))?
            .values()
            .cloned()
            .collect())
    }

    fn persist(&self) -> Result<(), DeveloperError> {
        let values = self
            .workspaces
            .read()
            .map_err(|_| DeveloperError::Store("workspace registry lock failed".to_owned()))?
            .values()
            .cloned()
            .collect();
        persist_atomically(
            &self.path,
            &serde_json::to_vec_pretty(&RegistryDocument {
                schema_version: REGISTRY_SCHEMA_VERSION,
                workspaces: values,
            })?,
        )
    }
}

#[derive(Clone)]
pub struct FileToolkit {
    workspace: Workspace,
    root: Arc<PathBuf>,
    originals: Arc<Mutex<BTreeMap<String, Option<Vec<u8>>>>>,
}

impl FileToolkit {
    pub fn new(workspace: Workspace) -> Result<Self, DeveloperError> {
        let root = fs::canonicalize(&workspace.root)?;
        Ok(Self {
            workspace,
            root: Arc::new(root),
            originals: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn list_directory(&self, relative: &str) -> Result<String, DeveloperError> {
        let path = self.resolve_existing(relative)?;
        if !path.is_dir() {
            return Err(DeveloperError::Invalid(
                "path is not a directory".to_owned(),
            ));
        }
        let mut entries = fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|entry| {
                let kind = if entry.path().is_dir() {
                    "directory"
                } else {
                    "file"
                };
                format!("{kind}\t{}", entry.file_name().to_string_lossy())
            })
            .collect::<Vec<_>>();
        entries.sort();
        Ok(entries.join("\n"))
    }

    pub fn read_file(&self, relative: &str) -> Result<String, DeveloperError> {
        let path = self.resolve_existing(relative)?;
        ensure_safe_read(&path)?;
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_READ_BYTES {
            return Err(DeveloperError::Invalid(format!(
                "file exceeds {} byte read limit",
                MAX_READ_BYTES
            )));
        }
        String::from_utf8(fs::read(path)?)
            .map_err(|_| DeveloperError::Invalid("file is not UTF-8 text".to_owned()))
    }

    pub fn read_file_range(
        &self,
        relative: &str,
        start_line: usize,
        end_line: usize,
    ) -> Result<String, DeveloperError> {
        if start_line == 0 || end_line < start_line || end_line - start_line > 1_000 {
            return Err(DeveloperError::Invalid("invalid line range".to_owned()));
        }
        let content = self.read_file(relative)?;
        Ok(content
            .lines()
            .enumerate()
            .filter(|(index, _)| (*index + 1) >= start_line && (*index + 1) <= end_line)
            .map(|(index, line)| format!("{}: {}", index + 1, line))
            .collect::<Vec<_>>()
            .join("\n"))
    }

    pub fn get_file_metadata(&self, relative: &str) -> Result<String, DeveloperError> {
        let path = self.resolve_existing(relative)?;
        let metadata = fs::metadata(&path)?;
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "path": self.relative_display(&path),
            "is_file": metadata.is_file(),
            "is_directory": metadata.is_dir(),
            "size_bytes": metadata.len(),
            "readonly": metadata.permissions().readonly(),
            "modified": metadata.modified().ok().map(chrono::DateTime::<Utc>::from),
        }))?)
    }

    pub fn project_tree(&self, depth: usize) -> Result<String, DeveloperError> {
        let depth = depth.clamp(1, 8);
        let mut lines = Vec::new();
        for entry in WalkDir::new(self.root.as_ref())
            .max_depth(depth)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| !ignored_directory(entry.path()))
            .filter_map(Result::ok)
            .take(MAX_SEARCH_FILES)
        {
            if entry.depth() == 0 {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(self.root.as_ref())
                .unwrap_or(entry.path());
            lines.push(format!(
                "{}{}{}",
                "  ".repeat(entry.depth() - 1),
                if entry.file_type().is_dir() {
                    "[D] "
                } else {
                    "[F] "
                },
                normalize(relative)
            ));
        }
        Ok(lines.join("\n"))
    }

    pub fn search_files(
        &self,
        query: &str,
        extension: Option<&str>,
        directory: Option<&str>,
    ) -> Result<String, DeveloperError> {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Err(DeveloperError::Invalid(
                "filename query is blank".to_owned(),
            ));
        }
        let start = self.resolve_existing(directory.unwrap_or("."))?;
        let extension = extension.map(|value| value.trim_start_matches('.').to_ascii_lowercase());
        let mut matches = Vec::new();
        for entry in safe_walk(&start).take(MAX_SEARCH_FILES) {
            let Some(name) = entry.file_name().to_str() else {
                continue;
            };
            if !name.to_ascii_lowercase().contains(&query) {
                continue;
            }
            if let Some(extension) = &extension
                && entry
                    .path()
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_ref()
                    != Some(extension)
            {
                continue;
            }
            matches.push(self.relative_display(entry.path()));
            if matches.len() >= MAX_SEARCH_RESULTS {
                break;
            }
        }
        Ok(matches.join("\n"))
    }

    pub fn search_text(
        &self,
        query: &str,
        extension: Option<&str>,
        directory: Option<&str>,
    ) -> Result<String, DeveloperError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(DeveloperError::Invalid("text query is blank".to_owned()));
        }
        let start = self.resolve_existing(directory.unwrap_or("."))?;
        let extension = extension.map(|value| value.trim_start_matches('.').to_ascii_lowercase());
        let mut matches = Vec::new();
        for entry in safe_walk(&start).take(MAX_SEARCH_FILES) {
            let path = entry.path();
            if is_secret_path(path) {
                continue;
            }
            if let Some(extension) = &extension
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_ref()
                    != Some(extension)
            {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.len() > MAX_READ_BYTES {
                continue;
            }
            let Ok(bytes) = fs::read(path) else { continue };
            if bytes.contains(&0) {
                continue;
            }
            let Ok(content) = String::from_utf8(bytes) else {
                continue;
            };
            for (line_index, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&query.to_lowercase()) {
                    matches.push(format!(
                        "{}:{}:{}",
                        self.relative_display(path),
                        line_index + 1,
                        truncate(line, 300)
                    ));
                    if matches.len() >= MAX_SEARCH_RESULTS {
                        return Ok(matches.join("\n"));
                    }
                }
            }
        }
        Ok(matches.join("\n"))
    }

    pub fn create_file(&self, relative: &str, content: &str) -> Result<(), DeveloperError> {
        let path = self.resolve_write(relative)?;
        if path.exists() {
            return Err(DeveloperError::Invalid(
                "target file already exists".to_owned(),
            ));
        }
        self.capture_original(relative, &path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content.as_bytes())?;
        Ok(())
    }

    pub fn write_file(&self, relative: &str, content: &str) -> Result<(), DeveloperError> {
        let path = self.resolve_write(relative)?;
        self.capture_original(relative, &path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content.as_bytes())?;
        Ok(())
    }

    pub fn apply_patch(&self, replacements: &[TextReplacement]) -> Result<(), DeveloperError> {
        if replacements.is_empty() || replacements.len() > 50 {
            return Err(DeveloperError::Invalid(
                "patch replacement count is invalid".to_owned(),
            ));
        }
        let mut grouped: BTreeMap<&str, Vec<&TextReplacement>> = BTreeMap::new();
        for replacement in replacements {
            grouped
                .entry(&replacement.path)
                .or_default()
                .push(replacement);
        }
        for (relative, edits) in grouped {
            let path = self.resolve_existing(relative)?;
            ensure_safe_read(&path)?;
            let mut content = self.read_file(relative)?;
            self.capture_original(relative, &path)?;
            for edit in edits {
                if edit.expected.is_empty() {
                    return Err(DeveloperError::Invalid(
                        "patch expected text is blank".to_owned(),
                    ));
                }
                let count = content.matches(&edit.expected).count();
                if count == 0 || (!edit.replace_all && count != 1) {
                    return Err(DeveloperError::Invalid(format!(
                        "patch precondition failed for {relative}: expected occurrence count {}, found {count}",
                        if edit.replace_all { ">=1" } else { "1" }
                    )));
                }
                content = if edit.replace_all {
                    content.replace(&edit.expected, &edit.replacement)
                } else {
                    content.replacen(&edit.expected, &edit.replacement, 1)
                };
            }
            fs::write(path, content.as_bytes())?;
        }
        Ok(())
    }

    pub fn delete_file(&self, relative: &str) -> Result<(), DeveloperError> {
        let path = self.resolve_existing(relative)?;
        if !path.is_file() {
            return Err(DeveloperError::Permission(
                "directory deletion is not supported".to_owned(),
            ));
        }
        self.capture_original(relative, &path)?;
        fs::remove_file(path)?;
        Ok(())
    }

    pub fn unified_diff(&self) -> Result<String, DeveloperError> {
        let originals = self
            .originals
            .lock()
            .map_err(|_| DeveloperError::Store("file snapshot lock failed".to_owned()))?;
        let mut diff = String::new();
        for (relative, original) in originals.iter() {
            let path = self.resolve_write(relative)?;
            let current = fs::read(&path).ok();
            let old = original.as_deref().unwrap_or_default();
            let new = current.as_deref().unwrap_or_default();
            if old == new {
                continue;
            }
            match (
                String::from_utf8(old.to_vec()),
                String::from_utf8(new.to_vec()),
            ) {
                (Ok(old_text), Ok(new_text)) => {
                    diff.push_str(
                        &TextDiff::from_lines(&old_text, &new_text)
                            .unified_diff()
                            .context_radius(3)
                            .header(&format!("a/{relative}"), &format!("b/{relative}"))
                            .to_string(),
                    );
                }
                _ => diff.push_str(&format!("Binary file changed: {relative}\n")),
            }
        }
        Ok(diff)
    }

    pub fn file_changes(&self, reason: &str) -> Result<Vec<FileChange>, DeveloperError> {
        let originals = self
            .originals
            .lock()
            .map_err(|_| DeveloperError::Store("file snapshot lock failed".to_owned()))?;
        let mut changes = Vec::new();
        for (relative, original) in originals.iter() {
            let path = self.resolve_write(relative)?;
            let current = fs::read(&path).ok();
            if current.as_ref() == original.as_ref() {
                continue;
            }
            let (kind, old_lines, new_lines) = match (original, &current) {
                (None, Some(bytes)) => (FileChangeKind::Created, 0, line_count(bytes)),
                (Some(bytes), None) => (FileChangeKind::Deleted, line_count(bytes), 0),
                (Some(old), Some(new)) => {
                    (FileChangeKind::Modified, line_count(old), line_count(new))
                }
                (None, None) => continue,
            };
            changes.push(FileChange {
                path: relative.clone(),
                kind,
                additions: new_lines.saturating_sub(old_lines),
                deletions: old_lines.saturating_sub(new_lines),
                reason: reason.to_owned(),
            });
        }
        Ok(changes)
    }

    pub fn rollback(&self) -> Result<(), DeveloperError> {
        let originals = self
            .originals
            .lock()
            .map_err(|_| DeveloperError::Store("file snapshot lock failed".to_owned()))?;
        for (relative, original) in originals.iter() {
            let path = self.resolve_write(relative)?;
            match original {
                Some(bytes) => {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(path, bytes)?;
                }
                None if path.is_file() => fs::remove_file(path)?,
                None => {}
            }
        }
        Ok(())
    }

    pub fn resolve_working_directory(&self, relative: &str) -> Result<PathBuf, DeveloperError> {
        let path = self.resolve_existing(relative)?;
        if !path.is_dir() {
            return Err(DeveloperError::Invalid(
                "working directory is not a directory".to_owned(),
            ));
        }
        Ok(path)
    }

    fn resolve_existing(&self, relative: &str) -> Result<PathBuf, DeveloperError> {
        let candidate = self.resolve_write(relative)?;
        let canonical = fs::canonicalize(&candidate)?;
        self.ensure_contained(&canonical)?;
        Ok(canonical)
    }

    fn resolve_write(&self, relative: &str) -> Result<PathBuf, DeveloperError> {
        let path = Path::new(relative);
        if path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(DeveloperError::Sandbox(
                "absolute paths and parent traversal are forbidden".to_owned(),
            ));
        }
        let candidate = self.root.join(path);
        let mut ancestor = candidate.as_path();
        while !ancestor.exists() {
            ancestor = ancestor.parent().ok_or_else(|| {
                DeveloperError::Sandbox("target has no workspace ancestor".to_owned())
            })?;
        }
        self.ensure_contained(&fs::canonicalize(ancestor)?)?;
        Ok(candidate)
    }

    fn ensure_contained(&self, candidate: &Path) -> Result<(), DeveloperError> {
        if candidate == self.root.as_ref() || candidate.starts_with(self.root.as_ref()) {
            Ok(())
        } else {
            Err(DeveloperError::Sandbox(format!(
                "path escapes workspace root: {}",
                candidate.display()
            )))
        }
    }

    fn capture_original(&self, relative: &str, path: &Path) -> Result<(), DeveloperError> {
        let mut originals = self
            .originals
            .lock()
            .map_err(|_| DeveloperError::Store("file snapshot lock failed".to_owned()))?;
        originals
            .entry(normalize(Path::new(relative)))
            .or_insert_with(|| fs::read(path).ok());
        Ok(())
    }

    fn relative_display(&self, path: &Path) -> String {
        normalize(path.strip_prefix(self.root.as_ref()).unwrap_or(path))
    }
}

fn safe_walk(root: &Path) -> impl Iterator<Item = walkdir::DirEntry> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !ignored_directory(entry.path()))
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
}

fn ignored_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| matches!(name, ".git" | "node_modules" | "target" | ".idea" | ".vs"))
}

fn ensure_safe_read(path: &Path) -> Result<(), DeveloperError> {
    if is_secret_path(path) {
        return Err(DeveloperError::Permission(format!(
            "secret-like file is protected: {}",
            path.display()
        )));
    }
    if !path.is_file() {
        return Err(DeveloperError::Invalid("path is not a file".to_owned()));
    }
    Ok(())
}

fn is_secret_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || matches!(name.as_str(), "id_rsa" | "id_ed25519")
        || matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("pem" | "pfx" | "key")
        )
}

fn read_git_branch(root: &Path) -> Option<String> {
    let head = fs::read_to_string(root.join(".git").join("HEAD")).ok()?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map(str::to_owned)
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn line_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count() + usize::from(!bytes.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_rejects_parent_escape_and_secret_reads() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("safe.txt"), "safe").unwrap();
        fs::write(temp.path().join(".env"), "SECRET=value").unwrap();
        let toolkit = FileToolkit::new(Workspace {
            id: Uuid::new_v4(),
            name: "test".to_owned(),
            root: normalize(&fs::canonicalize(temp.path()).unwrap()),
            git_enabled: false,
            branch: None,
            registered_at: Utc::now(),
            last_opened_at: Utc::now(),
        })
        .unwrap();
        assert!(matches!(
            toolkit.read_file("../outside.txt"),
            Err(DeveloperError::Sandbox(_))
        ));
        assert!(matches!(
            toolkit.read_file(".env"),
            Err(DeveloperError::Permission(_))
        ));
        assert_eq!(toolkit.read_file("safe.txt").unwrap(), "safe");
    }

    #[test]
    fn patch_produces_diff_and_rollback_restores_original() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("value.txt"), "old\n").unwrap();
        let registry = WorkspaceRegistry::open(temp.path().join("registry.json")).unwrap();
        let workspace = registry.register("test", temp.path()).unwrap();
        let toolkit = FileToolkit::new(workspace).unwrap();
        toolkit
            .apply_patch(&[TextReplacement {
                path: "value.txt".to_owned(),
                expected: "old".to_owned(),
                replacement: "new".to_owned(),
                replace_all: false,
            }])
            .unwrap();
        assert!(toolkit.unified_diff().unwrap().contains("+new"));
        toolkit.rollback().unwrap();
        assert_eq!(
            fs::read_to_string(temp.path().join("value.txt")).unwrap(),
            "old\n"
        );
    }
}
