use crate::MemoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MemoryId(pub Uuid);

impl MemoryId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MemoryId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    Working,
    Conversation,
    LongTerm,
    Project,
    Knowledge,
    Decision,
    Preference,
    Experience,
    Success,
    Failure,
    System,
    VxnKnowledge,
}

impl MemoryCategory {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Conversation => "conversation",
            Self::LongTerm => "long_term",
            Self::Project => "project",
            Self::Knowledge => "knowledge",
            Self::Decision => "decision",
            Self::Preference => "preference",
            Self::Experience => "experience",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::System => "system",
            Self::VxnKnowledge => "vxn_knowledge",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, MemoryError> {
        match value {
            "working" => Ok(Self::Working),
            "conversation" => Ok(Self::Conversation),
            "long_term" => Ok(Self::LongTerm),
            "project" => Ok(Self::Project),
            "knowledge" => Ok(Self::Knowledge),
            "decision" => Ok(Self::Decision),
            "preference" => Ok(Self::Preference),
            "experience" => Ok(Self::Experience),
            "success" => Ok(Self::Success),
            "failure" => Ok(Self::Failure),
            "system" => Ok(Self::System),
            "vxn_knowledge" => Ok(Self::VxnKnowledge),
            _ => Err(MemoryError::Unavailable),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeType {
    System,
    Organization,
    User,
    Application,
    Project,
    Session,
}

impl ScopeType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Organization => "organization",
            Self::User => "user",
            Self::Application => "application",
            Self::Project => "project",
            Self::Session => "session",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, MemoryError> {
        match value {
            "system" => Ok(Self::System),
            "organization" => Ok(Self::Organization),
            "user" => Ok(Self::User),
            "application" => Ok(Self::Application),
            "project" => Ok(Self::Project),
            "session" => Ok(Self::Session),
            _ => Err(MemoryError::Unavailable),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryScope {
    pub scope_type: ScopeType,
    pub organization_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub application_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
}

impl MemoryScope {
    pub fn system() -> Self {
        Self {
            scope_type: ScopeType::System,
            organization_id: None,
            user_id: None,
            application_id: None,
            project_id: None,
            session_id: None,
        }
    }

    pub fn project(application_id: Uuid, project_id: Uuid) -> Self {
        Self {
            scope_type: ScopeType::Project,
            organization_id: None,
            user_id: None,
            application_id: Some(application_id),
            project_id: Some(project_id),
            session_id: None,
        }
    }

    pub fn validate(&self) -> Result<(), MemoryError> {
        let valid = match self.scope_type {
            ScopeType::System => self.all_ids_empty(),
            ScopeType::Organization => {
                self.organization_id.is_some()
                    && self.user_id.is_none()
                    && self.application_id.is_none()
                    && self.project_id.is_none()
                    && self.session_id.is_none()
            }
            ScopeType::User => self.user_id.is_some() && self.lower_ids_empty(),
            ScopeType::Application => {
                self.application_id.is_some()
                    && self.project_id.is_none()
                    && self.session_id.is_none()
            }
            ScopeType::Project => {
                self.application_id.is_some()
                    && self.project_id.is_some()
                    && self.session_id.is_none()
            }
            ScopeType::Session => self.application_id.is_some() && self.session_id.is_some(),
        };
        if valid {
            Ok(())
        } else {
            Err(MemoryError::Invalid(
                "scope identifiers do not match scope type".to_owned(),
            ))
        }
    }

    fn all_ids_empty(&self) -> bool {
        self.organization_id.is_none()
            && self.user_id.is_none()
            && self.application_id.is_none()
            && self.project_id.is_none()
            && self.session_id.is_none()
    }

    fn lower_ids_empty(&self) -> bool {
        self.application_id.is_none() && self.project_id.is_none() && self.session_id.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MemoryPrivacy {
    pub local_only: bool,
    pub cloud_allowed: bool,
    pub sensitive: bool,
    pub share_scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateMemory {
    pub category: MemoryCategory,
    pub scope: MemoryScope,
    pub owner_id: Option<Uuid>,
    pub content: String,
    pub structured_content: Value,
    pub priority: f32,
    pub confidence: f32,
    pub source: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub privacy: MemoryPrivacy,
    pub metadata: Value,
}

impl CreateMemory {
    pub fn validate(&self) -> Result<(), MemoryError> {
        self.scope.validate()?;
        if self.content.trim().is_empty() {
            return Err(MemoryError::Invalid("content cannot be empty".to_owned()));
        }
        if self.source.trim().is_empty() {
            return Err(MemoryError::Invalid("source cannot be empty".to_owned()));
        }
        if !(0.0..=1.0).contains(&self.priority) || !(0.0..=1.0).contains(&self.confidence) {
            return Err(MemoryError::Invalid(
                "priority and confidence must be between 0 and 1".to_owned(),
            ));
        }
        if self.privacy.local_only && self.privacy.cloud_allowed {
            return Err(MemoryError::Invalid(
                "local-only memory cannot allow cloud processing".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateMemory {
    pub expected_version: i64,
    pub content: String,
    pub structured_content: Value,
    pub priority: f32,
    pub confidence: f32,
    pub expires_at: Option<DateTime<Utc>>,
    pub privacy: MemoryPrivacy,
    pub metadata: Value,
}

impl UpdateMemory {
    pub fn validate(&self) -> Result<(), MemoryError> {
        if self.expected_version < 1 || self.content.trim().is_empty() {
            return Err(MemoryError::Invalid("invalid memory update".to_owned()));
        }
        if !(0.0..=1.0).contains(&self.priority) || !(0.0..=1.0).contains(&self.confidence) {
            return Err(MemoryError::Invalid(
                "priority and confidence must be between 0 and 1".to_owned(),
            ));
        }
        if self.privacy.local_only && self.privacy.cloud_allowed {
            return Err(MemoryError::Invalid(
                "local-only memory cannot allow cloud processing".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub memory_id: MemoryId,
    pub category: MemoryCategory,
    pub scope: MemoryScope,
    pub owner_id: Option<Uuid>,
    pub content: String,
    pub structured_content: Value,
    pub priority: f32,
    pub confidence: f32,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub privacy: MemoryPrivacy,
    pub metadata: Value,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryQuery {
    pub scope: MemoryScope,
    pub text: Option<String>,
    pub category: Option<MemoryCategory>,
    pub include_expired: bool,
    pub limit: u32,
}

impl MemoryQuery {
    pub fn validate(&self) -> Result<(), MemoryError> {
        self.scope.validate()?;
        if self.limit == 0 || self.limit > 100 {
            return Err(MemoryError::Invalid(
                "memory query limit must be between 1 and 100".to_owned(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn normalize_content(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
