use super::super::repo::{
    find_employee_session_seed_row, find_existing_session_skill_id, find_group_run_start_config,
    find_group_step_session_row, find_model_config_row, find_recent_group_step_session_id,
    insert_group_run_event, insert_group_run_record, insert_group_run_step_seed,
    insert_session_message, insert_session_seed, insert_tx_session_message,
    list_session_message_rows, SessionSeedInput,
};
use super::super::{EmployeeGroupRunResult, StartEmployeeGroupRunInput};
use super::{get_employee_group_run_snapshot_by_run_id_with_pool, list_agent_employees_with_pool};
use crate::agent::run_guard::RunStopReasonKind;
use crate::agent::runtime::kernel::execution_plan::ExecutionOutcome;
use crate::agent::runtime::runtime_io::insert_session_message_with_pool;
use crate::agent::runtime::task_active_run::{
    DelegatedTaskBackendRunRequest, TaskExecutionOutcome,
};
use crate::agent::runtime::task_backend::{
    execute_prepared_task_backend_with_context, prepare_task_backend,
    EmployeeStepTaskBackendPreparationRequest, TaskBackendExecutionContext,
    TaskBackendPreparationRequest, TaskBackendTokenCallback,
};
use crate::agent::runtime::task_entry;
use crate::agent::runtime::task_entry::{
    DelegatedTaskBackendRunAndFinalizeRequest, DelegatedTaskTerminalFinalizeEntryRequest,
};
use crate::agent::runtime::task_lifecycle;
use crate::agent::runtime::task_lifecycle::TaskBeginParentContext;
use crate::agent::runtime::task_record::TaskRecord;
use crate::agent::runtime::task_state::TaskState;
use crate::agent::runtime::task_terminal::DelegatedTaskTerminalOutcome;
use crate::agent::tools::{EmployeeManageTool, MemoryTool};
use crate::agent::{AgentExecutor, ToolRegistry};
use crate::commands::chat_runtime_io::extract_assistant_text_content;
use crate::commands::models::resolve_default_model_id_with_pool;
use crate::session_journal::SessionJournalStore;
use serde_json::Value;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct PreparedEmployeeStepSessionRun {
    run_id: String,
    task_state: TaskState,
    parent_task_record: Option<TaskRecord>,
}

async fn prepare_employee_step_session_run(
    pool: &SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    prompt: &str,
) -> Result<PreparedEmployeeStepSessionRun, String> {
    let user_message_id =
        insert_session_message_with_pool(pool, session_id, "user", prompt, None).await?;
    let run_id = Uuid::new_v4().to_string();
    let parent_task_record =
        task_lifecycle::resolve_latest_task_record_for_session(journal, session_id).await;
    let task_state = TaskState::new_employee_step(
        session_id,
        &user_message_id,
        &run_id,
        parent_task_record
            .as_ref()
            .map(|record| &record.task_identity),
    );
    Ok(PreparedEmployeeStepSessionRun {
        run_id,
        task_state,
        parent_task_record,
    })
}

#[cfg(test)]
async fn finalize_employee_step_execution_outcome(
    pool: &SqlitePool,
    journal: &SessionJournalStore,
    _prepared: &PreparedEmployeeStepSessionRun,
    outcome: TaskExecutionOutcome,
) -> Result<DelegatedTaskTerminalOutcome, String> {
    match task_entry::finalize_delegated_task_execution_outcome_entry(
        DelegatedTaskTerminalFinalizeEntryRequest {
            db: pool,
            journal,
            task_execution_outcome: outcome,
        },
    )
    .await?
    {
        outcome => Ok(outcome),
    }
}

pub(crate) async fn execute_group_step_in_employee_context_with_pool(
    pool: &SqlitePool,
    journal: Option<&SessionJournalStore>,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    user_goal: &str,
    step_input: &str,
) -> Result<String, String> {
    let session_row = find_group_step_session_row(pool, session_id)
        .await?
        .ok_or_else(|| "group step session not found".to_string())?;

    let employee = list_agent_employees_with_pool(pool)
        .await?
        .into_iter()
        .find(|item| {
            item.employee_id.eq_ignore_ascii_case(assignee_employee_id)
                || item.role_id.eq_ignore_ascii_case(assignee_employee_id)
                || item.id.eq_ignore_ascii_case(assignee_employee_id)
        })
        .ok_or_else(|| "assignee employee not found".to_string())?;

    let model_row = find_model_config_row(pool, &session_row.model_id)
        .await?
        .ok_or_else(|| "model config not found".to_string())?;

    let (system_prompt, allowed_tools, max_iterations) =
        super::super::build_group_step_system_prompt(&employee, &session_row.skill_id);
    let user_prompt = super::super::build_group_step_user_prompt(
        run_id, step_id, user_goal, step_input, &employee,
    );

    let prepared_run = if let Some(journal) = journal {
        Some(prepare_employee_step_session_run(pool, journal, session_id, &user_prompt).await?)
    } else {
        let now = chrono::Utc::now().to_rfc3339();
        insert_session_message(pool, session_id, "user", &user_prompt, &now).await?;
        None
    };

    let messages: Vec<Value> = list_session_message_rows(pool, session_id)
        .await?
        .into_iter()
        .map(|row| {
            let normalized_content = if row.role == "assistant" {
                extract_assistant_text_content(&row.content)
            } else {
                row.content
            };
            serde_json::json!({ "role": row.role, "content": normalized_content })
        })
        .collect();

    let registry = Arc::new(ToolRegistry::with_standard_tools());
    let memory_root = if session_row.work_dir.trim().is_empty() {
        std::env::temp_dir().join("workclaw-group-run-memory")
    } else {
        PathBuf::from(session_row.work_dir.trim())
            .join("openclaw")
            .join(employee.employee_id.trim())
            .join("memory")
    };
    let memory_dir = memory_root.join(if session_row.skill_id.trim().is_empty() {
        "builtin-general"
    } else {
        session_row.skill_id.trim()
    });
    std::fs::create_dir_all(&memory_dir).map_err(|e| e.to_string())?;
    registry.register(Arc::new(MemoryTool::new(memory_dir)));
    registry.register(Arc::new(EmployeeManageTool::new(pool.clone())));

    let executor = Arc::new(AgentExecutor::with_max_iterations(
        Arc::clone(&registry),
        max_iterations,
    ));
    let assistant_output = if let (Some(journal), Some(prepared_run)) =
        (journal, prepared_run.as_ref())
    {
        match task_entry::run_and_finalize_delegated_task_backend(
            DelegatedTaskBackendRunAndFinalizeRequest {
                backend_request: DelegatedTaskBackendRunRequest {
                    db: pool,
                    journal,
                    task_state: prepared_run.task_state.clone(),
                    parent_context: prepared_run.parent_task_record.as_ref().map(|record| {
                        TaskBeginParentContext {
                            session_id,
                            active_task_record: record,
                        }
                    }),
                    preparation_request: TaskBackendPreparationRequest::EmployeeStep(
                        EmployeeStepTaskBackendPreparationRequest {
                            agent_executor: &executor,
                            user_prompt: &user_prompt,
                            employee_step_system_prompt: &system_prompt,
                            api_format: &model_row.api_format,
                            base_url: &model_row.base_url,
                            api_key: &model_row.api_key,
                            model: &model_row.model_name,
                            allowed_tools,
                            max_iterations,
                            work_dir: if session_row.work_dir.trim().is_empty() {
                                None
                            } else {
                                Some(session_row.work_dir.clone())
                            },
                        },
                    ),
                    app_handle: None,
                    agent_executor: Arc::clone(&executor),
                    on_token: Arc::new(|_| {}) as TaskBackendTokenCallback,
                    prepare_surface: move |prepared_surface| {
                        prepared_surface.turn_context.messages = messages;
                    },
                },
            },
        )
        .await?
        {
            DelegatedTaskTerminalOutcome::Completed { output } => output,
            DelegatedTaskTerminalOutcome::Stopped { stop_reason, error } => {
                if stop_reason.kind != RunStopReasonKind::MaxTurns {
                    return Err(error);
                }
                let fallback_output = super::super::build_group_step_iteration_fallback_output(
                    &employee,
                    user_goal,
                    step_input,
                    stop_reason
                        .detail
                        .as_deref()
                        .unwrap_or(stop_reason.message.as_str()),
                );
                let finished_at = chrono::Utc::now().to_rfc3339();
                insert_session_message(
                    pool,
                    session_id,
                    "assistant",
                    &fallback_output,
                    &finished_at,
                )
                .await?;
                return Ok(fallback_output);
            }
            DelegatedTaskTerminalOutcome::Failed { error } => return Err(error),
        }
    } else {
        let mut prepared_surface =
            prepare_task_backend(TaskBackendPreparationRequest::EmployeeStep(
                EmployeeStepTaskBackendPreparationRequest {
                    agent_executor: &executor,
                    user_prompt: &user_prompt,
                    employee_step_system_prompt: &system_prompt,
                    api_format: &model_row.api_format,
                    base_url: &model_row.base_url,
                    api_key: &model_row.api_key,
                    model: &model_row.model_name,
                    allowed_tools,
                    max_iterations,
                    work_dir: if session_row.work_dir.trim().is_empty() {
                        None
                    } else {
                        Some(session_row.work_dir.clone())
                    },
                },
            ))
            .await?;
        prepared_surface.turn_context.messages = messages;

        match execute_prepared_task_backend_with_context(
            &prepared_surface,
            TaskBackendExecutionContext::Delegated {
                app_handle: None,
                agent_executor: Arc::clone(&executor),
                session_id,
                on_token: Arc::new(|_| {}) as TaskBackendTokenCallback,
            },
        )
        .await
        {
            Ok(outcome) => match outcome {
                ExecutionOutcome::RouteExecution {
                    route_execution, ..
                } => {
                    if let Some(final_messages) = route_execution.final_messages {
                        let assistant_output =
                            super::super::extract_assistant_text(&final_messages);
                        if assistant_output.trim().is_empty() {
                            return Err("employee step execution returned empty assistant output"
                                .to_string());
                        }
                        assistant_output
                    } else if let Some(stop_reason) = route_execution.last_stop_reason {
                        if stop_reason.kind != RunStopReasonKind::MaxTurns {
                            return Err(route_execution
                                .last_error
                                .unwrap_or_else(|| stop_reason.message.clone()));
                        }
                        let fallback_output =
                            super::super::build_group_step_iteration_fallback_output(
                                &employee,
                                user_goal,
                                step_input,
                                stop_reason
                                    .detail
                                    .as_deref()
                                    .unwrap_or(stop_reason.message.as_str()),
                            );
                        let finished_at = chrono::Utc::now().to_rfc3339();
                        insert_session_message(
                            pool,
                            session_id,
                            "assistant",
                            &fallback_output,
                            &finished_at,
                        )
                        .await?;
                        return Ok(fallback_output);
                    } else {
                        return Err(route_execution
                            .last_error
                            .unwrap_or_else(|| "employee step execution failed".to_string()));
                    }
                }
                ExecutionOutcome::DirectDispatch { output, .. } => output,
                ExecutionOutcome::SkillCommandFailed { error, .. }
                | ExecutionOutcome::SkillCommandStopped { error, .. } => return Err(error),
            },
            Err(message) => return Err(message),
        }
    };

    let finished_at = chrono::Utc::now().to_rfc3339();
    insert_session_message(
        pool,
        session_id,
        "assistant",
        &assistant_output,
        &finished_at,
    )
    .await?;

    Ok(assistant_output)
}

pub(crate) async fn ensure_group_run_session_with_pool(
    pool: &SqlitePool,
    coordinator_employee_id: &str,
    group_name: &str,
    now: &str,
    preferred_session_id: Option<&str>,
) -> Result<(String, String), String> {
    let employee_row = find_employee_session_seed_row(pool, coordinator_employee_id)
        .await?
        .ok_or_else(|| "coordinator employee not found".to_string())?;

    let session_skill_id = if employee_row.primary_skill_id.trim().is_empty() {
        "builtin-general".to_string()
    } else {
        employee_row.primary_skill_id.trim().to_string()
    };

    if let Some(existing_session_id) = preferred_session_id
        .map(str::trim)
        .filter(|session_id| !session_id.is_empty())
    {
        let existing_skill_id = find_existing_session_skill_id(pool, existing_session_id)
            .await?
            .ok_or_else(|| "preferred group run session not found".to_string())?;
        let existing_skill_id = if existing_skill_id.trim().is_empty() {
            session_skill_id.clone()
        } else {
            existing_skill_id.trim().to_string()
        };
        return Ok((existing_session_id.to_string(), existing_skill_id));
    }

    let model_id = resolve_default_model_id_with_pool(pool)
        .await?
        .ok_or_else(|| "model config not found".to_string())?;

    let session_id = Uuid::new_v4().to_string();
    insert_session_seed(
        pool,
        &SessionSeedInput {
            id: &session_id,
            skill_id: &session_skill_id,
            title: &format!("群组协作：{}", group_name.trim()),
            created_at: now,
            model_id: &model_id,
            work_dir: &employee_row.default_work_dir,
            employee_id: coordinator_employee_id,
        },
    )
    .await?;

    Ok((session_id, session_skill_id))
}

pub(crate) async fn append_group_run_assistant_message_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    content: &str,
) -> Result<(), String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let now = chrono::Utc::now().to_rfc3339();
    insert_session_message(pool, session_id, "assistant", trimmed, &now).await
}

pub(crate) async fn ensure_group_step_session_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    assignee_employee_id: &str,
    now: &str,
) -> Result<String, String> {
    if let Some(session_id) =
        find_recent_group_step_session_id(pool, run_id, assignee_employee_id).await?
    {
        return Ok(session_id);
    }

    let employee_row = find_employee_session_seed_row(pool, assignee_employee_id)
        .await?
        .ok_or_else(|| "assignee employee not found".to_string())?;

    let session_skill_id = if employee_row.primary_skill_id.trim().is_empty() {
        "builtin-general".to_string()
    } else {
        employee_row.primary_skill_id.trim().to_string()
    };

    let model_id = resolve_default_model_id_with_pool(pool)
        .await?
        .ok_or_else(|| "model config not found".to_string())?;

    let session_id = Uuid::new_v4().to_string();
    insert_session_seed(
        pool,
        &SessionSeedInput {
            id: &session_id,
            skill_id: &session_skill_id,
            title: &format!("群组执行:{}@{}", run_id, assignee_employee_id),
            created_at: now,
            model_id: &model_id,
            work_dir: &employee_row.default_work_dir,
            employee_id: assignee_employee_id,
        },
    )
    .await?;

    Ok(session_id)
}

pub(crate) async fn start_employee_group_run_internal_with_pool(
    pool: &SqlitePool,
    journal: Option<&SessionJournalStore>,
    input: StartEmployeeGroupRunInput,
    preferred_session_id: Option<&str>,
    persist_user_message: bool,
) -> Result<EmployeeGroupRunResult, String> {
    let group_id = input.group_id.trim().to_string();
    if group_id.is_empty() {
        return Err("group_id is required".to_string());
    }
    let user_goal = input.user_goal.trim().to_string();
    if user_goal.is_empty() {
        return Err("user_goal is required".to_string());
    }

    let config = find_group_run_start_config(pool, &group_id)
        .await?
        .ok_or_else(|| "employee group not found".to_string())?;

    let member_employee_ids =
        serde_json::from_str::<Vec<String>>(&config.member_employee_ids_json).unwrap_or_default();
    let rules = super::super::list_employee_group_rules_with_pool(pool, &group_id).await?;
    let planner_employee_id = super::super::resolve_group_planner_employee_id(
        &config.entry_employee_id,
        &config.coordinator_employee_id,
        &rules,
    );
    let reviewer_employee_id = super::super::resolve_group_reviewer_employee_id(
        &config.review_mode,
        &planner_employee_id,
        &rules,
    );
    let (execute_targets, _) = super::super::select_group_execute_dispatch_targets(
        &rules,
        &member_employee_ids,
        &[
            config.coordinator_employee_id.clone(),
            planner_employee_id.clone(),
            config.entry_employee_id.clone(),
        ],
    );

    let plan = crate::agent::group_orchestrator::build_group_run_plan(
        crate::agent::group_orchestrator::GroupRunRequest {
            group_id: group_id.clone(),
            coordinator_employee_id: config.coordinator_employee_id.clone(),
            planner_employee_id: Some(planner_employee_id.clone()),
            reviewer_employee_id: reviewer_employee_id.clone(),
            member_employee_ids,
            execute_targets,
            user_goal: user_goal.clone(),
            execution_window: input.execution_window,
            timeout_employee_ids: input.timeout_employee_ids,
            max_retry_per_step: input.max_retry_per_step,
        },
    );
    let initial_report = plan.final_report.clone();
    let initial_state = plan.state.clone();
    let initial_round = plan.current_round;
    let now = chrono::Utc::now().to_rfc3339();
    let run_id = Uuid::new_v4().to_string();
    let (session_id, session_skill_id) = ensure_group_run_session_with_pool(
        pool,
        &config.coordinator_employee_id,
        &config.name,
        &now,
        preferred_session_id,
    )
    .await?;

    let waiting_for_employee_id = reviewer_employee_id
        .as_deref()
        .filter(|employee_id| !employee_id.trim().is_empty())
        .unwrap_or(config.coordinator_employee_id.as_str())
        .to_string();

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    insert_group_run_record(
        &mut tx,
        &run_id,
        &group_id,
        &session_id,
        &user_goal,
        &initial_state,
        initial_round,
        &plan.current_phase,
        &config.coordinator_employee_id,
        &waiting_for_employee_id,
        &now,
    )
    .await?;

    if persist_user_message {
        insert_tx_session_message(&mut tx, &session_id, "user", &user_goal, &now).await?;
    }

    for event in &plan.events {
        insert_group_run_event(
            &mut tx,
            &run_id,
            "",
            &event.event_type,
            &event.payload_json,
            &now,
        )
        .await?;
    }

    for step in plan.steps {
        let step_id = Uuid::new_v4().to_string();
        let dispatch_source_employee_id = step.dispatch_source_employee_id.clone();
        insert_group_run_step_seed(
            &mut tx,
            &run_id,
            &step_id,
            step.round_no,
            &step.assignee_employee_id,
            &dispatch_source_employee_id,
            &step.phase,
            &step.step_type,
            &user_goal,
            &step.output,
            &step.status,
            step.requires_review,
            &step.review_status,
            &now,
        )
        .await?;
        insert_group_run_event(
            &mut tx,
            &run_id,
            &step_id,
            "step_created",
            &serde_json::json!({
                "phase": step.phase,
                "step_type": step.step_type,
                "assignee_employee_id": step.assignee_employee_id,
                "dispatch_source_employee_id": dispatch_source_employee_id,
                "status": step.status
            })
            .to_string(),
            &now,
        )
        .await?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    let snapshot =
        super::super::continue_employee_group_run_with_pool_and_journal(pool, journal, &run_id)
            .await?;
    if snapshot.state != "done" {
        append_group_run_assistant_message_with_pool(pool, &session_id, &initial_report).await?;
    }
    let final_snapshot = get_employee_group_run_snapshot_by_run_id_with_pool(pool, &run_id).await?;

    Ok(EmployeeGroupRunResult {
        run_id,
        group_id,
        session_id,
        session_skill_id,
        state: final_snapshot.state,
        current_round: final_snapshot.current_round,
        final_report: final_snapshot.final_report,
        steps: final_snapshot.steps,
    })
}

#[cfg(test)]
mod tests {
    use super::{finalize_employee_step_execution_outcome, prepare_employee_step_session_run};
    use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
    use crate::agent::runtime::kernel::execution_plan::{ExecutionLane, ExecutionOutcome};
    use crate::agent::runtime::kernel::session_profile::SessionSurfaceKind;
    use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
    use crate::agent::runtime::task_active_run::TaskExecutionOutcome;
    use crate::agent::runtime::task_lifecycle;
    use crate::agent::runtime::task_lifecycle::TaskBeginParentContext;
    use crate::agent::runtime::task_terminal::DelegatedTaskTerminalOutcome;
    use crate::session_journal::{
        SessionJournalStore, SessionRunEvent, SessionRunTaskIdentitySnapshot,
    };
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    async fn setup_employee_step_runtime_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                content_json TEXT,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create messages table");

        sqlx::query(
            "CREATE TABLE session_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                user_message_id TEXT NOT NULL DEFAULT '',
                assistant_message_id TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'queued',
                buffered_text TEXT NOT NULL DEFAULT '',
                error_kind TEXT NOT NULL DEFAULT '',
                error_message TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_runs table");

        sqlx::query(
            "CREATE TABLE session_run_events (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_run_events table");

        pool
    }

    #[tokio::test]
    async fn finalize_employee_step_execution_outcome_persists_employee_step_turn_state() {
        let pool = setup_employee_step_runtime_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let prepared = prepare_employee_step_session_run(
            &pool,
            &journal,
            "session-1",
            "请先汇总本周日报，再补充风险项。",
        )
        .await
        .expect("prepare employee step session");

        let outcome = ExecutionOutcome::RouteExecution {
            route_execution: RouteExecutionOutcome {
                final_messages: Some(vec![
                    json!({
                        "role": "user",
                        "content": "请先汇总本周日报，再补充风险项。",
                    }),
                    json!({
                        "role": "assistant",
                        "content": "已汇总日报，并补充了当前风险项。",
                    }),
                ]),
                last_error: None,
                last_error_kind: None,
                last_stop_reason: None,
                partial_text: String::new(),
                reasoning_text: String::new(),
                reasoning_duration_ms: None,
                tool_exposure_expanded: false,
                tool_exposure_expansion_reason: None,
                compaction_boundary: None,
            },
            reconstructed_history_len: 1,
            turn_state: TurnStateSnapshot::default()
                .with_session_surface(SessionSurfaceKind::EmployeeStepSession)
                .with_execution_lane(ExecutionLane::OpenTask),
        };

        let active_task_record = task_lifecycle::begin_task_run(
            &pool,
            &journal,
            &prepared.task_state,
            None::<TaskBeginParentContext<'_>>,
        )
        .await
        .expect("begin employee step task run");
        let wrapped =
            TaskExecutionOutcome::new(prepared.task_state.clone(), active_task_record, outcome);

        let finalized =
            finalize_employee_step_execution_outcome(&pool, &journal, &prepared, wrapped)
                .await
                .expect("finalize employee step outcome");

        assert!(matches!(
            finalized,
            DelegatedTaskTerminalOutcome::Completed { ref output }
                if output == "已汇总日报，并补充了当前风险项。"
        ));

        let state = journal
            .read_state("session-1")
            .await
            .expect("read journal state");
        let run = state.runs.first().expect("run snapshot");
        assert_eq!(
            run.turn_state
                .as_ref()
                .and_then(|turn_state| turn_state.session_surface.as_deref()),
            Some("employee_step_session")
        );
        assert_eq!(
            run.turn_state
                .as_ref()
                .and_then(|turn_state| turn_state.task_identity.as_ref())
                .map(|identity| identity.task_kind.as_str()),
            Some("employee_step_task")
        );
    }

    #[tokio::test]
    async fn prepare_employee_step_session_run_projects_employee_task_identity_from_session_lineage(
    ) {
        let pool = setup_employee_step_runtime_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskStateProjected {
                    run_id: "run-parent".to_string(),
                    task_identity: SessionRunTaskIdentitySnapshot {
                        task_id: "task-parent".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-root".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                        backend_kind: "interactive_chat_backend".to_string(),
                    },
                },
            )
            .await
            .expect("append parent task identity");

        let prepared = prepare_employee_step_session_run(
            &pool,
            &journal,
            "session-1",
            "请先汇总本周日报，再补充风险项。",
        )
        .await
        .expect("prepare employee step session");

        let state = journal.read_state("session-1").await.expect("read state");
        let run = state
            .runs
            .iter()
            .find(|run| run.run_id == prepared.run_id)
            .expect("prepared run snapshot");

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
            run.task_identity
                .as_ref()
                .map(|identity| identity.task_kind.as_str()),
            Some("employee_step_task")
        );
        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.surface_kind.as_str()),
            Some("employee_step_surface")
        );
    }
}
