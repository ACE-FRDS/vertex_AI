use crate::{
    AgentLimits, ArdAssignment, ArdCoordinator, ArdExecutionStatus, ArdIntervention, ArdMemberId,
    ArdSession, ArdSessionId, ArdSessionState, ArdStageId, BrainAssignment, CommandExecution,
    CommandStatus, CompleteArdStage, DeveloperCoordinator, DeveloperEngine, DeveloperError,
    DeveloperErrorEvent, DeveloperMode, DeveloperTask, DeveloperTaskId, DeveloperTaskState,
    FileChange, HandoffDecision, HardPermission, StartDeveloperTask, StructuredHandoff,
    StructuredTestResult, ToolCapability, WorkspaceId,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedBrain {
    pub provider_id: String,
    pub model_id: String,
    pub runtime_id: Option<String>,
    pub temporary_auto_fallback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourcePolicy {
    PreferSpeed,
    PreferQuality,
    PreferLocal,
    MinimizeVram,
    #[default]
    Balanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FallbackPolicy {
    Disabled,
    AutoAlternative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrainResolutionContext {
    pub resource_policy: ResourcePolicy,
    pub fallback_policy: FallbackPolicy,
    pub excluded_model_ids: BTreeSet<String>,
}

impl Default for BrainResolutionContext {
    fn default() -> Self {
        Self {
            resource_policy: ResourcePolicy::Balanced,
            fallback_policy: FallbackPolicy::AutoAlternative,
            excluded_model_ids: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrainResolution {
    pub requested: String,
    pub resolved_brain: ResolvedBrain,
    pub reason: String,
    pub compatibility: String,
    pub fallback_used: bool,
    pub required_capabilities: Vec<String>,
    pub score: u32,
    pub resource_policy: ResourcePolicy,
}

impl ResolvedBrain {
    pub fn label(&self) -> String {
        format!("{}/{}", self.provider_id, self.model_id)
    }
}

pub struct ResolvedArdEngine {
    pub brain: ResolvedBrain,
    pub resolution: BrainResolution,
    pub engine: Arc<dyn DeveloperEngine>,
}

#[async_trait]
pub trait ArdEngineResolver: Send + Sync {
    async fn resolve(
        &self,
        assignment: &ArdAssignment,
        context: &BrainResolutionContext,
    ) -> Result<ResolvedArdEngine, DeveloperError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRotationPlan {
    pub current_model: Option<String>,
    pub next_model: String,
    pub current_runtime: Option<String>,
    pub next_runtime: String,
    pub reuse_possible: bool,
    pub rotation_required: bool,
    pub resource_policy: ResourcePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModelRotationEventKind {
    ModelRotationStarted,
    ModelUnloading,
    ModelLoading,
    ModelReused,
    ModelRotationCompleted,
    ModelRotationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRotationEvent {
    pub kind: ModelRotationEventKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRotationResult {
    pub plan: ModelRotationPlan,
    pub attempts: u32,
    pub reused: bool,
    pub actual_model_state: String,
    pub events: Vec<ModelRotationEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationControl {
    Continue,
    Pause,
    Cancel,
}

#[async_trait]
pub trait ArdRuntimeController: Send + Sync {
    async fn rotate(
        &self,
        plan: &ModelRotationPlan,
        events: mpsc::UnboundedSender<ModelRotationEvent>,
        control: watch::Receiver<RotationControl>,
    ) -> Result<ModelRotationResult, DeveloperError>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentExecutionRequest {
    pub ard_session_id: ArdSessionId,
    pub stage_id: ArdStageId,
    pub member_id: ArdMemberId,
    pub role: String,
    pub resolved_brain: ResolvedBrain,
    pub workspace_id: WorkspaceId,
    pub task: String,
    pub soft_policy: String,
    pub hard_permissions: HardPermission,
    pub context: Vec<StructuredHandoff>,
    pub actual_diff: String,
    pub user_interrupts: Vec<ArdIntervention>,
    pub execution_mode: DeveloperMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentExecutionStatus {
    Completed,
    Failed,
    Cancelled,
    WaitingApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandFact {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub status: CommandStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestFact {
    pub command: CommandFact,
    pub result: StructuredTestResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentExecutionResult {
    pub developer_task_id: DeveloperTaskId,
    pub status: AgentExecutionStatus,
    pub summary: String,
    pub files_read: Vec<String>,
    pub files_changed: Vec<FileChange>,
    pub tool_calls: u32,
    pub build_results: Vec<CommandFact>,
    pub test_results: Vec<TestFact>,
    pub errors: Vec<DeveloperErrorEvent>,
    pub cancelled: bool,
    pub timed_out: bool,
    pub needs_human_decision: bool,
    pub confidence: f32,
    pub confidence_reason: String,
    pub unified_diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewResult {
    pub approved: bool,
    #[serde(default)]
    pub issues: Vec<String>,
    pub severity: String,
    #[serde(default)]
    pub required_changes: Vec<String>,
    pub reason: String,
}

#[derive(Clone)]
pub struct ArdExecutionBridge {
    ard: Arc<ArdCoordinator>,
    developer: Arc<DeveloperCoordinator>,
    resolver: Arc<dyn ArdEngineResolver>,
    runtime: Arc<dyn ArdRuntimeController>,
    workers: Arc<Mutex<BTreeSet<ArdSessionId>>>,
    rotation_controls: Arc<Mutex<BTreeMap<ArdSessionId, watch::Sender<RotationControl>>>>,
    poll_interval: Duration,
    limits: AgentLimits,
}

impl ArdExecutionBridge {
    pub fn new(
        ard: Arc<ArdCoordinator>,
        developer: Arc<DeveloperCoordinator>,
        resolver: Arc<dyn ArdEngineResolver>,
        runtime: Arc<dyn ArdRuntimeController>,
    ) -> Self {
        Self {
            ard,
            developer,
            resolver,
            runtime,
            workers: Arc::new(Mutex::new(BTreeSet::new())),
            rotation_controls: Arc::new(Mutex::new(BTreeMap::new())),
            poll_interval: Duration::from_millis(250),
            limits: AgentLimits::default(),
        }
    }

    pub fn with_limits(mut self, limits: AgentLimits) -> Self {
        self.limits = limits;
        self
    }

    #[cfg(test)]
    fn with_poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }

    pub fn start(
        &self,
        workflow_id: crate::ArdWorkflowId,
        goal: impl Into<String>,
    ) -> Result<ArdSession, DeveloperError> {
        let session = self.ard.start_session(workflow_id, goal)?;
        self.spawn(session.id)?;
        Ok(session)
    }

    pub fn spawn(&self, session_id: ArdSessionId) -> Result<(), DeveloperError> {
        {
            let mut workers = self
                .workers
                .lock()
                .map_err(|_| DeveloperError::Store("ARD bridge worker lock failed".to_owned()))?;
            if !workers.insert(session_id) {
                return Ok(());
            }
        }
        let bridge = self.clone();
        tokio::spawn(async move {
            let result = bridge.run_to_completion(session_id).await;
            if let Err(error) = result {
                let _ = bridge.ard.append_activity(
                    session_id,
                    None,
                    "bridge_error",
                    &format!("ARD Execution Bridgeが停止しました: {error}"),
                );
            }
            if let Ok(mut workers) = bridge.workers.lock() {
                workers.remove(&session_id);
            }
        });
        Ok(())
    }

    pub async fn run_to_completion(
        &self,
        session_id: ArdSessionId,
    ) -> Result<ArdSession, DeveloperError> {
        loop {
            let session = self.ard.get_session(session_id)?;
            if session.state != ArdSessionState::Running {
                return Ok(session);
            }
            let assignment = self.ard.current_assignment(session_id)?;
            self.ard.append_activity(
                session_id,
                Some(assignment.member.id),
                "stage_execution_started",
                &format!(
                    "{} / {} started",
                    assignment.member.name, assignment.member.role
                ),
            )?;
            let resolved = match self.resolve_and_prepare(session_id, &assignment).await {
                Ok(resolved) => resolved,
                Err(error) => {
                    let current = self.ard.get_session(session_id)?;
                    if matches!(
                        current.state,
                        ArdSessionState::Paused | ArdSessionState::Cancelled
                    ) {
                        return Ok(current);
                    }
                    return self.block_stage(session_id, &assignment, error.to_string());
                }
            };
            let request = self
                .build_execution_request(&assignment, resolved.brain.clone())
                .await?;
            let task = self
                .developer
                .start_task(
                    StartDeveloperTask {
                        workspace_id: request.workspace_id,
                        request: execution_prompt(&request)?,
                        mode: request.execution_mode,
                        provider_id: request.resolved_brain.provider_id.clone(),
                        model_id: request.resolved_brain.model_id.clone(),
                        soft_policy: Some(request.soft_policy.clone()),
                        hard_permission: Some(request.hard_permissions.clone()),
                        limits: Some(self.limits.clone()),
                    },
                    resolved.engine,
                )
                .await?;
            self.ard.begin_stage_execution(
                session_id,
                assignment.stage.id,
                assignment.member.id,
                task.id,
                request.resolved_brain.label(),
            )?;
            let completed = self.wait_for_task(session_id, &assignment, task.id).await?;
            let result = map_execution_result(&completed);
            let execution_status = match result.status {
                AgentExecutionStatus::Completed => ArdExecutionStatus::Completed,
                AgentExecutionStatus::Failed => ArdExecutionStatus::Failed,
                AgentExecutionStatus::Cancelled => ArdExecutionStatus::Cancelled,
                AgentExecutionStatus::WaitingApproval => ArdExecutionStatus::WaitingApproval,
            };
            self.ard
                .finish_stage_execution(session_id, task.id, execution_status)?;
            let current = self.ard.get_session(session_id)?;
            if matches!(
                current.state,
                ArdSessionState::Paused | ArdSessionState::Cancelled
            ) {
                return Ok(current);
            }
            if result.cancelled && current.state == ArdSessionState::Running {
                self.ard.append_activity(
                    session_id,
                    Some(assignment.member.id),
                    "stage_execution_restarted",
                    "中断されたステージを現在の制御状態から再実行します",
                )?;
                continue;
            }
            let handoff = build_handoff(&assignment, &result);
            let next = self.ard.complete_stage(session_id, handoff)?;
            if next.state != ArdSessionState::Running {
                return Ok(next);
            }
        }
    }

    pub async fn pause(&self, session_id: ArdSessionId) -> Result<ArdSession, DeveloperError> {
        let session = self.ard.pause(session_id)?;
        self.signal_rotation(session_id, RotationControl::Pause)?;
        if let Some(execution) = &session.active_execution {
            let _ = self
                .developer
                .cancel_task(execution.developer_task_id)
                .await?;
        }
        Ok(session)
    }

    pub async fn resume(&self, session_id: ArdSessionId) -> Result<ArdSession, DeveloperError> {
        let session = self.ard.resume(session_id)?;
        self.spawn(session_id)?;
        Ok(session)
    }

    pub async fn cancel(&self, session_id: ArdSessionId) -> Result<ArdSession, DeveloperError> {
        let session = self.ard.cancel(session_id)?;
        self.signal_rotation(session_id, RotationControl::Cancel)?;
        if let Some(execution) = &session.active_execution {
            let _ = self
                .developer
                .cancel_task(execution.developer_task_id)
                .await?;
        }
        Ok(session)
    }

    fn signal_rotation(
        &self,
        session_id: ArdSessionId,
        control: RotationControl,
    ) -> Result<(), DeveloperError> {
        if let Some(sender) = self
            .rotation_controls
            .lock()
            .map_err(|_| DeveloperError::Store("rotation control lock failed".to_owned()))?
            .get(&session_id)
        {
            let _ = sender.send(control);
        }
        Ok(())
    }

    async fn resolve_and_prepare(
        &self,
        session_id: ArdSessionId,
        assignment: &ArdAssignment,
    ) -> Result<ResolvedArdEngine, DeveloperError> {
        let auto = matches!(assignment.member.brain, BrainAssignment::Auto);
        let mut context = BrainResolutionContext {
            resource_policy: ResourcePolicy::Balanced,
            fallback_policy: if auto {
                FallbackPolicy::AutoAlternative
            } else {
                FallbackPolicy::Disabled
            },
            excluded_model_ids: BTreeSet::new(),
        };
        let mut fallback_used = false;
        loop {
            let mut resolved = self.resolver.resolve(assignment, &context).await?;
            resolved.resolution.fallback_used |= fallback_used;
            self.ard
                .record_brain_resolution(session_id, assignment, &resolved.resolution)?;
            let session = self.ard.get_session(session_id)?;
            let next_runtime = resolved
                .brain
                .runtime_id
                .clone()
                .unwrap_or_else(|| resolved.brain.provider_id.clone());
            let next_model = resolved.brain.label();
            let reuse_possible = session.active_model.as_deref() == Some(&next_model)
                && session.active_runtime.as_deref() == Some(&next_runtime);
            let plan = ModelRotationPlan {
                current_model: session.active_model.clone(),
                next_model: next_model.clone(),
                current_runtime: session.active_runtime.clone(),
                next_runtime,
                reuse_possible,
                rotation_required: !reuse_possible,
                resource_policy: context.resource_policy,
            };
            match self
                .execute_rotation(session_id, assignment.member.id, &plan)
                .await
            {
                Ok(_) => return Ok(resolved),
                Err(error) => {
                    let current = self.ard.get_session(session_id)?;
                    if matches!(
                        current.state,
                        ArdSessionState::Paused | ArdSessionState::Cancelled
                    ) {
                        return Err(error);
                    }
                    if context.fallback_policy == FallbackPolicy::AutoAlternative && !fallback_used
                    {
                        context
                            .excluded_model_ids
                            .insert(resolved.brain.model_id.clone());
                        fallback_used = true;
                        self.ard.append_activity(
                            session_id,
                            Some(assignment.member.id),
                            "model_fallback",
                            "モデル切替に失敗したためPolicyに従って代替候補を再解決します",
                        )?;
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    async fn execute_rotation(
        &self,
        session_id: ArdSessionId,
        member_id: ArdMemberId,
        plan: &ModelRotationPlan,
    ) -> Result<ModelRotationResult, DeveloperError> {
        self.ard.begin_model_rotation(session_id, member_id, plan)?;
        let (control_tx, control_rx) = watch::channel(RotationControl::Continue);
        self.rotation_controls
            .lock()
            .map_err(|_| DeveloperError::Store("rotation control lock failed".to_owned()))?
            .insert(session_id, control_tx);
        let mut last_error = None;
        let mut attempts = 0;
        for attempt in 1..=2 {
            attempts = attempt;
            let (event_tx, mut event_rx) = mpsc::unbounded_channel();
            let operation = self.runtime.rotate(plan, event_tx, control_rx.clone());
            tokio::pin!(operation);
            let result = loop {
                tokio::select! {
                    result = &mut operation => break result,
                    event = event_rx.recv() => {
                        if let Some(event) = event {
                            self.ard.append_model_rotation_event(
                                session_id,
                                member_id,
                                event,
                            )?;
                        }
                    }
                }
            };
            while let Ok(event) = event_rx.try_recv() {
                self.ard
                    .append_model_rotation_event(session_id, member_id, event)?;
            }
            match result {
                Ok(mut result) => {
                    result.attempts = attempt;
                    self.ard.finish_model_rotation(
                        session_id,
                        member_id,
                        true,
                        attempt,
                        result.reused,
                        &format!(
                            "Model準備完了: {} ({})",
                            result.plan.next_model, result.actual_model_state
                        ),
                    )?;
                    self.rotation_controls
                        .lock()
                        .map_err(|_| {
                            DeveloperError::Store("rotation control lock failed".to_owned())
                        })?
                        .remove(&session_id);
                    return Ok(result);
                }
                Err(error) => {
                    last_error = Some(error);
                    let session = self.ard.get_session(session_id)?;
                    if matches!(
                        session.state,
                        ArdSessionState::Paused | ArdSessionState::Cancelled
                    ) {
                        break;
                    }
                    if attempt < 2 {
                        self.ard.append_activity(
                            session_id,
                            Some(member_id),
                            "model_rotation_retry",
                            "モデル切替を再試行します",
                        )?;
                    }
                }
            }
        }
        self.rotation_controls
            .lock()
            .map_err(|_| DeveloperError::Store("rotation control lock failed".to_owned()))?
            .remove(&session_id);
        let error = last_error.unwrap_or_else(|| {
            DeveloperError::Model("model rotation failed without an error".to_owned())
        });
        self.ard.append_model_rotation_event(
            session_id,
            member_id,
            ModelRotationEvent {
                kind: ModelRotationEventKind::ModelRotationFailed,
                message: format!("Model rotation failed: {error}"),
            },
        )?;
        self.ard.finish_model_rotation(
            session_id,
            member_id,
            false,
            attempts,
            false,
            &format!("Model rotation failed: {error}"),
        )?;
        Err(error)
    }

    async fn build_execution_request(
        &self,
        assignment: &ArdAssignment,
        resolved_brain: ResolvedBrain,
    ) -> Result<AgentExecutionRequest, DeveloperError> {
        let session = self.ard.get_session(assignment.session_id)?;
        let actual_diff = if let Some(execution) = session.executions.last() {
            self.developer
                .get_task(execution.developer_task_id)
                .await
                .map(|task| task.unified_diff)
                .unwrap_or_default()
        } else {
            String::new()
        };
        Ok(AgentExecutionRequest {
            ard_session_id: assignment.session_id,
            stage_id: assignment.stage.id,
            member_id: assignment.member.id,
            role: assignment.member.role.clone(),
            resolved_brain,
            workspace_id: assignment.member.workspace_id,
            task: format!(
                "{}\n\nStage objective: {}",
                assignment.goal, assignment.stage.objective
            ),
            soft_policy: assignment.role_policy.clone(),
            hard_permissions: assignment.member.permission.clone(),
            context: assignment.relevant_handoffs.clone(),
            actual_diff,
            user_interrupts: assignment.interventions.clone(),
            execution_mode: execution_mode(&assignment.member.permission),
        })
    }

    async fn wait_for_task(
        &self,
        session_id: ArdSessionId,
        assignment: &ArdAssignment,
        task_id: DeveloperTaskId,
    ) -> Result<DeveloperTask, DeveloperError> {
        let mut forwarded = 0_usize;
        loop {
            let session = self.ard.get_session(session_id)?;
            if matches!(
                session.state,
                ArdSessionState::Paused | ArdSessionState::Cancelled
            ) {
                let _ = self.developer.cancel_task(task_id).await?;
            }
            let task = self.developer.get_task(task_id).await?;
            for activity in task.activities.iter().skip(forwarded) {
                self.ard.append_activity(
                    session_id,
                    Some(assignment.member.id),
                    &format!("agent_{}", activity.kind),
                    &activity.message,
                )?;
            }
            forwarded = task.activities.len();
            if task.state.is_terminal() || task.state == DeveloperTaskState::WaitingApproval {
                return Ok(task);
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }

    fn block_stage(
        &self,
        session_id: ArdSessionId,
        assignment: &ArdAssignment,
        reason: String,
    ) -> Result<ArdSession, DeveloperError> {
        self.ard.complete_stage(
            session_id,
            CompleteArdStage {
                decision: HandoffDecision::Blocked,
                task_result: reason.clone(),
                decisions: Vec::new(),
                files_read: Vec::new(),
                files_changed: Vec::new(),
                build_results: Vec::new(),
                tests_run: Vec::new(),
                test_results: Vec::new(),
                known_issues: vec![reason],
                unresolved_questions: Vec::new(),
                next_action: format!("{}のBrain設定を確認してください", assignment.member.name),
                confidence: 0.0,
            },
        )
    }
}

fn execution_mode(permission: &HardPermission) -> DeveloperMode {
    let write = permission.allowed.contains(&ToolCapability::WriteFiles);
    let execute = permission.allowed.contains(&ToolCapability::Terminal);
    match (write, execute) {
        (true, true) => DeveloperMode::Auto,
        (true, false) => DeveloperMode::Edit,
        (false, true) => DeveloperMode::Execute,
        (false, false) => DeveloperMode::ReadOnly,
    }
}

fn execution_prompt(request: &AgentExecutionRequest) -> Result<String, DeveloperError> {
    let reviewer_instruction = if is_reviewer(&request.role) {
        r#"Review only the supplied deterministic actual_diff, build_results, and test_results. Narrative task_result text is not evidence and must never override command facts. exit=Some(0) and failed=0 mean that command/test passed. Do not claim a failure that is absent from the deterministic evidence. Do not edit files. When complete, the AgentAction `summary` string MUST contain only this JSON schema: {"approved":true,"issues":[],"severity":"LOW","required_changes":[],"reason":"evidence"}. Set approved=false only when the actual diff or command facts prove a concrete change is required."#
    } else if is_architect(&request.role) {
        "Inspect only what is necessary to produce the architecture handoff. Do not perform downstream Developer edits or commands. Complete this stage after the bounded analysis."
    } else {
        "Complete the assigned stage using the available typed tools and report verified facts."
    };
    let payload = serde_json::to_string(request)?;
    if payload.chars().count() > 90_000 {
        return Err(DeveloperError::Limit(
            "ARD execution context exceeds the safe task request budget".to_owned(),
        ));
    }
    Ok(format!(
        "ARD Stage Execution\n{reviewer_instruction}\nBuild/test claims must come only from actual Tool results.\n\nAGENT_EXECUTION_REQUEST_JSON:\n{payload}"
    ))
}

fn map_execution_result(task: &DeveloperTask) -> AgentExecutionResult {
    let status = match task.state {
        DeveloperTaskState::Completed => AgentExecutionStatus::Completed,
        DeveloperTaskState::Cancelled => AgentExecutionStatus::Cancelled,
        DeveloperTaskState::WaitingApproval => AgentExecutionStatus::WaitingApproval,
        _ => AgentExecutionStatus::Failed,
    };
    let mut build_results = Vec::new();
    let mut test_results = Vec::new();
    for command in &task.commands {
        let fact = command_fact(command);
        if is_test_command(command) {
            test_results.push(TestFact {
                result: crate::agent::parse_test_result(&format!(
                    "{}\n{}",
                    command.stdout, command.stderr
                )),
                command: fact,
            });
        } else {
            build_results.push(fact);
        }
    }
    let timed_out = task
        .commands
        .iter()
        .any(|command| command.status == CommandStatus::Timeout)
        || task
            .result_summary
            .as_deref()
            .is_some_and(|summary| summary.to_ascii_lowercase().contains("runtime exceeded"));
    AgentExecutionResult {
        developer_task_id: task.id,
        status,
        summary: task.result_summary.clone().unwrap_or_else(|| match status {
            AgentExecutionStatus::Completed => "Stage completed".to_owned(),
            _ => format!("Developer Task ended in {:?}", task.state),
        }),
        files_read: task.files_read.clone(),
        files_changed: task.files_changed.clone(),
        tool_calls: task.tool_calls,
        build_results,
        test_results,
        errors: task.errors.clone(),
        cancelled: status == AgentExecutionStatus::Cancelled,
        timed_out,
        needs_human_decision: status == AgentExecutionStatus::WaitingApproval,
        confidence: task.confidence,
        confidence_reason: task.confidence_reason.clone(),
        unified_diff: task.unified_diff.clone(),
    }
}

fn build_handoff(assignment: &ArdAssignment, result: &AgentExecutionResult) -> CompleteArdStage {
    let review = is_reviewer(&assignment.member.role)
        .then(|| parse_review_result(&result.summary))
        .flatten();
    let decision = if result.status != AgentExecutionStatus::Completed {
        HandoffDecision::Blocked
    } else if let Some(review) = &review {
        if review.approved {
            HandoffDecision::Accepted
        } else {
            HandoffDecision::Rework
        }
    } else if is_reviewer(&assignment.member.role) {
        HandoffDecision::Blocked
    } else {
        HandoffDecision::Accepted
    };
    let mut known_issues = result
        .errors
        .iter()
        .map(|error| error.message.clone())
        .collect::<Vec<_>>();
    if let Some(review) = &review {
        known_issues.extend(review.issues.clone());
    }
    let next_action = review
        .as_ref()
        .filter(|review| !review.approved)
        .map(|review| review.required_changes.join("\n"))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if decision == HandoffDecision::Blocked {
                "Human Decision Required".to_owned()
            } else {
                "Continue to the next ARD stage".to_owned()
            }
        });
    let task_result = if !is_reviewer(&assignment.member.role)
        && (!result.build_results.is_empty()
            || !result.test_results.is_empty()
            || !result.files_changed.is_empty())
    {
        format!(
            "Verified stage facts: files_changed={}, build_commands={}, test_commands={}, failed_tests={}",
            result.files_changed.len(),
            result.build_results.len(),
            result.test_results.len(),
            result
                .test_results
                .iter()
                .map(|test| test.result.failed)
                .sum::<u32>()
        )
    } else {
        result.summary.clone()
    };
    CompleteArdStage {
        decision,
        task_result,
        decisions: review
            .as_ref()
            .map(|review| vec![review.reason.clone()])
            .unwrap_or_else(|| vec![result.confidence_reason.clone()]),
        files_read: result.files_read.clone(),
        files_changed: result
            .files_changed
            .iter()
            .map(|change| change.path.clone())
            .collect(),
        build_results: result
            .build_results
            .iter()
            .map(format_command_fact)
            .collect(),
        tests_run: result
            .test_results
            .iter()
            .map(|test| test.command.command.clone())
            .collect(),
        test_results: result
            .test_results
            .iter()
            .map(|test| {
                format!(
                    "{}: exit={:?}, passed={}, failed={}, skipped={}",
                    test.command.command,
                    test.command.exit_code,
                    test.result.passed,
                    test.result.failed,
                    test.result.skipped
                )
            })
            .collect(),
        known_issues,
        unresolved_questions: Vec::new(),
        next_action,
        confidence: result.confidence.clamp(0.0, 1.0),
    }
}

fn command_fact(command: &CommandExecution) -> CommandFact {
    let duration_ms = command
        .finished_at
        .map(|finished| (finished - command.started_at).num_milliseconds().max(0) as u64)
        .unwrap_or_default();
    CommandFact {
        command: format!("{} {}", command.executable, command.args.join(" "))
            .trim()
            .to_owned(),
        exit_code: command.exit_code,
        stdout: truncate(&command.stdout, 32_000),
        stderr: truncate(&command.stderr, 32_000),
        duration_ms,
        status: command.status,
    }
}

fn format_command_fact(fact: &CommandFact) -> String {
    format!(
        "{}: status={:?}, exit={:?}, duration_ms={}",
        fact.command, fact.status, fact.exit_code, fact.duration_ms
    )
}

fn is_test_command(command: &CommandExecution) -> bool {
    let executable = command
        .executable
        .trim_end_matches(".exe")
        .to_ascii_lowercase();
    executable == "pytest"
        || command
            .args
            .first()
            .is_some_and(|argument| argument.to_ascii_lowercase().contains("test"))
}

fn is_reviewer(role: &str) -> bool {
    let role = role.to_ascii_lowercase();
    role.contains("review") || role.contains("レビュー")
}

fn is_architect(role: &str) -> bool {
    let role = role.to_ascii_lowercase();
    role.contains("architect") || role.contains("アーキテクト")
}

fn parse_review_result(summary: &str) -> Option<ReviewResult> {
    let trimmed = summary.trim();
    let candidate = if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        &trimmed[start..=end]
    } else {
        trimmed
    };
    serde_json::from_str(candidate).ok()
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentAction, AgentTurnInput, ArdTeam, ArdWorkflow, BrainAssignment, CreateArdMember,
        CreateArdTeam, JsonDeveloperStore, TextReplacement, ToolCall, WorkspaceRegistry,
    };
    use chrono::Utc;
    use std::{collections::VecDeque, sync::Mutex as StdMutex};
    use tempfile::tempdir;

    struct ScriptedEngine {
        actions: StdMutex<VecDeque<AgentAction>>,
    }

    #[async_trait]
    impl DeveloperEngine for ScriptedEngine {
        fn name(&self) -> &str {
            "scripted"
        }
        async fn next_action(&self, _input: AgentTurnInput) -> Result<AgentAction, DeveloperError> {
            self.actions
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| DeveloperError::Model("script exhausted".to_owned()))
        }
    }

    struct ScriptedResolver;

    #[async_trait]
    impl ArdEngineResolver for ScriptedResolver {
        async fn resolve(
            &self,
            assignment: &ArdAssignment,
            context: &BrainResolutionContext,
        ) -> Result<ResolvedArdEngine, DeveloperError> {
            let actions = if is_reviewer(&assignment.member.role) {
                vec![
                    AgentAction::Plan {
                        steps: vec!["Review deterministic facts".to_owned()],
                        reason: "review".to_owned(),
                    },
                    AgentAction::Tool {
                        call: ToolCall::ReadFile {
                            path: "lib.rs".to_owned(),
                        },
                        rationale: "Inspect final source".to_owned(),
                    },
                    AgentAction::Complete {
                        summary: r#"{"approved":true,"issues":[],"severity":"LOW","required_changes":[],"reason":"build evidence passed"}"#.to_owned(),
                        confidence: 0.98,
                        reason: "verified".to_owned(),
                    },
                ]
            } else if assignment.member.role.contains("Developer") {
                vec![
                    AgentAction::Plan {
                        steps: vec!["Patch".to_owned(), "Build".to_owned()],
                        reason: "implement".to_owned(),
                    },
                    AgentAction::Tool {
                        call: ToolCall::ApplyPatch {
                            replacements: vec![TextReplacement {
                                path: "lib.rs".to_owned(),
                                expected: "pub fn value() -> u32 { 1 }".to_owned(),
                                replacement: "pub fn value() -> u32 { 2 }".to_owned(),
                                replace_all: false,
                            }],
                            reason: "safe change".to_owned(),
                        },
                        rationale: "Apply minimal diff".to_owned(),
                    },
                    AgentAction::Tool {
                        call: ToolCall::RunCommand {
                            executable: "cargo".to_owned(),
                            args: vec!["check".to_owned()],
                            working_directory: ".".to_owned(),
                            timeout_ms: 30_000,
                            purpose: "build".to_owned(),
                        },
                        rationale: "Compile actual source".to_owned(),
                    },
                    AgentAction::Tool {
                        call: ToolCall::RunCommand {
                            executable: "cargo".to_owned(),
                            args: vec!["test".to_owned()],
                            working_directory: ".".to_owned(),
                            timeout_ms: 30_000,
                            purpose: "test".to_owned(),
                        },
                        rationale: "Run actual tests".to_owned(),
                    },
                    AgentAction::Complete {
                        summary: "implementation complete".to_owned(),
                        confidence: 0.95,
                        reason: "rustc passed".to_owned(),
                    },
                ]
            } else {
                vec![
                    AgentAction::Plan {
                        steps: vec!["Inspect".to_owned()],
                        reason: "architecture".to_owned(),
                    },
                    AgentAction::Tool {
                        call: ToolCall::ReadFile {
                            path: "lib.rs".to_owned(),
                        },
                        rationale: "Read target".to_owned(),
                    },
                    AgentAction::Complete {
                        summary: "plan ready".to_owned(),
                        confidence: 0.9,
                        reason: "source inspected".to_owned(),
                    },
                ]
            };
            let brain = ResolvedBrain {
                provider_id: "test".to_owned(),
                model_id: "scripted".to_owned(),
                runtime_id: Some("test".to_owned()),
                temporary_auto_fallback: false,
            };
            Ok(ResolvedArdEngine {
                resolution: BrainResolution {
                    requested: "Auto".to_owned(),
                    resolved_brain: brain.clone(),
                    reason: "deterministic test resolver".to_owned(),
                    compatibility: "COMPATIBLE".to_owned(),
                    fallback_used: false,
                    required_capabilities: vec![assignment.member.role.clone()],
                    score: 100,
                    resource_policy: context.resource_policy,
                },
                brain,
                engine: Arc::new(ScriptedEngine {
                    actions: StdMutex::new(actions.into()),
                }),
            })
        }
    }

    struct ScriptedRuntime;

    #[async_trait]
    impl ArdRuntimeController for ScriptedRuntime {
        async fn rotate(
            &self,
            plan: &ModelRotationPlan,
            events: mpsc::UnboundedSender<ModelRotationEvent>,
            _control: watch::Receiver<RotationControl>,
        ) -> Result<ModelRotationResult, DeveloperError> {
            let kind = if plan.reuse_possible {
                ModelRotationEventKind::ModelReused
            } else {
                ModelRotationEventKind::ModelLoading
            };
            let event = ModelRotationEvent {
                kind,
                message: format!("prepared {}", plan.next_model),
            };
            let _ = events.send(event.clone());
            Ok(ModelRotationResult {
                plan: plan.clone(),
                attempts: 1,
                reused: plan.reuse_possible,
                actual_model_state: "LOADED".to_owned(),
                events: vec![event],
            })
        }
    }

    struct AlternativeResolver;

    #[async_trait]
    impl ArdEngineResolver for AlternativeResolver {
        async fn resolve(
            &self,
            _assignment: &ArdAssignment,
            context: &BrainResolutionContext,
        ) -> Result<ResolvedArdEngine, DeveloperError> {
            let model_id = if context.excluded_model_ids.contains("model-a") {
                "model-b"
            } else {
                "model-a"
            };
            let brain = ResolvedBrain {
                provider_id: "test".to_owned(),
                model_id: model_id.to_owned(),
                runtime_id: Some("test".to_owned()),
                temporary_auto_fallback: false,
            };
            Ok(ResolvedArdEngine {
                resolution: BrainResolution {
                    requested: "Auto".to_owned(),
                    resolved_brain: brain.clone(),
                    reason: format!("selected {model_id}"),
                    compatibility: "COMPATIBLE".to_owned(),
                    fallback_used: !context.excluded_model_ids.is_empty(),
                    required_capabilities: vec!["Reasoning".to_owned()],
                    score: if model_id == "model-a" { 100 } else { 90 },
                    resource_policy: context.resource_policy,
                },
                brain,
                engine: Arc::new(ScriptedEngine {
                    actions: StdMutex::new(VecDeque::from([AgentAction::Complete {
                        summary: "unused".to_owned(),
                        confidence: 1.0,
                        reason: "unused".to_owned(),
                    }])),
                }),
            })
        }
    }

    struct FailPrimaryRuntime;

    #[async_trait]
    impl ArdRuntimeController for FailPrimaryRuntime {
        async fn rotate(
            &self,
            plan: &ModelRotationPlan,
            events: mpsc::UnboundedSender<ModelRotationEvent>,
            _control: watch::Receiver<RotationControl>,
        ) -> Result<ModelRotationResult, DeveloperError> {
            if plan.next_model.ends_with("model-a") {
                return Err(DeveloperError::Model("simulated load failure".to_owned()));
            }
            let event = ModelRotationEvent {
                kind: ModelRotationEventKind::ModelRotationCompleted,
                message: "alternative loaded".to_owned(),
            };
            let _ = events.send(event.clone());
            Ok(ModelRotationResult {
                plan: plan.clone(),
                attempts: 1,
                reused: false,
                actual_model_state: "LOADED_OBSERVED".to_owned(),
                events: vec![event],
            })
        }
    }

    async fn setup() -> (
        tempfile::TempDir,
        Arc<ArdCoordinator>,
        ArdExecutionBridge,
        ArdTeam,
        ArdWorkflow,
    ) {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("lib.rs"), "pub fn value() -> u32 { 1 }\n").unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"bridge-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"lib.rs\"\n",
        )
        .unwrap();
        let registry =
            Arc::new(WorkspaceRegistry::open(dir.path().join("workspaces.json")).unwrap());
        let store = Arc::new(JsonDeveloperStore::open(dir.path().join("tasks.json")).unwrap());
        let developer = Arc::new(DeveloperCoordinator::new(registry, store));
        let workspace = developer
            .register_workspace("test", dir.path())
            .await
            .unwrap();
        let ard = Arc::new(ArdCoordinator::open(dir.path().join("ard.json")).unwrap());
        let team = ard
            .create_team(CreateArdTeam {
                name: "team".to_owned(),
                workspace_id: workspace.id,
                members: vec![
                    CreateArdMember {
                        name: "Alice".to_owned(),
                        role: "Architect".to_owned(),
                        brain: BrainAssignment::Auto,
                        permission: HardPermission::read_only(),
                        responsibilities: vec!["plan".to_owned()],
                        forbidden_actions: vec!["write".to_owned()],
                    },
                    CreateArdMember {
                        name: "Bob".to_owned(),
                        role: "Developer".to_owned(),
                        brain: BrainAssignment::Auto,
                        permission: HardPermission::developer(),
                        responsibilities: vec!["edit".to_owned()],
                        forbidden_actions: vec!["escape".to_owned()],
                    },
                    CreateArdMember {
                        name: "Carol".to_owned(),
                        role: "Reviewer".to_owned(),
                        brain: BrainAssignment::Auto,
                        permission: HardPermission::read_only(),
                        responsibilities: vec!["review".to_owned()],
                        forbidden_actions: vec!["write".to_owned()],
                    },
                ],
            })
            .unwrap();
        let workflow = ard.create_relay_workflow(team.id, "relay").unwrap();
        let bridge = ArdExecutionBridge::new(
            ard.clone(),
            developer,
            Arc::new(ScriptedResolver),
            Arc::new(ScriptedRuntime),
        )
        .with_poll_interval(Duration::from_millis(5));
        (dir, ard, bridge, team, workflow)
    }

    #[tokio::test]
    async fn bridge_runs_architect_developer_reviewer_to_completion() {
        let (dir, ard, bridge, _, workflow) = setup().await;
        let session = ard.start_session(workflow.id, "change value").unwrap();
        let completed = bridge.run_to_completion(session.id).await.unwrap();
        assert_eq!(
            completed.state,
            ArdSessionState::Completed,
            "{completed:#?}"
        );
        assert_eq!(completed.handoffs.len(), 3);
        assert_eq!(completed.executions.len(), 3);
        assert_eq!(completed.brain_resolutions.len(), 3);
        assert_eq!(completed.model_rotations.len(), 3);
        assert!(completed.model_rotations[1].reused_loaded_model);
        assert!(
            completed.handoffs[0]
                .files_read
                .contains(&"lib.rs".to_owned())
        );
        assert!(!completed.handoffs[1].build_results.is_empty());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("lib.rs")).unwrap(),
            "pub fn value() -> u32 { 2 }\n"
        );
    }

    #[tokio::test]
    async fn auto_rotation_retries_then_uses_one_policy_bounded_fallback() {
        let (_dir, ard, bridge, _, workflow) = setup().await;
        let session = ard.start_session(workflow.id, "fallback").unwrap();
        let assignment = ard.current_assignment(session.id).unwrap();
        let fallback_bridge = ArdExecutionBridge::new(
            ard.clone(),
            bridge.developer.clone(),
            Arc::new(AlternativeResolver),
            Arc::new(FailPrimaryRuntime),
        );
        let resolved = fallback_bridge
            .resolve_and_prepare(session.id, &assignment)
            .await
            .unwrap();
        assert_eq!(resolved.brain.model_id, "model-b");
        let persisted = ard.get_session(session.id).unwrap();
        assert_eq!(persisted.brain_resolutions.len(), 2);
        assert!(persisted.brain_resolutions[1].fallback_used);
        assert_eq!(persisted.model_rotations.len(), 2);
        assert_eq!(persisted.model_rotations[0].attempts, 2);
        assert_eq!(
            persisted.model_rotations[0].status,
            crate::ModelRotationStatus::Failed
        );
        assert_eq!(
            persisted.model_rotations[1].status,
            crate::ModelRotationStatus::Completed
        );
    }

    #[tokio::test]
    async fn explicit_brain_rotation_failure_never_falls_back_automatically() {
        let (_dir, ard, bridge, team, _) = setup().await;
        let explicit_team = ard
            .create_team(CreateArdTeam {
                name: "explicit".to_owned(),
                workspace_id: team.workspace_id,
                members: vec![CreateArdMember {
                    name: "Explicit Architect".to_owned(),
                    role: "Architect".to_owned(),
                    brain: BrainAssignment::Model {
                        provider_id: "test".to_owned(),
                        model_id: "model-a".to_owned(),
                        runtime_id: Some("test".to_owned()),
                    },
                    permission: HardPermission::read_only(),
                    responsibilities: vec!["plan".to_owned()],
                    forbidden_actions: Vec::new(),
                }],
            })
            .unwrap();
        let workflow = ard
            .create_relay_workflow(explicit_team.id, "explicit")
            .unwrap();
        let session = ard.start_session(workflow.id, "no fallback").unwrap();
        let assignment = ard.current_assignment(session.id).unwrap();
        let explicit_bridge = ArdExecutionBridge::new(
            ard.clone(),
            bridge.developer.clone(),
            Arc::new(AlternativeResolver),
            Arc::new(FailPrimaryRuntime),
        );
        assert!(
            explicit_bridge
                .resolve_and_prepare(session.id, &assignment)
                .await
                .is_err()
        );
        let persisted = ard.get_session(session.id).unwrap();
        assert_eq!(persisted.brain_resolutions.len(), 1);
        assert!(!persisted.brain_resolutions[0].fallback_used);
        assert_eq!(persisted.model_rotations.len(), 1);
    }

    #[test]
    fn deterministic_mapping_never_invents_command_facts() {
        let mut task = DeveloperTask::new(
            WorkspaceId::new_v4(),
            "test",
            DeveloperMode::Auto,
            "test/model",
        );
        task.state = DeveloperTaskState::Completed;
        task.commands.push(CommandExecution {
            id: DeveloperTaskId::new_v4(),
            executable: "cargo".to_owned(),
            args: vec!["test".to_owned()],
            working_directory: ".".to_owned(),
            process_id: None,
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            timeout_ms: 1000,
            exit_code: Some(0),
            stdout: "test result: ok. 2 passed; 0 failed; 1 ignored".to_owned(),
            stderr: String::new(),
            status: CommandStatus::Completed,
        });
        let result = map_execution_result(&task);
        assert_eq!(result.test_results.len(), 1);
        assert_eq!(result.test_results[0].result.passed, 2);
        assert_eq!(result.test_results[0].result.failed, 0);
        assert_eq!(result.test_results[0].command.exit_code, Some(0));
    }

    #[test]
    fn reviewer_schema_is_strictly_typed() {
        let review = parse_review_result(r#"{"approved":false,"issues":["missing test"],"severity":"HIGH","required_changes":["add test"],"reason":"evidence absent"}"#).unwrap();
        assert!(!review.approved);
        assert_eq!(review.required_changes, vec!["add test"]);
        assert!(parse_review_result("looks good").is_none());
    }
}
