use crate::agent::run_guard::RunStopReason;
use crate::agent::runtime::kernel::execution_plan::ExecutionLane;
use crate::agent::runtime::kernel::session_profile::SessionSurfaceKind;
use crate::agent::runtime::kernel::turn_state::{TurnCompactionBoundary, TurnStateSnapshot};
use crate::agent::runtime::skill_routing::observability::route_fallback_reason_key;
use crate::agent::runtime::task_record::TaskLifecycleStatus;
use crate::agent::runtime::task_repo::{
    TaskRecordUpsertPayload, TaskRepo, TaskStatusChangedPayload,
};
use crate::agent::runtime::{
    effective_tool_set::EffectiveToolDecisionRecord, RunRegistry, RuntimeObservability,
    RuntimeObservedEvent, RuntimeObservedRunEvent,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone)]
pub struct SessionJournalStore {
    root: PathBuf,
    run_registry: Arc<RunRegistry>,
    observability: Arc<RuntimeObservability>,
}

impl SessionJournalStore {
    pub fn new(root: PathBuf) -> Self {
        Self::with_registry_and_observability(
            root,
            Arc::new(RunRegistry::default()),
            Arc::new(RuntimeObservability::default()),
        )
    }

    pub fn with_registry(root: PathBuf, run_registry: Arc<RunRegistry>) -> Self {
        Self::with_registry_and_observability(
            root,
            run_registry,
            Arc::new(RuntimeObservability::default()),
        )
    }

    pub fn with_registry_and_observability(
        root: PathBuf,
        run_registry: Arc<RunRegistry>,
        observability: Arc<RuntimeObservability>,
    ) -> Self {
        Self {
            root,
            run_registry,
            observability,
        }
    }

    pub fn observability(&self) -> Arc<RuntimeObservability> {
        Arc::clone(&self.observability)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub async fn append_event(
        &self,
        session_id: &str,
        event: SessionRunEvent,
    ) -> Result<(), String> {
        let session_dir = self.session_dir(session_id);
        fs::create_dir_all(&session_dir)
            .await
            .map_err(|e| format!("创建 session journal 目录失败: {e}"))?;

        let record = SessionJournalRecord {
            session_id: session_id.to_string(),
            recorded_at: Utc::now().to_rfc3339(),
            event: event.clone(),
        };

        let mut events_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(session_dir.join("events.jsonl"))
            .await
            .map_err(|e| format!("打开 session events 文件失败: {e}"))?;
        let line = serde_json::to_string(&record)
            .map_err(|e| format!("序列化 session event 失败: {e}"))?;
        events_file
            .write_all(format!("{line}\n").as_bytes())
            .await
            .map_err(|e| format!("写入 session event 失败: {e}"))?;
        events_file
            .flush()
            .await
            .map_err(|e| format!("刷新 session event 失败: {e}"))?;

        let mut state = self.read_state(session_id).await?;
        apply_event(&mut state, &event);
        self.run_registry.apply_event(session_id, &event);
        self.observability
            .record_recent_event(build_observed_session_run_event(
                session_id,
                &record.recorded_at,
                &event,
            ));
        state.current_run_id = self
            .run_registry
            .restore_session(session_id, state.current_run_id.as_deref());
        let state_json = serde_json::to_string_pretty(&state)
            .map_err(|e| format!("序列化 session state 失败: {e}"))?;
        fs::write(session_dir.join("state.json"), state_json)
            .await
            .map_err(|e| format!("写入 session state 失败: {e}"))?;

        let transcript = render_transcript_markdown(&state);
        fs::write(session_dir.join("transcript.md"), transcript)
            .await
            .map_err(|e| format!("写入 session transcript 失败: {e}"))?;

        Ok(())
    }

    pub async fn read_state(&self, session_id: &str) -> Result<SessionJournalState, String> {
        let path = self.session_dir(session_id).join("state.json");
        if !path.exists() {
            return Ok(SessionJournalState {
                session_id: session_id.to_string(),
                current_run_id: self.run_registry.active_run_id(session_id),
                ..SessionJournalState::default()
            });
        }

        let raw = fs::read_to_string(&path)
            .await
            .map_err(|e| format!("读取 session state 失败: {e}"))?;
        let mut state = serde_json::from_str::<SessionJournalState>(&raw)
            .map_err(|e| format!("解析 session state 失败: {e}"))?;
        if state.session_id.trim().is_empty() {
            state.session_id = session_id.to_string();
        }
        state.current_run_id = self.run_registry.restore_session(
            session_id,
            normalize_current_run_id(state.current_run_id.as_deref()),
        );
        Ok(state)
    }

    fn session_dir(&self, session_id: &str) -> PathBuf {
        self.root.join(session_id)
    }
}

#[derive(Debug, Clone)]
pub struct SessionJournalStateHandle(pub Arc<SessionJournalStore>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionJournalState {
    pub session_id: String,
    pub current_run_id: Option<String>,
    pub runs: Vec<SessionRunSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<SessionTaskRecordSnapshot>,
}

impl Default for SessionJournalState {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            current_run_id: None,
            runs: Vec::new(),
            tasks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRunSnapshot {
    pub run_id: String,
    pub user_message_id: String,
    pub status: SessionRunStatus,
    pub buffered_text: String,
    pub last_error_kind: Option<String>,
    pub last_error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_identity: Option<SessionRunTaskIdentitySnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_continuation_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_continuation_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_state: Option<SessionRunTurnStateSnapshot>,
}

impl SessionRunSnapshot {
    fn new(run_id: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            user_message_id: String::new(),
            status: SessionRunStatus::Queued,
            buffered_text: String::new(),
            last_error_kind: None,
            last_error_message: None,
            task_identity: None,
            task_continuation_mode: None,
            task_continuation_reason: None,
            turn_state: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRunTurnStateCompactionBoundary {
    pub transcript_path: String,
    pub original_tokens: usize,
    pub compacted_tokens: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRunTaskIdentitySnapshot {
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    pub root_task_id: String,
    pub task_kind: String,
    pub surface_kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub backend_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionTaskRecordSnapshot {
    pub task_identity: SessionRunTaskIdentitySnapshot,
    pub session_id: String,
    pub user_message_id: String,
    pub run_id: String,
    pub status: TaskLifecycleStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_reason: Option<String>,
}

impl From<&TaskRecordUpsertPayload> for SessionTaskRecordSnapshot {
    fn from(value: &TaskRecordUpsertPayload) -> Self {
        Self {
            task_identity: SessionRunTaskIdentitySnapshot {
                task_id: value.task_identity.task_id.clone(),
                parent_task_id: value.task_identity.parent_task_id.clone(),
                root_task_id: value.task_identity.root_task_id.clone(),
                task_kind: value.task_kind.journal_key().to_string(),
                surface_kind: value.surface_kind.journal_key().to_string(),
                backend_kind: value.backend_kind.journal_key().to_string(),
            },
            session_id: value.session_id.clone(),
            user_message_id: value.user_message_id.clone(),
            run_id: value.run_id.clone(),
            status: value.status,
            created_at: value.created_at.clone(),
            updated_at: value.updated_at.clone(),
            started_at: value.started_at.clone(),
            completed_at: value.completed_at.clone(),
            terminal_reason: value.terminal_reason.clone(),
        }
    }
}

impl From<&TurnCompactionBoundary> for SessionRunTurnStateCompactionBoundary {
    fn from(value: &TurnCompactionBoundary) -> Self {
        Self {
            transcript_path: value.transcript_path.clone(),
            original_tokens: value.original_tokens,
            compacted_tokens: value.compacted_tokens,
            summary: value.summary.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRunTurnStateSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_identity: Option<SessionRunTaskIdentitySnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_surface: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_lane: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_runner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_skill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invoked_skills: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub partial_assistant_text: String,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub tool_failure_streak: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconstructed_history_len: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction_boundary: Option<SessionRunTurnStateCompactionBoundary>,
}

impl From<&TurnStateSnapshot> for SessionRunTurnStateSnapshot {
    fn from(value: &TurnStateSnapshot) -> Self {
        Self {
            task_identity: value.task_identity.as_ref().map(|identity| {
                SessionRunTaskIdentitySnapshot {
                    task_id: identity.task_id.clone(),
                    parent_task_id: identity.parent_task_id.clone(),
                    root_task_id: identity.root_task_id.clone(),
                    task_kind: value
                        .task_kind
                        .map(|kind| kind.journal_key().to_string())
                        .unwrap_or_default(),
                    surface_kind: value
                        .task_surface
                        .map(|surface| surface.journal_key().to_string())
                        .unwrap_or_default(),
                    backend_kind: value
                        .task_backend
                        .map(|backend| backend.journal_key().to_string())
                        .unwrap_or_default(),
                }
            }),
            session_surface: value.session_surface.map(session_surface_key),
            execution_lane: value.execution_lane.map(execution_lane_key),
            selected_runner: value
                .route_observation
                .as_ref()
                .map(|observation| observation.selected_runner.clone()),
            selected_skill: value
                .route_observation
                .as_ref()
                .and_then(|observation| observation.selected_skill.clone())
                .or_else(|| value.invoked_skills.first().cloned()),
            fallback_reason: value.route_observation.as_ref().and_then(|observation| {
                observation
                    .fallback_reason
                    .map(|reason| route_fallback_reason_key(reason).to_string())
            }),
            allowed_tools: value.allowed_tools.clone(),
            invoked_skills: value.invoked_skills.clone(),
            partial_assistant_text: value.partial_assistant_text.clone(),
            tool_failure_streak: value.tool_failure_streak,
            reconstructed_history_len: value.reconstructed_history_len,
            compaction_boundary: value.compaction_boundary.as_ref().map(Into::into),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionRunStatus {
    Queued,
    Thinking,
    ToolCalling,
    WaitingApproval,
    WaitingUser,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionRunEvent {
    TaskContinued {
        run_id: String,
        task_identity: SessionRunTaskIdentitySnapshot,
        continuation_mode: String,
        continuation_reason: String,
    },
    TaskStateProjected {
        run_id: String,
        task_identity: SessionRunTaskIdentitySnapshot,
    },
    TaskDelegated {
        run_id: String,
        from_task_id: String,
        from_task_kind: String,
        from_surface_kind: String,
        delegated_task: SessionRunTaskIdentitySnapshot,
    },
    TaskReturned {
        run_id: String,
        to_task_id: String,
        to_task_kind: String,
        to_surface_kind: String,
        returned_task: SessionRunTaskIdentitySnapshot,
        returned_status: TaskLifecycleStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        terminal_reason: Option<String>,
    },
    TaskRecordUpserted {
        run_id: String,
        task: TaskRecordUpsertPayload,
    },
    TaskStatusChanged {
        run_id: String,
        status_change: TaskStatusChangedPayload,
    },
    RunStarted {
        run_id: String,
        user_message_id: String,
    },
    SkillRouteRecorded {
        run_id: String,
        route_latency_ms: u64,
        candidate_count: usize,
        selected_runner: String,
        selected_skill: Option<String>,
        fallback_reason: Option<String>,
        tool_recommendation_summary: Option<String>,
        tool_recommendation_aligned: Option<bool>,
        tool_plan_summary: Option<EffectiveToolDecisionRecord>,
    },
    AssistantChunkAppended {
        run_id: String,
        chunk: String,
    },
    ToolStarted {
        run_id: String,
        tool_name: String,
        call_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task_identity: Option<SessionRunTaskIdentitySnapshot>,
        input: Value,
    },
    ToolCompleted {
        run_id: String,
        tool_name: String,
        call_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task_identity: Option<SessionRunTaskIdentitySnapshot>,
        input: Value,
        output: String,
        is_error: bool,
    },
    ApprovalRequested {
        run_id: String,
        approval_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task_identity: Option<SessionRunTaskIdentitySnapshot>,
        tool_name: String,
        call_id: String,
        input: Value,
        summary: String,
        impact: Option<String>,
        irreversible: bool,
    },
    RunCompleted {
        run_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turn_state: Option<SessionRunTurnStateSnapshot>,
    },
    RunGuardWarning {
        run_id: String,
        warning_kind: String,
        title: String,
        message: String,
        detail: Option<String>,
        last_completed_step: Option<String>,
    },
    RunStopped {
        run_id: String,
        stop_reason: RunStopReason,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turn_state: Option<SessionRunTurnStateSnapshot>,
    },
    RunFailed {
        run_id: String,
        error_kind: String,
        error_message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turn_state: Option<SessionRunTurnStateSnapshot>,
    },
    RunCancelled {
        run_id: String,
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionJournalRecord {
    pub session_id: String,
    pub recorded_at: String,
    #[serde(flatten)]
    pub event: SessionRunEvent,
}

fn apply_event(state: &mut SessionJournalState, event: &SessionRunEvent) {
    let run_id = match event {
        SessionRunEvent::TaskContinued { run_id, .. }
        | SessionRunEvent::TaskStateProjected { run_id, .. }
        | SessionRunEvent::TaskDelegated { run_id, .. }
        | SessionRunEvent::TaskReturned { run_id, .. }
        | SessionRunEvent::TaskRecordUpserted { run_id, .. }
        | SessionRunEvent::TaskStatusChanged { run_id, .. }
        | SessionRunEvent::RunStarted { run_id, .. }
        | SessionRunEvent::SkillRouteRecorded { run_id, .. }
        | SessionRunEvent::AssistantChunkAppended { run_id, .. }
        | SessionRunEvent::ToolStarted { run_id, .. }
        | SessionRunEvent::ToolCompleted { run_id, .. }
        | SessionRunEvent::ApprovalRequested { run_id, .. }
        | SessionRunEvent::RunCompleted { run_id, .. }
        | SessionRunEvent::RunGuardWarning { run_id, .. }
        | SessionRunEvent::RunStopped { run_id, .. }
        | SessionRunEvent::RunFailed { run_id, .. }
        | SessionRunEvent::RunCancelled { run_id, .. } => run_id.clone(),
    };
    let run_index = upsert_run_index(state, &run_id);

    match event {
        SessionRunEvent::TaskContinued {
            task_identity,
            continuation_mode,
            continuation_reason,
            ..
        } => {
            state.runs[run_index].task_identity = Some(task_identity.clone());
            state.runs[run_index].task_continuation_mode = Some(continuation_mode.clone());
            state.runs[run_index].task_continuation_reason = Some(continuation_reason.clone());
        }
        SessionRunEvent::TaskStateProjected { task_identity, .. } => {
            state.runs[run_index].task_identity = Some(task_identity.clone());
        }
        SessionRunEvent::TaskDelegated { .. } | SessionRunEvent::TaskReturned { .. } => {}
        SessionRunEvent::TaskRecordUpserted { task, .. } => {
            project_task_record_upsert(state, task);
        }
        SessionRunEvent::TaskStatusChanged { status_change, .. } => {
            project_task_status_change(state, status_change);
        }
        SessionRunEvent::RunStarted {
            run_id,
            user_message_id,
        } => {
            state.current_run_id = Some(run_id.clone());
            let run = &mut state.runs[run_index];
            run.user_message_id = user_message_id.clone();
            run.status = SessionRunStatus::Thinking;
            run.last_error_kind = None;
            run.last_error_message = None;
            run.task_continuation_mode = None;
            run.task_continuation_reason = None;
            run.turn_state = None;
        }
        SessionRunEvent::SkillRouteRecorded { .. } => {}
        SessionRunEvent::AssistantChunkAppended { chunk, .. } => {
            let run = &mut state.runs[run_index];
            run.buffered_text.push_str(chunk);
            if matches!(run.status, SessionRunStatus::Queued) {
                run.status = SessionRunStatus::Thinking;
            }
        }
        SessionRunEvent::ToolStarted { task_identity, .. } => {
            let run = &mut state.runs[run_index];
            run.status = SessionRunStatus::ToolCalling;
            if let Some(task_identity) = task_identity {
                run.task_identity = Some(task_identity.clone());
            }
        }
        SessionRunEvent::ToolCompleted { task_identity, .. } => {
            let run = &mut state.runs[run_index];
            run.status = SessionRunStatus::Thinking;
            if let Some(task_identity) = task_identity {
                run.task_identity = Some(task_identity.clone());
            }
        }
        SessionRunEvent::ApprovalRequested { task_identity, .. } => {
            let run = &mut state.runs[run_index];
            run.status = SessionRunStatus::WaitingApproval;
            if let Some(task_identity) = task_identity {
                run.task_identity = Some(task_identity.clone());
            }
        }
        SessionRunEvent::RunCompleted { run_id, turn_state } => {
            state.runs[run_index].status = SessionRunStatus::Completed;
            state.runs[run_index].turn_state = turn_state.clone();
            if state.current_run_id.as_deref() == Some(run_id.as_str()) {
                state.current_run_id = None;
            }
        }
        SessionRunEvent::RunGuardWarning { .. } => {}
        SessionRunEvent::RunStopped {
            run_id,
            stop_reason,
            turn_state,
        } => {
            if state.current_run_id.as_deref() == Some(run_id.as_str()) {
                state.current_run_id = None;
            }
            let run = &mut state.runs[run_index];
            run.status = SessionRunStatus::Failed;
            run.last_error_kind = Some(stop_reason.kind.as_key().to_string());
            run.last_error_message = Some(format_run_stop_message(stop_reason));
            run.turn_state = turn_state.clone();
        }
        SessionRunEvent::RunFailed {
            run_id,
            error_kind,
            error_message,
            turn_state,
        } => {
            if state.current_run_id.as_deref() == Some(run_id.as_str()) {
                state.current_run_id = None;
            }
            let run = &mut state.runs[run_index];
            run.status = SessionRunStatus::Failed;
            run.last_error_kind = Some(error_kind.clone());
            run.last_error_message = Some(error_message.clone());
            run.turn_state = turn_state.clone();
        }
        SessionRunEvent::RunCancelled { run_id, reason } => {
            if state.current_run_id.as_deref() == Some(run_id.as_str()) {
                state.current_run_id = None;
            }
            let run = &mut state.runs[run_index];
            run.status = SessionRunStatus::Cancelled;
            run.last_error_kind = Some("cancelled".to_string());
            run.last_error_message = reason.clone();
        }
    }
}

fn execution_lane_key(lane: ExecutionLane) -> String {
    match lane {
        ExecutionLane::OpenTask => "open_task".to_string(),
        ExecutionLane::PromptInline => "prompt_inline".to_string(),
        ExecutionLane::PromptFork => "prompt_fork".to_string(),
        ExecutionLane::DirectDispatch => "direct_dispatch".to_string(),
    }
}

fn session_surface_key(surface: SessionSurfaceKind) -> String {
    match surface {
        SessionSurfaceKind::LocalChat => "local_chat".to_string(),
        SessionSurfaceKind::HiddenChildSession => "hidden_child_session".to_string(),
        SessionSurfaceKind::EmployeeStepSession => "employee_step_session".to_string(),
    }
}

fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}

fn upsert_run_index(state: &mut SessionJournalState, run_id: &str) -> usize {
    if let Some(index) = state.runs.iter().position(|run| run.run_id == run_id) {
        return index;
    }
    state.runs.push(SessionRunSnapshot::new(run_id));
    state.runs.len() - 1
}

fn upsert_task_record_index(state: &mut SessionJournalState, task_id: &str) -> usize {
    if let Some(index) = state
        .tasks
        .iter()
        .position(|task| task.task_identity.task_id == task_id)
    {
        return index;
    }
    state.tasks.push(SessionTaskRecordSnapshot {
        task_identity: SessionRunTaskIdentitySnapshot {
            task_id: task_id.to_string(),
            parent_task_id: None,
            root_task_id: task_id.to_string(),
            task_kind: String::new(),
            surface_kind: String::new(),
            backend_kind: String::new(),
        },
        session_id: String::new(),
        user_message_id: String::new(),
        run_id: String::new(),
        status: TaskLifecycleStatus::Pending,
        created_at: String::new(),
        updated_at: String::new(),
        started_at: None,
        completed_at: None,
        terminal_reason: None,
    });
    state.tasks.len() - 1
}

fn project_task_record_upsert(state: &mut SessionJournalState, task: &TaskRecordUpsertPayload) {
    let task_index = upsert_task_record_index(state, &task.task_identity.task_id);
    state.tasks[task_index] = SessionTaskRecordSnapshot::from(task);
}

fn project_task_status_change(
    state: &mut SessionJournalState,
    status_change: &TaskStatusChangedPayload,
) {
    if let Some(task_index) = state
        .tasks
        .iter()
        .position(|task| task.task_identity.task_id == status_change.task_id)
    {
        TaskRepo::apply_task_status_change(&mut state.tasks[task_index], status_change);
    }
}

fn normalize_current_run_id(run_id: Option<&str>) -> Option<&str> {
    run_id.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn format_run_stop_message(stop_reason: &RunStopReason) -> String {
    let mut lines = vec![stop_reason.message.clone()];
    if let Some(detail) = stop_reason.detail.as_deref() {
        if !detail.trim().is_empty() && detail != stop_reason.message {
            lines.push(detail.to_string());
        }
    }
    if let Some(step) = stop_reason.last_completed_step.as_deref() {
        if !step.trim().is_empty() {
            lines.push(format!("最后完成步骤：{step}"));
        }
    }
    lines.join("\n")
}

fn build_observed_session_run_event(
    session_id: &str,
    recorded_at: &str,
    event: &SessionRunEvent,
) -> RuntimeObservedEvent {
    let observed = match event {
        SessionRunEvent::TaskContinued {
            run_id,
            task_identity,
            continuation_mode,
            continuation_reason,
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "task_continued".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("continued".to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: Some(task_identity.task_kind.clone()),
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(
                json!({
                    "task_id": task_identity.task_id,
                    "parent_task_id": task_identity.parent_task_id,
                    "root_task_id": task_identity.root_task_id,
                    "task_kind": task_identity.task_kind,
                    "surface_kind": task_identity.surface_kind,
                    "backend_kind": task_identity.backend_kind,
                    "continuation_mode": continuation_mode,
                    "continuation_reason": continuation_reason,
                })
                .to_string(),
            ),
        },
        SessionRunEvent::TaskStateProjected {
            run_id,
            task_identity,
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "task_state_projected".to_string(),
            created_at: recorded_at.to_string(),
            status: None,
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(
                json!({
                    "task_id": task_identity.task_id,
                    "parent_task_id": task_identity.parent_task_id,
                    "root_task_id": task_identity.root_task_id,
                    "task_kind": task_identity.task_kind,
                    "surface_kind": task_identity.surface_kind,
                    "backend_kind": task_identity.backend_kind,
                })
                .to_string(),
            ),
        },
        SessionRunEvent::TaskDelegated {
            run_id,
            from_task_id,
            from_task_kind,
            from_surface_kind,
            delegated_task,
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "task_delegated".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("delegated".to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: Some(delegated_task.task_kind.clone()),
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(
                json!({
                    "from_task_id": from_task_id,
                    "from_task_kind": from_task_kind,
                    "from_surface_kind": from_surface_kind,
                    "delegated_task_id": delegated_task.task_id,
                    "delegated_parent_task_id": delegated_task.parent_task_id,
                    "delegated_root_task_id": delegated_task.root_task_id,
                    "delegated_task_kind": delegated_task.task_kind,
                    "delegated_surface_kind": delegated_task.surface_kind,
                    "delegated_backend_kind": delegated_task.backend_kind,
                })
                .to_string(),
            ),
        },
        SessionRunEvent::TaskReturned {
            run_id,
            to_task_id,
            to_task_kind,
            to_surface_kind,
            returned_task,
            returned_status,
            terminal_reason,
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "task_returned".to_string(),
            created_at: recorded_at.to_string(),
            status: Some(returned_status.as_key().to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: Some(returned_task.task_kind.clone()),
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(
                json!({
                    "to_task_id": to_task_id,
                    "to_task_kind": to_task_kind,
                    "to_surface_kind": to_surface_kind,
                    "returned_task_id": returned_task.task_id,
                    "returned_parent_task_id": returned_task.parent_task_id,
                    "returned_root_task_id": returned_task.root_task_id,
                    "returned_task_kind": returned_task.task_kind,
                    "returned_surface_kind": returned_task.surface_kind,
                    "returned_backend_kind": returned_task.backend_kind,
                    "returned_status": returned_status,
                    "terminal_reason": terminal_reason,
                })
                .to_string(),
            ),
        },
        SessionRunEvent::TaskRecordUpserted { run_id, task } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "task_record_upserted".to_string(),
            created_at: recorded_at.to_string(),
            status: Some(task.status.as_key().to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(
                json!({
                    "task_id": task.task_identity.task_id,
                    "parent_task_id": task.task_identity.parent_task_id,
                    "root_task_id": task.task_identity.root_task_id,
                    "task_kind": task.task_kind.journal_key(),
                    "surface_kind": task.surface_kind.journal_key(),
                    "status": task.status,
                })
                .to_string(),
            ),
        },
        SessionRunEvent::TaskStatusChanged {
            run_id,
            status_change,
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "task_status_changed".to_string(),
            created_at: recorded_at.to_string(),
            status: Some(status_change.to_status.as_key().to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(
                json!({
                    "task_id": status_change.task_id,
                    "parent_task_id": status_change.parent_task_id,
                    "root_task_id": status_change.root_task_id,
                    "from_status": status_change.from_status,
                    "to_status": status_change.to_status,
                    "terminal_reason": status_change.terminal_reason,
                })
                .to_string(),
            ),
        },
        SessionRunEvent::RunStarted {
            run_id,
            user_message_id,
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "run_started".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("thinking".to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(format!("user_message_id={user_message_id}")),
        },
        SessionRunEvent::SkillRouteRecorded {
            run_id,
            route_latency_ms,
            candidate_count,
            selected_runner,
            selected_skill,
            fallback_reason,
            tool_recommendation_summary,
            tool_recommendation_aligned,
            tool_plan_summary,
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "skill_route_recorded".to_string(),
            created_at: recorded_at.to_string(),
            status: Some(selected_runner.clone()),
            tool_name: None,
            approval_id: None,
            warning_kind: fallback_reason.clone(),
            error_kind: None,
            child_session_id: None,
            route_latency_ms: Some(*route_latency_ms),
            candidate_count: Some(*candidate_count),
            selected_skill: selected_skill.clone(),
            fallback_reason: fallback_reason.clone(),
            tool_recommendation_summary: tool_recommendation_summary.clone(),
            tool_recommendation_aligned: *tool_recommendation_aligned,
            tool_plan_summary: tool_plan_summary.clone(),
            message: Some(truncate_observed_message(
                &json!({
                    "route_latency_ms": route_latency_ms,
                    "candidate_count": candidate_count,
                    "selected_runner": selected_runner,
                    "selected_skill": selected_skill,
                    "fallback_reason": fallback_reason,
                    "tool_recommendation_summary": tool_recommendation_summary,
                    "tool_recommendation_aligned": tool_recommendation_aligned,
                    "tool_plan_summary": tool_plan_summary,
                })
                .to_string(),
            )),
        },
        SessionRunEvent::AssistantChunkAppended { run_id, chunk } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "assistant_chunk_appended".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("thinking".to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(truncate_observed_message(chunk)),
        },
        SessionRunEvent::ToolStarted {
            run_id,
            tool_name,
            input,
            ..
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "tool_started".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("tool_calling".to_string()),
            tool_name: Some(tool_name.clone()),
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: observed_child_session_id(input),
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(truncate_observed_message(&input.to_string())),
        },
        SessionRunEvent::ToolCompleted {
            run_id,
            tool_name,
            input,
            output,
            is_error,
            ..
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "tool_completed".to_string(),
            created_at: recorded_at.to_string(),
            status: Some(if *is_error {
                "tool_error".to_string()
            } else {
                "thinking".to_string()
            }),
            tool_name: Some(tool_name.clone()),
            approval_id: None,
            warning_kind: None,
            error_kind: (*is_error).then_some("tool_error".to_string()),
            child_session_id: observed_child_session_id(input),
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(truncate_observed_message(output)),
        },
        SessionRunEvent::ApprovalRequested {
            run_id,
            approval_id,
            tool_name,
            input,
            summary,
            ..
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "approval_requested".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("waiting_approval".to_string()),
            tool_name: Some(tool_name.clone()),
            approval_id: Some(approval_id.clone()),
            warning_kind: None,
            error_kind: None,
            child_session_id: observed_child_session_id(input),
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(truncate_observed_message(summary)),
        },
        SessionRunEvent::RunCompleted { run_id, .. } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "run_completed".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("completed".to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some("run completed".to_string()),
        },
        SessionRunEvent::RunGuardWarning {
            run_id,
            warning_kind,
            title,
            detail,
            ..
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "run_guard_warning".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("warning".to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: Some(warning_kind.clone()),
            error_kind: None,
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(detail.clone().unwrap_or_else(|| title.clone())),
        },
        SessionRunEvent::RunStopped {
            run_id,
            stop_reason,
            ..
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "run_stopped".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("failed".to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: Some(stop_reason.kind.as_key().to_string()),
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(truncate_observed_message(&stop_reason.message)),
        },
        SessionRunEvent::RunFailed {
            run_id,
            error_kind,
            error_message,
            ..
        } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "run_failed".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("failed".to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: Some(error_kind.clone()),
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: Some(truncate_observed_message(error_message)),
        },
        SessionRunEvent::RunCancelled { run_id, reason } => RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.clone(),
            event_type: "run_cancelled".to_string(),
            created_at: recorded_at.to_string(),
            status: Some("cancelled".to_string()),
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: Some("cancelled".to_string()),
            child_session_id: None,
            route_latency_ms: None,
            candidate_count: None,
            selected_skill: None,
            fallback_reason: None,
            tool_recommendation_summary: None,
            tool_recommendation_aligned: None,
            tool_plan_summary: None,
            message: reason.clone(),
        },
    };

    RuntimeObservedEvent::SessionRun(observed)
}

fn observed_child_session_id(input: &Value) -> Option<String> {
    input
        .get("child_session_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn truncate_observed_message(value: &str) -> String {
    let mut truncated = value.chars().take(160).collect::<String>();
    if value.chars().count() > 160 {
        truncated.push_str("...");
    }
    truncated
}

fn render_transcript_markdown(state: &SessionJournalState) -> String {
    let mut lines = vec![format!("# Session {}", state.session_id), String::new()];

    for run in &state.runs {
        lines.push(format!("## Run {}", run.run_id));
        lines.push(format!("- status: {}", run.status.as_str()));
        if !run.user_message_id.trim().is_empty() {
            lines.push(format!("- user_message_id: {}", run.user_message_id));
        }
        if let Some(error_kind) = &run.last_error_kind {
            if !error_kind.trim().is_empty() {
                lines.push(format!("- error_kind: {}", error_kind));
            }
        }
        if let Some(error_message) = &run.last_error_message {
            if !error_message.trim().is_empty() {
                lines.push(format!("- error_message: {}", error_message));
            }
        }
        lines.push(String::new());
        if !run.buffered_text.trim().is_empty() {
            lines.push("```text".to_string());
            lines.push(run.buffered_text.clone());
            lines.push("```".to_string());
            lines.push(String::new());
        }
    }

    lines.join("\n")
}

impl SessionRunStatus {
    fn as_str(&self) -> &'static str {
        match self {
            SessionRunStatus::Queued => "queued",
            SessionRunStatus::Thinking => "thinking",
            SessionRunStatus::ToolCalling => "tool_calling",
            SessionRunStatus::WaitingApproval => "waiting_approval",
            SessionRunStatus::WaitingUser => "waiting_user",
            SessionRunStatus::Completed => "completed",
            SessionRunStatus::Failed => "failed",
            SessionRunStatus::Cancelled => "cancelled",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_run_stop_message, SessionJournalState, SessionJournalStore, SessionRunEvent,
        SessionRunTaskIdentitySnapshot, SessionRunTurnStateCompactionBoundary,
        SessionRunTurnStateSnapshot,
    };
    use crate::agent::run_guard::RunStopReason;
    use crate::agent::runtime::effective_tool_set::{
        EffectiveToolDecisionRecord, EffectiveToolExclusion, EffectiveToolPolicySummary,
        EffectiveToolReasonCount, EffectiveToolSetSource, EffectiveToolSourceCount,
        ToolFilterReason, ToolLoadingPolicy,
    };
    use crate::agent::runtime::kernel::execution_plan::ExecutionLane;
    use crate::agent::runtime::kernel::session_profile::SessionSurfaceKind;
    use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
    use crate::agent::runtime::task_record::TaskLifecycleStatus;
    use crate::agent::runtime::task_repo::{TaskRecordUpsertPayload, TaskStatusChangedPayload};
    use crate::agent::runtime::{RunRegistry, RuntimeObservability, RuntimeObservedEvent};
    use crate::agent::tool_manifest::ToolSource;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn format_run_stop_message_preserves_policy_blocked_detail() {
        let reason = RunStopReason::policy_blocked(
            "目标路径不在当前工作目录范围内。你可以先切换当前会话的工作目录后重试。",
        )
        .with_last_completed_step("已读取当前工作区");

        let formatted = format_run_stop_message(&reason);

        assert!(formatted.contains("本次请求触发了安全或工作区限制"));
        assert!(formatted
            .contains("目标路径不在当前工作目录范围内。你可以先切换当前会话的工作目录后重试。"));
        assert!(formatted.contains("最后完成步骤：已读取当前工作区"));
    }

    #[tokio::test]
    async fn read_state_recovers_active_run_id_from_legacy_snapshot() {
        let journal_root = tempdir().expect("journal tempdir");
        let session_dir = journal_root.path().join("session-legacy");
        tokio::fs::create_dir_all(&session_dir)
            .await
            .expect("create session dir");
        let state = SessionJournalState {
            session_id: "session-legacy".to_string(),
            current_run_id: Some("run-legacy".to_string()),
            runs: vec![],
            tasks: vec![],
        };
        let state_json = serde_json::to_string_pretty(&state).expect("serialize state");
        tokio::fs::write(session_dir.join("state.json"), state_json)
            .await
            .expect("write state");

        let registry = Arc::new(RunRegistry::default());
        let journal =
            SessionJournalStore::with_registry(journal_root.path().to_path_buf(), registry.clone());

        let recovered = journal
            .read_state("session-legacy")
            .await
            .expect("read state");

        assert_eq!(recovered.current_run_id.as_deref(), Some("run-legacy"));
        assert_eq!(
            registry.active_run_id("session-legacy").as_deref(),
            Some("run-legacy")
        );
    }

    #[tokio::test]
    async fn append_event_keeps_registry_aligned_with_terminal_state() {
        let journal_root = tempdir().expect("journal tempdir");
        let registry = Arc::new(RunRegistry::default());
        let journal =
            SessionJournalStore::with_registry(journal_root.path().to_path_buf(), registry.clone());

        journal
            .append_event(
                "session-aligned",
                SessionRunEvent::RunStarted {
                    run_id: "run-1".to_string(),
                    user_message_id: "user-1".to_string(),
                },
            )
            .await
            .expect("append run started");
        assert_eq!(
            registry.active_run_id("session-aligned").as_deref(),
            Some("run-1")
        );

        journal
            .append_event(
                "session-aligned",
                SessionRunEvent::RunCompleted {
                    run_id: "run-1".to_string(),
                    turn_state: None,
                },
            )
            .await
            .expect("append run completed");

        let state = journal
            .read_state("session-aligned")
            .await
            .expect("read aligned state");
        assert_eq!(state.current_run_id, None);
        assert_eq!(registry.active_run_id("session-aligned"), None);
    }

    #[tokio::test]
    async fn append_event_updates_observability_snapshot() {
        let journal_root = tempdir().expect("journal tempdir");
        let registry = Arc::new(RunRegistry::default());
        let observability = Arc::new(RuntimeObservability::new(8));
        let journal = SessionJournalStore::with_registry_and_observability(
            journal_root.path().to_path_buf(),
            registry,
            observability.clone(),
        );

        journal
            .append_event(
                "session-observability",
                SessionRunEvent::RunStarted {
                    run_id: "run-1".to_string(),
                    user_message_id: "user-1".to_string(),
                },
            )
            .await
            .expect("append run started");
        journal
            .append_event(
                "session-observability",
                SessionRunEvent::RunGuardWarning {
                    run_id: "run-1".to_string(),
                    warning_kind: "loop_detected".to_string(),
                    title: "loop".to_string(),
                    message: "loop warning".to_string(),
                    detail: None,
                    last_completed_step: None,
                },
            )
            .await
            .expect("append run guard warning");
        journal
            .append_event(
                "session-observability",
                SessionRunEvent::RunCompleted {
                    run_id: "run-1".to_string(),
                    turn_state: None,
                },
            )
            .await
            .expect("append run completed");

        let snapshot = observability.snapshot();
        assert_eq!(snapshot.turns.active, 0);
        assert_eq!(snapshot.turns.completed, 1);
        assert_eq!(
            snapshot.guard.warnings_by_kind.get("loop_detected"),
            Some(&1)
        );
        assert_eq!(snapshot.recent_events.buffered, 3);
    }

    #[tokio::test]
    async fn skill_route_recorded_keeps_tool_plan_summary_in_observed_message() {
        let journal_root = tempdir().expect("journal tempdir");
        let registry = Arc::new(RunRegistry::default());
        let observability = Arc::new(RuntimeObservability::new(8));
        let journal = SessionJournalStore::with_registry_and_observability(
            journal_root.path().to_path_buf(),
            registry,
            observability.clone(),
        );

        journal
            .append_event(
                "session-route-plan",
                SessionRunEvent::SkillRouteRecorded {
                    run_id: "run-1".to_string(),
                    route_latency_ms: 11,
                    candidate_count: 2,
                    selected_runner: "prompt_skill_inline".to_string(),
                    selected_skill: Some("repo-skill".to_string()),
                    fallback_reason: None,
                    tool_recommendation_summary: Some(
                        "tool_recommendation=read_file active=3 deferred=0 loading_policy=full"
                            .to_string(),
                    ),
                    tool_recommendation_aligned: Some(true),
                    tool_plan_summary: Some(EffectiveToolDecisionRecord {
                        source: EffectiveToolSetSource::ExplicitAllowList,
                        allowed_tool_count: 3,
                        active_tool_count: 3,
                        recommended_tool_count: 0,
                        supporting_tool_count: 0,
                        deferred_tool_count: 0,
                        excluded_tool_count: 2,
                        active_tools: vec![
                            "read_file".to_string(),
                            "glob".to_string(),
                            "mcp_repo_files_read".to_string(),
                        ],
                        recommended_tools: Vec::new(),
                        supporting_tools: Vec::new(),
                        deferred_tools: Vec::new(),
                        missing_tools: vec!["missing_tool".to_string()],
                        filtered_out_tools: vec!["bash".to_string()],
                        excluded_tools: vec![EffectiveToolExclusion {
                            name: "bash".to_string(),
                            source: Some(ToolSource::Runtime),
                            category: None,
                            reason: ToolFilterReason::ExplicitDenyList,
                        }],
                        source_counts: vec![
                            EffectiveToolSourceCount {
                                source: ToolSource::Native,
                                count: 2,
                            },
                            EffectiveToolSourceCount {
                                source: ToolSource::Mcp,
                                count: 1,
                            },
                        ],
                        exclusion_counts: vec![EffectiveToolReasonCount {
                            reason: ToolFilterReason::ExplicitDenyList,
                            count: 2,
                        }],
                        policy: EffectiveToolPolicySummary {
                            denied_tool_names: vec!["bash".to_string()],
                            denied_categories: vec![],
                            allowed_categories: None,
                            allowed_sources: None,
                            denied_sources: Vec::new(),
                            allowed_mcp_servers: Some(vec!["repo-files".to_string()]),
                            inputs: Vec::new(),
                        },
                        loading_policy: ToolLoadingPolicy::Full,
                        expanded_to_full: false,
                        expansion_reason: None,
                        discovery_candidates: Vec::new(),
                    }),
                },
            )
            .await
            .expect("append skill route recorded");

        let recent = observability.recent_events();
        let observed = match &recent[0] {
            RuntimeObservedEvent::SessionRun(event) => event,
            other => panic!("unexpected event: {other:?}"),
        };

        assert_eq!(observed.event_type, "skill_route_recorded");
        assert_eq!(observed.route_latency_ms, Some(11));
        assert_eq!(observed.candidate_count, Some(2));
        assert_eq!(observed.selected_skill.as_deref(), Some("repo-skill"));
        assert_eq!(
            observed
                .tool_plan_summary
                .as_ref()
                .map(|summary| summary.excluded_tool_count),
            Some(2)
        );
        assert_eq!(
            observed
                .tool_plan_summary
                .as_ref()
                .map(|summary| summary.filtered_out_tools.clone()),
            Some(vec!["bash".to_string()])
        );
        assert!(observed
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("\"allowed_tool_count\":3"));
        assert!(observed
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("\"explicit_deny_list\""));
    }

    #[tokio::test]
    async fn append_event_projects_terminal_turn_state_into_session_state() {
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        journal
            .append_event(
                "session-turn-state",
                SessionRunEvent::RunStarted {
                    run_id: "run-1".to_string(),
                    user_message_id: "user-1".to_string(),
                },
            )
            .await
            .expect("append run started");
        journal
            .append_event(
                "session-turn-state",
                SessionRunEvent::RunFailed {
                    run_id: "run-1".to_string(),
                    error_kind: "max_turns".to_string(),
                    error_message: "达到最大迭代次数".to_string(),
                    turn_state: Some(SessionRunTurnStateSnapshot {
                        task_identity: None,
                        session_surface: None,
                        execution_lane: Some("open_task".to_string()),
                        selected_runner: Some("open_task".to_string()),
                        selected_skill: None,
                        fallback_reason: None,
                        allowed_tools: vec!["read".to_string(), "exec".to_string()],
                        invoked_skills: Vec::new(),
                        partial_assistant_text: "正在继续处理剩余步骤".to_string(),
                        tool_failure_streak: 0,
                        reconstructed_history_len: Some(5),
                        compaction_boundary: Some(SessionRunTurnStateCompactionBoundary {
                            transcript_path: "temp/transcripts/session-1.json".to_string(),
                            original_tokens: 4096,
                            compacted_tokens: 1024,
                            summary: "压缩摘要".to_string(),
                        }),
                    }),
                },
            )
            .await
            .expect("append run failed");

        let state = journal
            .read_state("session-turn-state")
            .await
            .expect("read projected state");
        let run = state.runs.first().expect("run snapshot");

        assert_eq!(run.last_error_kind.as_deref(), Some("max_turns"));
        assert_eq!(
            run.turn_state
                .as_ref()
                .and_then(|turn_state| turn_state.compaction_boundary.as_ref())
                .map(|boundary| boundary.original_tokens),
            Some(4096)
        );
        assert_eq!(
            run.turn_state
                .as_ref()
                .map(|turn_state| turn_state.allowed_tools.clone()),
            Some(vec!["read".to_string(), "exec".to_string()])
        );
    }

    #[tokio::test]
    async fn append_event_projects_task_identity_into_session_state() {
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        journal
            .append_event(
                "session-task-state",
                SessionRunEvent::TaskStateProjected {
                    run_id: "run-1".to_string(),
                    task_identity: SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                        backend_kind: "interactive_chat_backend".to_string(),
                    },
                },
            )
            .await
            .expect("append task state projected");

        let state = journal
            .read_state("session-task-state")
            .await
            .expect("read projected state");
        let run = state.runs.first().expect("run snapshot");

        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.task_id.as_str()),
            Some("task-1")
        );
        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.task_kind.as_str()),
            Some("primary_user_task")
        );
        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.surface_kind.as_str()),
            Some("local_chat_surface")
        );
        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.backend_kind.as_str()),
            Some("interactive_chat_backend")
        );
    }

    #[tokio::test]
    async fn append_event_projects_task_continued_into_session_state() {
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        journal
            .append_event(
                "session-task-continued",
                SessionRunEvent::TaskContinued {
                    run_id: "run-1".to_string(),
                    task_identity: SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: Some("task-parent".to_string()),
                        root_task_id: "task-root".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                        backend_kind: "interactive_chat_backend".to_string(),
                    },
                    continuation_mode: "recovery_resume".to_string(),
                    continuation_reason: "recovery_resume".to_string(),
                },
            )
            .await
            .expect("append task continued");

        let state = journal
            .read_state("session-task-continued")
            .await
            .expect("read projected state");
        let run = state.runs.first().expect("run snapshot");

        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.task_id.as_str()),
            Some("task-1")
        );
        assert_eq!(
            run.task_identity
                .as_ref()
                .and_then(|identity| identity.parent_task_id.as_deref()),
            Some("task-parent")
        );
        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.root_task_id.as_str()),
            Some("task-root")
        );
        assert_eq!(
            run.task_continuation_mode.as_deref(),
            Some("recovery_resume")
        );
        assert_eq!(
            run.task_continuation_reason.as_deref(),
            Some("recovery_resume")
        );
    }

    #[tokio::test]
    async fn append_event_projects_task_record_into_session_state() {
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        journal
            .append_event(
                "session-task-record",
                SessionRunEvent::TaskRecordUpserted {
                    run_id: "run-1".to_string(),
                    task: TaskRecordUpsertPayload {
                        task_identity: crate::agent::runtime::task_state::TaskIdentity::new(
                            "task-1",
                            Some("task-parent"),
                            Some("task-root"),
                        ),
                        task_kind: crate::agent::runtime::task_state::TaskKind::EmployeeStepTask,
                        surface_kind:
                            crate::agent::runtime::task_state::TaskSurfaceKind::EmployeeStepSurface,
                        backend_kind:
                            crate::agent::runtime::task_state::TaskBackendKind::EmployeeStepBackend,
                        session_id: "session-task-record".to_string(),
                        user_message_id: "user-1".to_string(),
                        run_id: "run-1".to_string(),
                        status: TaskLifecycleStatus::Running,
                        created_at: "2026-04-09T10:00:00Z".to_string(),
                        updated_at: "2026-04-09T10:01:00Z".to_string(),
                        started_at: Some("2026-04-09T10:01:00Z".to_string()),
                        completed_at: None,
                        terminal_reason: None,
                    },
                },
            )
            .await
            .expect("append task record upserted");

        let state = journal
            .read_state("session-task-record")
            .await
            .expect("read projected state");
        let task = state.tasks.first().expect("task snapshot");

        assert_eq!(task.task_identity.task_id, "task-1");
        assert_eq!(
            task.task_identity.parent_task_id.as_deref(),
            Some("task-parent")
        );
        assert_eq!(task.task_identity.root_task_id, "task-root");
        assert_eq!(task.task_identity.task_kind, "employee_step_task");
        assert_eq!(task.task_identity.surface_kind, "employee_step_surface");
        assert_eq!(task.task_identity.backend_kind, "employee_step_backend");
        assert_eq!(task.status, TaskLifecycleStatus::Running);
        assert_eq!(task.started_at.as_deref(), Some("2026-04-09T10:01:00Z"));
    }

    #[tokio::test]
    async fn append_event_projects_task_status_changes_into_session_state() {
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        journal
            .append_event(
                "session-task-status",
                SessionRunEvent::TaskRecordUpserted {
                    run_id: "run-1".to_string(),
                    task: TaskRecordUpsertPayload {
                        task_identity: crate::agent::runtime::task_state::TaskIdentity::new(
                            "task-1",
                            Option::<String>::None,
                            Some("task-1"),
                        ),
                        task_kind: crate::agent::runtime::task_state::TaskKind::PrimaryUserTask,
                        surface_kind:
                            crate::agent::runtime::task_state::TaskSurfaceKind::LocalChatSurface,
                        backend_kind:
                            crate::agent::runtime::task_state::TaskBackendKind::InteractiveChatBackend,
                        session_id: "session-task-status".to_string(),
                        user_message_id: "user-1".to_string(),
                        run_id: "run-1".to_string(),
                        status: TaskLifecycleStatus::Running,
                        created_at: "2026-04-09T10:00:00Z".to_string(),
                        updated_at: "2026-04-09T10:01:00Z".to_string(),
                        started_at: Some("2026-04-09T10:01:00Z".to_string()),
                        completed_at: None,
                        terminal_reason: None,
                    },
                },
            )
            .await
            .expect("append task record upserted");
        journal
            .append_event(
                "session-task-status",
                SessionRunEvent::TaskStatusChanged {
                    run_id: "run-1".to_string(),
                    status_change: TaskStatusChangedPayload {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        from_status: TaskLifecycleStatus::Running,
                        to_status: TaskLifecycleStatus::Failed,
                        terminal_reason: Some("tool_timeout".to_string()),
                        updated_at: "2026-04-09T10:02:00Z".to_string(),
                    },
                },
            )
            .await
            .expect("append task status changed");

        let state = journal
            .read_state("session-task-status")
            .await
            .expect("read projected state");
        let task = state.tasks.first().expect("task snapshot");

        assert_eq!(task.status, TaskLifecycleStatus::Failed);
        assert_eq!(task.updated_at, "2026-04-09T10:02:00Z");
        assert_eq!(task.completed_at.as_deref(), Some("2026-04-09T10:02:00Z"));
        assert_eq!(task.terminal_reason.as_deref(), Some("tool_timeout"));
    }

    #[test]
    fn session_journal_state_deserializes_without_task_records_for_legacy_snapshots() {
        let legacy = serde_json::json!({
            "session_id": "session-legacy",
            "current_run_id": null,
            "runs": []
        });

        let state: SessionJournalState = serde_json::from_value(legacy).expect("legacy state");

        assert!(state.tasks.is_empty());
    }

    #[test]
    fn session_run_turn_state_snapshot_projects_session_surface_from_kernel_state() {
        let projected = SessionRunTurnStateSnapshot::from(
            &TurnStateSnapshot::default()
                .with_session_surface(SessionSurfaceKind::LocalChat)
                .with_execution_lane(ExecutionLane::OpenTask),
        );

        assert_eq!(projected.session_surface.as_deref(), Some("local_chat"));
        assert_eq!(projected.execution_lane.as_deref(), Some("open_task"));

        let employee_step_projected = SessionRunTurnStateSnapshot::from(
            &TurnStateSnapshot::default()
                .with_session_surface(SessionSurfaceKind::EmployeeStepSession),
        );
        assert_eq!(
            employee_step_projected.session_surface.as_deref(),
            Some("employee_step_session")
        );
    }

    #[test]
    fn session_run_turn_state_snapshot_projects_task_identity_from_kernel_state() {
        let task_state = crate::agent::runtime::task_state::TaskState::new_primary_local_chat(
            "session-1",
            "user-1",
            "run-1",
        );
        let projected = SessionRunTurnStateSnapshot::from(
            &TurnStateSnapshot::default().with_task_state(&task_state),
        );

        assert_eq!(
            projected
                .task_identity
                .as_ref()
                .map(|identity| identity.task_id.as_str()),
            Some(task_state.task_identity.task_id.as_str())
        );
        assert_eq!(
            projected
                .task_identity
                .as_ref()
                .map(|identity| identity.task_kind.as_str()),
            Some("primary_user_task")
        );
        assert_eq!(
            projected
                .task_identity
                .as_ref()
                .map(|identity| identity.surface_kind.as_str()),
            Some("local_chat_surface")
        );
    }
}
