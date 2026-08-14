//! Durable, bounded JSON Lines audit log for desktop operations.

use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
use vertex_ai_types::AuditEvent;

const DEFAULT_MAX_BYTES: u64 = 5 * 1024 * 1024;
const MAX_QUERY_LIMIT: usize = 500;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("audit log I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("audit event serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("audit log lock is poisoned")]
    LockPoisoned,
}

#[derive(Debug)]
pub struct PersistentAuditLog {
    path: PathBuf,
    max_bytes: u64,
}

impl PersistentAuditLog {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, AuditError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(Self {
            path,
            max_bytes: DEFAULT_MAX_BYTES,
        })
    }

    pub fn append(&mut self, event: &AuditEvent) -> Result<(), AuditError> {
        let bytes = serde_json::to_vec(event)?;
        self.rotate_if_needed(bytes.len() as u64 + 1)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_data()?;
        Ok(())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<AuditEvent>, AuditError> {
        let limit = limit.clamp(1, MAX_QUERY_LIMIT);
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let reader = BufReader::new(fs::File::open(&self.path)?);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            events.push(serde_json::from_str(&line)?);
            if events.len() > limit {
                events.remove(0);
            }
        }
        Ok(events)
    }

    fn rotate_if_needed(&self, incoming_bytes: u64) -> Result<(), std::io::Error> {
        let current_bytes = fs::metadata(&self.path)
            .map(|value| value.len())
            .unwrap_or(0);
        if current_bytes.saturating_add(incoming_bytes) <= self.max_bytes {
            return Ok(());
        }
        let rotated = rotated_path(&self.path);
        if rotated.exists() {
            fs::remove_file(&rotated)?;
        }
        if self.path.exists() {
            fs::rename(&self.path, rotated)?;
        }
        Ok(())
    }
}

fn rotated_path(path: &Path) -> PathBuf {
    path.with_extension("previous.jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::BTreeMap;
    use uuid::Uuid;
    use vertex_ai_types::{AuditEventId, AuditOutcome};

    fn event(operation: &str) -> AuditEvent {
        AuditEvent {
            id: AuditEventId::new(format!("audit:{}", Uuid::new_v4())).expect("id"),
            occurred_at: Utc::now(),
            actor: "test".to_owned(),
            operation: operation.to_owned(),
            target_ids: Vec::new(),
            outcome: AuditOutcome::Succeeded,
            elevated: false,
            details: BTreeMap::new(),
        }
    }

    #[test]
    fn audit_events_survive_reopen_and_keep_order() {
        let root = std::env::temp_dir().join(format!("vertex-audit-test-{}", Uuid::new_v4()));
        let path = root.join("audit.jsonl");
        let mut log = PersistentAuditLog::open(&path).expect("open");
        log.append(&event("first")).expect("append first");
        log.append(&event("second")).expect("append second");
        let reopened = PersistentAuditLog::open(&path).expect("reopen");
        let events = reopened.recent(10).expect("read");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].operation, "first");
        assert_eq!(events[1].operation, "second");
        fs::remove_dir_all(root).expect("cleanup");
    }
}
