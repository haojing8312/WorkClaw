use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone)]
pub struct SessionJournalStore {
    root: PathBuf,
}

impl SessionJournalStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
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
}

impl Default for SessionJournalState {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            current_run_id: None,
            runs: Vec::new(),
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionRunStatus {
    Queued,
    Thinking,
    ToolCalling,
    WaitingUser,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionRunEvent {
    RunStarted {
        run_id: String,
        user_message_id: String,
    },
    AssistantChunkAppended {
        run_id: String,
        chunk: String,
    },
    ToolStarted {
        run_id: String,
        tool_name: String,
        call_id: String,
    },
    ToolCompleted {
        run_id: String,
        tool_name: String,
        call_id: String,
        output: String,
    },
    RunCompleted {
        run_id: String,
    },
    RunFailed {
        run_id: String,
        error_kind: String,
        error_message: String,
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
        SessionRunEvent::RunStarted { run_id, .. }
        | SessionRunEvent::AssistantChunkAppended { run_id, .. }
        | SessionRunEvent::ToolStarted { run_id, .. }
        | SessionRunEvent::ToolCompleted { run_id, .. }
        | SessionRunEvent::RunCompleted { run_id }
        | SessionRunEvent::RunFailed { run_id, .. }
        | SessionRunEvent::RunCancelled { run_id, .. } => run_id.clone(),
    };
    let run_index = upsert_run_index(state, &run_id);

    match event {
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
        }
        SessionRunEvent::AssistantChunkAppended { chunk, .. } => {
            let run = &mut state.runs[run_index];
            run.buffered_text.push_str(chunk);
            if matches!(run.status, SessionRunStatus::Queued) {
                run.status = SessionRunStatus::Thinking;
            }
        }
        SessionRunEvent::ToolStarted { .. } => {
            let run = &mut state.runs[run_index];
            run.status = SessionRunStatus::ToolCalling;
        }
        SessionRunEvent::ToolCompleted { .. } => {
            let run = &mut state.runs[run_index];
            run.status = SessionRunStatus::Thinking;
        }
        SessionRunEvent::RunCompleted { run_id } => {
            state.runs[run_index].status = SessionRunStatus::Completed;
            if state.current_run_id.as_deref() == Some(run_id.as_str()) {
                state.current_run_id = None;
            }
        }
        SessionRunEvent::RunFailed {
            run_id,
            error_kind,
            error_message,
        } => {
            if state.current_run_id.as_deref() == Some(run_id.as_str()) {
                state.current_run_id = None;
            }
            let run = &mut state.runs[run_index];
            run.status = SessionRunStatus::Failed;
            run.last_error_kind = Some(error_kind.clone());
            run.last_error_message = Some(error_message.clone());
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

fn upsert_run_index(state: &mut SessionJournalState, run_id: &str) -> usize {
    if let Some(index) = state.runs.iter().position(|run| run.run_id == run_id) {
        return index;
    }
    state.runs.push(SessionRunSnapshot::new(run_id));
    state.runs.len() - 1
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
            SessionRunStatus::WaitingUser => "waiting_user",
            SessionRunStatus::Completed => "completed",
            SessionRunStatus::Failed => "failed",
            SessionRunStatus::Cancelled => "cancelled",
        }
    }
}
