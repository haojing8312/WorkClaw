use super::repo::{
    find_employee_session_seed_row, find_existing_session_skill_id, find_group_step_session_row,
    find_group_run_start_config, find_model_config_row, find_recent_group_step_session_id,
    find_group_run_execute_step_context, find_group_run_finalize_state, find_group_run_state,
    find_pending_review_step,
    insert_group_run_assistant_message, insert_group_run_event,
    insert_group_run_record, insert_group_run_step_seed,
    insert_session_message, insert_session_seed, insert_tx_session_message,
    list_group_run_execute_outputs, list_pending_execute_step_ids, list_session_message_rows, load_group_run_blocking_counts,
    mark_group_run_executing,
    mark_group_run_failed, mark_group_run_finalized, mark_group_run_step_completed,
    mark_group_run_step_dispatched, mark_group_run_step_failed, mark_group_run_waiting_review,
    review_requested_event_exists, clear_group_run_execute_waiting_state, SessionSeedInput,
};
use super::{
    EmployeeGroupRunResult, StartEmployeeGroupRunInput,
};
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{parse_run_stop_reason, RunStopReasonKind};
use crate::agent::tools::{EmployeeManageTool, MemoryTool};
use crate::agent::{AgentExecutor, ToolRegistry};
use crate::commands::chat_runtime_io::extract_assistant_text_content;
use crate::commands::models::resolve_default_model_id_with_pool;
use serde_json::Value;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[path = "profile_service.rs"]
mod profile_service;

#[path = "feishu_service.rs"]
mod feishu_service;

#[path = "routing_service.rs"]
mod routing_service;

#[path = "session_service.rs"]
mod session_service;

#[path = "group_run_service.rs"]
mod group_run_service;

#[path = "group_run_snapshot_service.rs"]
mod group_run_snapshot_service;

#[path = "group_run_action_service.rs"]
mod group_run_action_service;

pub(crate) use profile_service::{
    delete_agent_employee_with_pool, list_agent_employees_with_pool,
    normalize_enabled_scopes_for_storage, resolve_employee_agent_id,
    upsert_agent_employee_with_pool,
};
pub(crate) use feishu_service::save_feishu_employee_association_with_pool;
pub(crate) use routing_service::{
    resolve_target_employees_for_event, resolve_team_entry_employee_for_event_with_pool,
};
pub(crate) use session_service::{
    bridge_inbound_event_to_employee_sessions_with_pool,
    ensure_employee_sessions_for_event_with_pool, link_inbound_event_to_session_with_pool,
};
pub(crate) use group_run_service::{
    cancel_employee_group_run_with_pool, pause_employee_group_run_with_pool,
    resume_employee_group_run_with_pool,
};
pub(crate) use group_run_snapshot_service::{
    get_employee_group_run_snapshot_by_run_id_with_pool,
    get_employee_group_run_snapshot_with_pool,
};
pub(crate) use group_run_action_service::{
    reassign_group_run_step_with_pool, retry_employee_group_run_failed_steps_with_pool,
    review_group_run_step_with_pool,
};

pub(super) async fn load_group_run_execute_step_context(
    pool: &SqlitePool,
    step_id: &str,
) -> Result<
    (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ),
    String,
> {
    let row = find_group_run_execute_step_context(pool, step_id)
        .await?
        .ok_or_else(|| "group run step not found".to_string())?;
    if row.step_type != "execute" {
        return Err("only execute steps can be run".to_string());
    }
    Ok((
        row.step_id,
        row.run_id,
        row.assignee_employee_id,
        row.dispatch_source_employee_id,
        row.existing_session_id,
        row.step_input,
        row.user_goal,
        row.step_type,
    ))
}

pub(super) async fn mark_group_run_step_dispatched_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    dispatch_source_employee_id: &str,
    now: &str,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_step_dispatched(&mut tx, step_id, session_id, now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        step_id,
        "step_dispatched",
        &serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
        now,
    )
    .await?;
    mark_group_run_executing(&mut tx, run_id, assignee_employee_id, now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_step_failed_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    dispatch_source_employee_id: &str,
    error: &str,
    now: &str,
) -> Result<(), String> {
    let failed_summary = error.chars().take(120).collect::<String>();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_step_failed(&mut tx, step_id, error, &failed_summary, session_id, now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        step_id,
        "step_failed",
        &serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "status": "failed",
            "error": error,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
        now,
    )
    .await?;
    mark_group_run_failed(&mut tx, run_id, assignee_employee_id, error, now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_step_completed_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    dispatch_source_employee_id: &str,
    output: &str,
    now: &str,
) -> Result<(), String> {
    let output_summary = output.chars().take(120).collect::<String>();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_step_completed(&mut tx, step_id, output, &output_summary, session_id, now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        step_id,
        "step_completed",
        &serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "status": "completed",
            "output_summary": output_summary,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
        now,
    )
    .await?;
    clear_group_run_execute_waiting_state(&mut tx, run_id, now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn load_group_run_continue_state(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(String, String), String> {
    let normalized_run_id = run_id.trim();
    if normalized_run_id.is_empty() {
        return Err("run_id is required".to_string());
    }
    let run_row = find_group_run_state(pool, normalized_run_id)
        .await?
        .ok_or_else(|| "group run not found".to_string())?;
    Ok((run_row.state, run_row.current_phase))
}

pub(super) async fn maybe_mark_group_run_waiting_review(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<String>, String> {
    let Some(review_row) = find_pending_review_step(pool, run_id).await? else {
        return Ok(None);
    };

    let review_requested_exists = review_requested_event_exists(pool, run_id, &review_row.step_id).await?;
    let default_reason = format!("等待{}审议", review_row.assignee_employee_id.trim());
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_waiting_review(
        &mut tx,
        run_id,
        &review_row.assignee_employee_id,
        &default_reason,
        &now,
    )
    .await?;
    if !review_requested_exists {
        insert_group_run_event(
            &mut tx,
            run_id,
            &review_row.step_id,
            "review_requested",
            &serde_json::json!({
                "assignee_employee_id": review_row.assignee_employee_id,
                "phase": "review",
            })
            .to_string(),
            &now,
        )
        .await?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(Some(review_row.assignee_employee_id))
}

pub(super) async fn list_pending_execute_steps_for_continue(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<String>, String> {
    list_pending_execute_step_ids(pool, run_id).await
}

pub(super) async fn maybe_finalize_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    let (execute_blocking, review_blocking) = load_group_run_blocking_counts(pool, run_id).await?;
    if execute_blocking > 0 || review_blocking > 0 {
        return Ok(());
    }

    let run_row = find_group_run_finalize_state(pool, run_id)
        .await?
        .ok_or_else(|| "group run not found".to_string())?;
    if run_row.state == "done" {
        return Ok(());
    }

    let execute_rows = list_group_run_execute_outputs(pool, run_id).await?;
    let mut summary_lines = vec![
        format!("计划：围绕“{}”的团队执行已完成。", run_row.user_goal.trim()),
        "执行：".to_string(),
    ];
    for (assignee_employee_id, output) in execute_rows {
        summary_lines.push(format!("- {}: {}", assignee_employee_id, output.trim()));
    }
    summary_lines.push("汇报：团队协作已完成，可继续进入人工复核或直接对外回复。".to_string());
    let final_report = summary_lines.join("\n");

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    insert_group_run_assistant_message(&mut tx, &run_row.session_id, &final_report, &now).await?;
    mark_group_run_finalized(&mut tx, run_id, &now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        "",
        "run_completed",
        &serde_json::json!({
            "state": "done",
            "phase": "finalize",
            "summary": final_report,
        })
        .to_string(),
        &now,
    )
    .await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn execute_group_step_in_employee_context_with_pool(
    pool: &SqlitePool,
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
        super::build_group_step_system_prompt(&employee, &session_row.skill_id);
    let user_prompt =
        super::build_group_step_user_prompt(run_id, step_id, user_goal, step_input, &employee);

    let now = chrono::Utc::now().to_rfc3339();
    insert_session_message(pool, session_id, "user", &user_prompt, &now).await?;

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

    let executor = AgentExecutor::with_max_iterations(Arc::clone(&registry), max_iterations);
    let final_messages = match executor
        .execute_turn(
            &model_row.api_format,
            &model_row.base_url,
            &model_row.api_key,
            &model_row.model_name,
            &system_prompt,
            messages,
            |_| {},
            None,
            None,
            allowed_tools.as_deref(),
            PermissionMode::Unrestricted,
            None,
            if session_row.work_dir.trim().is_empty() {
                None
            } else {
                Some(session_row.work_dir.clone())
            },
            Some(max_iterations),
            None,
            None,
            None,
        )
        .await
    {
        Ok(final_messages) => final_messages,
        Err(error) => {
            let error_text = error.to_string();
            let stop_reason = match parse_run_stop_reason(&error_text) {
                Some(reason) => reason,
                None => return Err(error_text),
            };
            if stop_reason.kind != RunStopReasonKind::MaxTurns {
                return Err(error_text);
            }

            let fallback_output = super::build_group_step_iteration_fallback_output(
                &employee,
                user_goal,
                step_input,
                stop_reason
                    .detail
                    .as_deref()
                    .unwrap_or(stop_reason.message.as_str()),
            );
            let finished_at = chrono::Utc::now().to_rfc3339();
            insert_session_message(pool, session_id, "assistant", &fallback_output, &finished_at)
                .await?;
            return Ok(fallback_output);
        }
    };

    let assistant_output = super::extract_assistant_text(&final_messages);
    if assistant_output.trim().is_empty() {
        return Err("employee step execution returned empty assistant output".to_string());
    }

    let finished_at = chrono::Utc::now().to_rfc3339();
    insert_session_message(pool, session_id, "assistant", &assistant_output, &finished_at).await?;

    Ok(assistant_output)
}

pub(super) async fn ensure_group_run_session_with_pool(
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

pub(super) async fn append_group_run_assistant_message_with_pool(
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

pub(super) async fn ensure_group_step_session_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    assignee_employee_id: &str,
    now: &str,
) -> Result<String, String> {
    if let Some(session_id) = find_recent_group_step_session_id(pool, run_id, assignee_employee_id).await? {
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

pub(super) async fn start_employee_group_run_internal_with_pool(
    pool: &SqlitePool,
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
    let rules = super::list_employee_group_rules_with_pool(pool, &group_id).await?;
    let planner_employee_id = super::resolve_group_planner_employee_id(
        &config.entry_employee_id,
        &config.coordinator_employee_id,
        &rules,
    );
    let reviewer_employee_id = super::resolve_group_reviewer_employee_id(
        &config.review_mode,
        &planner_employee_id,
        &rules,
    );
    let (execute_targets, _) = super::select_group_execute_dispatch_targets(
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

    let snapshot = super::continue_employee_group_run_with_pool(pool, &run_id).await?;
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
    use super::super::repo::AgentEmployeeRow;
    use super::super::SaveFeishuEmployeeAssociationInput;
    use super::feishu_service::save_feishu_employee_association_with_pool;
    use super::profile_service::build_agent_employee;
    use super::routing_service::resolve_target_employees_for_event;
    use super::session_service::ensure_employee_sessions_for_event_with_pool;
    use super::group_run_service::resume_employee_group_run_with_pool;
    use super::group_run_snapshot_service::get_group_run_session_id_with_pool;
    use super::group_run_action_service::{
        reassign_group_run_step_with_pool, retry_employee_group_run_failed_steps_with_pool,
        review_group_run_step_with_pool,
    };
    use crate::im::types::{ImEvent, ImEventType};
    use sqlx::SqlitePool;

    #[test]
    fn build_agent_employee_falls_back_to_role_id_and_default_scope() {
        let employee = build_agent_employee(
            AgentEmployeeRow {
                id: "emp-1".to_string(),
                employee_id: String::new(),
                name: "Planner".to_string(),
                role_id: "planner".to_string(),
                persona: "Owns planning".to_string(),
                feishu_open_id: String::new(),
                feishu_app_id: String::new(),
                feishu_app_secret: String::new(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: "D:/work".to_string(),
                openclaw_agent_id: "planner".to_string(),
                routing_priority: 100,
                enabled_scopes_json: "not-json".to_string(),
                enabled: true,
                is_default: true,
                created_at: "2026-03-23T00:00:00Z".to_string(),
                updated_at: "2026-03-23T00:00:00Z".to_string(),
            },
            vec!["skill-a".to_string(), "skill-b".to_string()],
        );

        assert_eq!(employee.employee_id, "planner");
        assert_eq!(employee.enabled_scopes, vec!["app".to_string()]);
        assert_eq!(
            employee.skill_ids,
            vec!["skill-a".to_string(), "skill-b".to_string()]
        );
    }

    #[tokio::test]
    async fn save_feishu_employee_association_rejects_invalid_mode() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        let err = save_feishu_employee_association_with_pool(
            &pool,
            SaveFeishuEmployeeAssociationInput {
                employee_db_id: "employee-db-id".to_string(),
                enabled: true,
                mode: "unsupported".to_string(),
                peer_kind: String::new(),
                peer_id: String::new(),
                priority: 10,
            },
        )
        .await
        .expect_err("invalid mode should fail before db lookup");

        assert_eq!(err, "mode must be default or scoped");
    }

    #[tokio::test]
    async fn resolve_target_employees_for_event_prefers_explicit_role_match() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY NOT NULL,
                employee_id TEXT NOT NULL,
                name TEXT NOT NULL,
                role_id TEXT NOT NULL,
                persona TEXT NOT NULL,
                feishu_open_id TEXT NOT NULL,
                feishu_app_id TEXT NOT NULL,
                feishu_app_secret TEXT NOT NULL,
                primary_skill_id TEXT NOT NULL,
                default_work_dir TEXT NOT NULL,
                openclaw_agent_id TEXT NOT NULL,
                routing_priority INTEGER NOT NULL,
                enabled_scopes_json TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                is_default INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create agent_employees");

        sqlx::query(
            r#"
            CREATE TABLE agent_employee_skills (
                employee_id TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                sort_order INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create agent_employee_skills");

        for (id, employee_id, name, role_id, is_default) in [
            ("emp-default", "default-worker", "Default Worker", "default-role", 1_i64),
            ("emp-target", "target-worker", "Target Worker", "target-role", 0_i64),
        ] {
            sqlx::query(
                r#"
                INSERT INTO agent_employees (
                    id, employee_id, name, role_id, persona, feishu_open_id, feishu_app_id,
                    feishu_app_secret, primary_skill_id, default_work_dir, openclaw_agent_id,
                    routing_priority, enabled_scopes_json, enabled, is_default, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(id)
            .bind(employee_id)
            .bind(name)
            .bind(role_id)
            .bind("persona")
            .bind("")
            .bind("")
            .bind("")
            .bind("builtin-general")
            .bind("D:/work")
            .bind(employee_id)
            .bind(100_i64)
            .bind("[\"feishu\"]")
            .bind(1_i64)
            .bind(is_default)
            .bind("2026-03-23T00:00:00Z")
            .bind("2026-03-23T00:00:00Z")
            .execute(&pool)
            .await
            .expect("insert employee");
        }

        let targeted = resolve_target_employees_for_event(
            &pool,
            &ImEvent {
                channel: "feishu".to_string(),
                event_type: ImEventType::MessageCreated,
                thread_id: "thread-1".to_string(),
                event_id: None,
                message_id: None,
                text: Some("hello team".to_string()),
                role_id: Some("target-role".to_string()),
                account_id: None,
                tenant_id: None,
                sender_id: None,
                chat_type: None,
            },
        )
        .await
        .expect("resolve employees");

        assert_eq!(targeted.len(), 1);
        assert_eq!(targeted[0].employee_id, "target-worker");
    }

    #[tokio::test]
    async fn ensure_employee_sessions_for_event_returns_empty_when_no_employee_matches() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY NOT NULL,
                employee_id TEXT NOT NULL,
                name TEXT NOT NULL,
                role_id TEXT NOT NULL,
                persona TEXT NOT NULL,
                feishu_open_id TEXT NOT NULL,
                feishu_app_id TEXT NOT NULL,
                feishu_app_secret TEXT NOT NULL,
                primary_skill_id TEXT NOT NULL,
                default_work_dir TEXT NOT NULL,
                openclaw_agent_id TEXT NOT NULL,
                routing_priority INTEGER NOT NULL,
                enabled_scopes_json TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                is_default INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create agent_employees");

        sqlx::query(
            r#"
            CREATE TABLE im_routing_bindings (
                id TEXT PRIMARY KEY NOT NULL,
                agent_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                account_id TEXT NOT NULL,
                peer_kind TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                guild_id TEXT NOT NULL,
                team_id TEXT NOT NULL,
                role_ids_json TEXT NOT NULL,
                connector_meta_json TEXT,
                priority INTEGER NOT NULL,
                enabled INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create im_routing_bindings");

        let sessions = ensure_employee_sessions_for_event_with_pool(
            &pool,
            &ImEvent {
                channel: "feishu".to_string(),
                event_type: ImEventType::MessageCreated,
                thread_id: "thread-empty".to_string(),
                event_id: None,
                message_id: None,
                text: Some("hello".to_string()),
                role_id: None,
                account_id: None,
                tenant_id: None,
                sender_id: None,
                chat_type: None,
            },
        )
        .await
        .expect("empty employee set should not fail");

        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn resume_employee_group_run_requires_paused_state() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE group_runs (
                id TEXT PRIMARY KEY NOT NULL,
                group_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                user_goal TEXT NOT NULL,
                state TEXT NOT NULL,
                current_round INTEGER NOT NULL,
                current_phase TEXT NOT NULL,
                entry_session_id TEXT NOT NULL,
                main_employee_id TEXT NOT NULL,
                review_round INTEGER NOT NULL,
                status_reason TEXT NOT NULL,
                template_version TEXT NOT NULL,
                waiting_for_employee_id TEXT NOT NULL,
                waiting_for_user INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create employee_group_runs");

        sqlx::query(
            r#"
            CREATE TABLE group_run_events (
                id TEXT PRIMARY KEY NOT NULL,
                run_id TEXT NOT NULL,
                step_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create group_run_events");

        sqlx::query(
            r#"
            INSERT INTO group_runs (
                id, group_id, session_id, user_goal, state, current_round, current_phase,
                entry_session_id, main_employee_id, review_round, status_reason, template_version,
                waiting_for_employee_id, waiting_for_user, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind("run-1")
        .bind("group-1")
        .bind("session-1")
        .bind("Ship feature")
        .bind("planning")
        .bind(1_i64)
        .bind("plan")
        .bind("session-1")
        .bind("coordinator-1")
        .bind(0_i64)
        .bind("")
        .bind("")
        .bind("")
        .bind(0_i64)
        .bind("2026-03-23T00:00:00Z")
        .bind("2026-03-23T00:00:00Z")
        .execute(&pool)
        .await
        .expect("insert group run");

        let err = resume_employee_group_run_with_pool(&pool, "run-1")
            .await
            .expect_err("non-paused run should be rejected");

        assert_eq!(err, "group run is not paused");
    }

    #[tokio::test]
    async fn get_group_run_session_id_returns_not_found_for_missing_run() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE group_runs (
                id TEXT PRIMARY KEY NOT NULL,
                session_id TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create group_runs");

        let err = get_group_run_session_id_with_pool(&pool, "missing-run")
            .await
            .expect_err("missing run should fail");

        assert_eq!(err, "group run not found");
    }

    #[tokio::test]
    async fn retry_employee_group_run_failed_steps_requires_failed_rows() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            "CREATE TABLE group_run_steps (id TEXT PRIMARY KEY NOT NULL, run_id TEXT NOT NULL, status TEXT NOT NULL, output TEXT NOT NULL)"
        )
        .execute(&pool)
        .await
        .expect("create group_run_steps");

        let err = retry_employee_group_run_failed_steps_with_pool(&pool, "run-empty")
            .await
            .expect_err("retry should reject when no failed rows exist");

        assert_eq!(err, "no failed steps to retry");
    }

    #[tokio::test]
    async fn reassign_group_run_step_requires_assignee() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        let err = reassign_group_run_step_with_pool(&pool, "step-1", "   ")
            .await
            .expect_err("blank assignee should fail");

        assert_eq!(err, "assignee_employee_id is required");
    }

    #[tokio::test]
    async fn review_group_run_step_requires_valid_action() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        let err = review_group_run_step_with_pool(&pool, "run-1", "hold", "comment")
            .await
            .expect_err("unsupported action should fail");

        assert_eq!(err, "review action must be approve or reject");
    }
}
