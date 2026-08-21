use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

use crate::HardPermission;

pub type WorkspaceId = Uuid;
pub type DeveloperTaskId = Uuid;
pub type CommandExecutionId = Uuid;

#[derive(Debug, Error)]
pub enum DeveloperError {
    #[error("invalid developer request: {0}")]
    Invalid(String),
    #[error("workspace sandbox rejected access: {0}")]
    Sandbox(String),
    #[error("developer permission denied: {0}")]
    Permission(String),
    #[error("developer item was not found: {0}")]
    NotFound(String),
    #[error("developer I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("developer serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("developer terminal failed: {0}")]
    Terminal(String),
    #[error("developer model failed: {0}")]
    Model(String),
    #[error("developer store failed: {0}")]
    Store(String),
    #[error("developer agent limit reached: {0}")]
    Limit(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeveloperMode {
    Ask,
    ReadOnly,
    Edit,
    Execute,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeveloperTaskState {
    Queued,
    Analyzing,
    Planning,
    Implementing,
    Building,
    Testing,
    Fixing,
    Reviewing,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

impl DeveloperTaskState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub root: String,
    pub git_enabled: bool,
    pub branch: Option<String>,
    pub registered_at: DateTime<Utc>,
    pub last_opened_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLimits {
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub max_runtime_seconds: u64,
    pub max_failed_attempts: u32,
    pub max_consecutive_errors: u32,
    pub max_context_chars: usize,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            max_steps: 40,
            max_tool_calls: 60,
            max_runtime_seconds: 30 * 60,
            max_failed_attempts: 8,
            max_consecutive_errors: 3,
            max_context_chars: 60_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: u32,
    pub description: String,
    pub state: PlanStepState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepState {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanRevision {
    pub version: u32,
    pub reason: String,
    pub steps: Vec<PlanStep>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeveloperActivity {
    pub sequence: u64,
    pub occurred_at: DateTime<Utc>,
    pub kind: String,
    pub message: String,
    pub detail: Option<String>,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub kind: FileChangeKind,
    pub additions: usize,
    pub deletions: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandExecution {
    pub id: CommandExecutionId,
    pub executable: String,
    pub args: Vec<String>,
    pub working_directory: String,
    pub process_id: Option<u32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub timeout_ms: u64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub status: CommandStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StructuredTestResult {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeveloperErrorEvent {
    pub error_type: String,
    pub language: Option<String>,
    pub code: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeveloperTask {
    pub id: DeveloperTaskId,
    pub workspace_id: WorkspaceId,
    pub request: String,
    pub mode: DeveloperMode,
    pub model: String,
    #[serde(default)]
    pub soft_policy: Option<String>,
    #[serde(default)]
    pub hard_permission: Option<HardPermission>,
    pub state: DeveloperTaskState,
    pub risk: RiskLevel,
    pub confidence: f32,
    pub confidence_reason: String,
    pub plan_revisions: Vec<PlanRevision>,
    pub activities: Vec<DeveloperActivity>,
    #[serde(default)]
    pub files_read: Vec<String>,
    pub files_changed: Vec<FileChange>,
    pub commands: Vec<CommandExecution>,
    pub errors: Vec<DeveloperErrorEvent>,
    pub unified_diff: String,
    pub result_summary: Option<String>,
    pub knowledge_saved: bool,
    pub steps_completed: u32,
    pub tool_calls: u32,
    pub failed_attempts: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl DeveloperTask {
    pub fn new(
        workspace_id: WorkspaceId,
        request: impl Into<String>,
        mode: DeveloperMode,
        model: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            workspace_id,
            request: request.into(),
            mode,
            model: model.into(),
            soft_policy: None,
            hard_permission: None,
            state: DeveloperTaskState::Queued,
            risk: RiskLevel::Low,
            confidence: 0.0,
            confidence_reason: "not evaluated".to_owned(),
            plan_revisions: Vec::new(),
            activities: Vec::new(),
            files_read: Vec::new(),
            files_changed: Vec::new(),
            commands: Vec::new(),
            errors: Vec::new(),
            unified_diff: String::new(),
            result_summary: None,
            knowledge_saved: false,
            steps_completed: 0,
            tool_calls: 0,
            failed_attempts: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub required_mode: DeveloperMode,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextReplacement {
    pub path: String,
    pub expected: String,
    pub replacement: String,
    pub replace_all: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tool", content = "input", rename_all = "snake_case")]
pub enum ToolCall {
    ListDirectory {
        path: String,
    },
    ReadFile {
        path: String,
    },
    ReadFileRange {
        path: String,
        start_line: usize,
        end_line: usize,
    },
    SearchFiles {
        query: String,
        extension: Option<String>,
        directory: Option<String>,
    },
    SearchText {
        query: String,
        extension: Option<String>,
        directory: Option<String>,
    },
    GetFileMetadata {
        path: String,
    },
    GetProjectTree {
        depth: usize,
    },
    CreateFile {
        path: String,
        content: String,
        reason: String,
    },
    WriteFile {
        path: String,
        content: String,
        reason: String,
    },
    ApplyPatch {
        replacements: Vec<TextReplacement>,
        reason: String,
    },
    DeleteFile {
        path: String,
        reason: String,
    },
    RunCommand {
        executable: String,
        args: Vec<String>,
        working_directory: String,
        timeout_ms: u64,
        purpose: String,
    },
    GetDiff,
}

impl ToolCall {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ListDirectory { .. } => "list_directory",
            Self::ReadFile { .. } => "read_file",
            Self::ReadFileRange { .. } => "read_file_range",
            Self::SearchFiles { .. } => "search_files",
            Self::SearchText { .. } => "search_text",
            Self::GetFileMetadata { .. } => "get_file_metadata",
            Self::GetProjectTree { .. } => "get_project_tree",
            Self::CreateFile { .. } => "create_file",
            Self::WriteFile { .. } => "write_file",
            Self::ApplyPatch { .. } => "apply_patch",
            Self::DeleteFile { .. } => "delete_file",
            Self::RunCommand { .. } => "run_command",
            Self::GetDiff => "get_diff",
        }
    }

    pub fn risk(&self) -> RiskLevel {
        match self {
            Self::DeleteFile { .. } => RiskLevel::High,
            Self::RunCommand {
                executable, args, ..
            } => command_risk(executable, args),
            Self::CreateFile { .. } | Self::WriteFile { .. } | Self::ApplyPatch { .. } => {
                RiskLevel::Medium
            }
            _ => RiskLevel::Low,
        }
    }

    pub fn required_mode(&self) -> DeveloperMode {
        match self {
            Self::CreateFile { .. }
            | Self::WriteFile { .. }
            | Self::ApplyPatch { .. }
            | Self::DeleteFile { .. } => DeveloperMode::Edit,
            Self::RunCommand { .. } => DeveloperMode::Execute,
            _ => DeveloperMode::ReadOnly,
        }
    }
}

fn command_risk(executable: &str, args: &[String]) -> RiskLevel {
    let executable = executable.to_ascii_lowercase();
    let joined = args.join(" ").to_ascii_lowercase();
    if executable.contains("powershell")
        || executable.contains("cmd")
        || (executable == "git"
            && (joined.contains("reset")
                || joined.contains("clean")
                || joined.contains("push")
                || joined.contains("commit")))
    {
        RiskLevel::High
    } else {
        RiskLevel::Low
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    Plan {
        steps: Vec<String>,
        reason: String,
    },
    Tool {
        call: ToolCall,
        rationale: String,
    },
    Complete {
        summary: String,
        confidence: f32,
        reason: String,
    },
    RequireApproval {
        summary: String,
        risk: RiskLevel,
    },
    Fail {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentTurnInput {
    pub task: DeveloperTask,
    pub workspace: Workspace,
    pub recent_observations: Vec<ToolResult>,
    pub available_tools: Vec<ToolDefinition>,
    pub runtime_directive: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartDeveloperTask {
    pub workspace_id: WorkspaceId,
    pub request: String,
    pub mode: DeveloperMode,
    pub provider_id: String,
    pub model_id: String,
    #[serde(default)]
    pub soft_policy: Option<String>,
    #[serde(default)]
    pub hard_permission: Option<HardPermission>,
    pub limits: Option<AgentLimits>,
}
