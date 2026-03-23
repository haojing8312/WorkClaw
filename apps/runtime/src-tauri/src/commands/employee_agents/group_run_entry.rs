use super::{
    default_group_execution_window, default_group_max_retry, list_employee_groups_with_pool,
    AgentEmployee, EmployeeGroupRunResult, EmployeeGroupRunSnapshot, GroupStepExecutionResult,
    StartEmployeeGroupRunInput,
};
use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::skill_config::SkillConfig;
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) async fn start_employee_group_run_with_pool(
    pool: &SqlitePool,
    input: StartEmployeeGroupRunInput,
) -> Result<EmployeeGroupRunResult, String> {
    start_employee_group_run_internal_with_pool(pool, input, None, true).await
}

pub(crate) async fn start_employee_group_run_internal_with_pool(
    pool: &SqlitePool,
    input: StartEmployeeGroupRunInput,
    preferred_session_id: Option<&str>,
    persist_user_message: bool,
) -> Result<EmployeeGroupRunResult, String> {
    super::service::start_employee_group_run_internal_with_pool(
        pool,
        input,
        preferred_session_id,
        persist_user_message,
    )
    .await
}

pub(crate) async fn ensure_group_step_session_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    assignee_employee_id: &str,
    now: &str,
) -> Result<String, String> {
    super::service::ensure_group_step_session_with_pool(pool, run_id, assignee_employee_id, now)
        .await
}

fn load_group_step_profile_markdown(employee: &AgentEmployee) -> String {
    if employee.default_work_dir.trim().is_empty() {
        return String::new();
    }

    let profile_dir = PathBuf::from(employee.default_work_dir.trim())
        .join("openclaw")
        .join(employee.employee_id.trim());
    let mut sections = Vec::new();
    for name in ["AGENTS.md", "SOUL.md", "USER.md"] {
        let path = profile_dir.join(name);
        if let Ok(content) = std::fs::read_to_string(path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                sections.push(format!("## {name}\n{trimmed}"));
            }
        }
    }
    sections.join("\n\n")
}

fn default_group_step_allowed_tools() -> Vec<String> {
    vec![
        "read_file".to_string(),
        "write_file".to_string(),
        "glob".to_string(),
        "grep".to_string(),
        "edit".to_string(),
        "list_dir".to_string(),
        "file_stat".to_string(),
        "file_copy".to_string(),
        "bash".to_string(),
        "web_fetch".to_string(),
    ]
}

pub(crate) fn build_group_step_iteration_fallback_output(
    employee: &AgentEmployee,
    user_goal: &str,
    step_input: &str,
    error: &str,
) -> String {
    let focus = if step_input.trim().is_empty() {
        user_goal.trim()
    } else {
        step_input.trim()
    };
    let responsibility = if employee.persona.trim().is_empty() {
        format!("负责围绕“{}”完成分配到本岗位的执行项", focus)
    } else {
        employee.persona.trim().to_string()
    };
    format!(
        "{} ({}) 在执行步骤时触发了迭代上限，现切换为保守交付模式。\n- 当前步骤: {}\n- 岗位职责: {}\n- 对用户目标“{}”可立即提供: 基于本岗位职责给出能力范围说明、所需补充信息以及下一步执行建议。\n- 备注: {}",
        employee.name,
        employee.employee_id,
        focus,
        responsibility,
        user_goal.trim(),
        error.trim(),
    )
}

pub(crate) fn build_group_step_system_prompt(
    employee: &AgentEmployee,
    session_skill_id: &str,
) -> (String, Option<Vec<String>>, usize) {
    let skill_config = SkillConfig::parse(crate::builtin_skills::builtin_general_skill_markdown());
    let base_prompt = if skill_config.system_prompt.trim().is_empty() {
        "你是一名专业、可靠、注重交付结果的 AI 员工。".to_string()
    } else {
        skill_config.system_prompt.clone()
    };
    let profile_markdown = load_group_step_profile_markdown(employee);
    let mut sections = vec![
        base_prompt,
        "---".to_string(),
        "你当前正在复杂任务团队中，以真实员工身份执行内部步骤。".to_string(),
        format!("- 员工名称: {}", employee.name),
        format!("- employee_id: {}", employee.employee_id),
        format!("- role_id: {}", employee.role_id),
        format!(
            "- primary_skill_id: {}",
            if session_skill_id.trim().is_empty() {
                "builtin-general"
            } else {
                session_skill_id.trim()
            }
        ),
    ];
    if !employee.default_work_dir.trim().is_empty() {
        sections.push(format!("- 工作目录: {}", employee.default_work_dir.trim()));
    }
    if !employee.persona.trim().is_empty() {
        sections.push(format!("- 员工人设: {}", employee.persona.trim()));
    }
    sections.push(
        "执行要求:\n- 聚焦当前分配步骤\n- 优先直接用自然语言给出结论，只有在当前步骤明确需要读取文件、编辑文件、执行命令或抓取网页时才使用工具\n- 先给结论，再给关键依据或产出\n- 不要输出“模拟结果”或“占位结果”措辞".to_string(),
    );
    if !profile_markdown.is_empty() {
        sections.push(format!("员工资料:\n{profile_markdown}"));
    }
    (
        sections.join("\n"),
        Some(default_group_step_allowed_tools()),
        RunBudgetPolicy::resolve(RunBudgetScope::Employee, skill_config.max_iterations).max_turns,
    )
}

pub(crate) fn build_group_step_user_prompt(
    run_id: &str,
    step_id: &str,
    user_goal: &str,
    step_input: &str,
    employee: &AgentEmployee,
) -> String {
    let effective_input = if step_input.trim().is_empty() {
        user_goal.trim()
    } else {
        step_input.trim()
    };
    format!(
        "你正在执行多员工团队中的 execute 步骤。\n- run_id: {run_id}\n- step_id: {step_id}\n- 当前负责人: {} ({})\n- 用户总目标: {}\n- 当前步骤要求: {}\n\n请直接给出你的执行结果。如果信息不足，先指出缺口，再给最合理的下一步。",
        employee.name,
        employee.employee_id,
        user_goal.trim(),
        effective_input,
    )
}

pub(crate) fn extract_assistant_text(messages: &[Value]) -> String {
    messages
        .iter()
        .rev()
        .find_map(|message| {
            if message["role"].as_str() != Some("assistant") {
                return None;
            }
            if let Some(content) = message["content"].as_str() {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            message["content"].as_array().map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| {
                        if block["type"].as_str() == Some("text") {
                            block["text"].as_str().map(str::trim).map(str::to_string)
                        } else {
                            None
                        }
                    })
                    .filter(|text| !text.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        })
        .unwrap_or_default()
}

async fn execute_group_step_in_employee_context_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    user_goal: &str,
    step_input: &str,
) -> Result<String, String> {
    super::service::execute_group_step_in_employee_context_with_pool(
        pool,
        run_id,
        step_id,
        session_id,
        assignee_employee_id,
        user_goal,
        step_input,
    )
    .await
}

async fn maybe_finalize_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    super::service::maybe_finalize_group_run_with_pool(pool, run_id).await
}

async fn get_employee_group_run_snapshot_by_run_id_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<EmployeeGroupRunSnapshot, String> {
    super::service::get_employee_group_run_snapshot_by_run_id_with_pool(pool, run_id).await
}

async fn get_group_run_reviewer_employee_id_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<String>, String> {
    let run_row = sqlx::query(
        "SELECT r.group_id, COALESCE(g.review_mode, 'none')
         FROM group_runs r
         INNER JOIN employee_groups g ON g.id = r.group_id
         WHERE r.id = ?",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "group run not found".to_string())?;
    let group_id: String = run_row.try_get(0).map_err(|e| e.to_string())?;
    let review_mode: String = run_row.try_get(1).map_err(|e| e.to_string())?;
    if review_mode.eq_ignore_ascii_case("none") {
        return Ok(None);
    }

    let reviewer = sqlx::query_as::<_, (String,)>(
        "SELECT to_employee_id
         FROM employee_group_rules
         WHERE group_id = ? AND relation_type = 'review'
         ORDER BY priority DESC, created_at ASC
         LIMIT 1",
    )
    .bind(&group_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .map(|(employee_id,)| employee_id.trim().to_string())
    .filter(|employee_id| !employee_id.is_empty());

    Ok(reviewer)
}

async fn advance_pending_plan_revision_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<bool, String> {
    let pending_plan_row = sqlx::query(
        "SELECT id, assignee_employee_id, COALESCE(input, ''), COALESCE(input_summary, '')
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'plan' AND status = 'pending'
         ORDER BY round_no DESC, id DESC
         LIMIT 1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some(pending_plan_row) = pending_plan_row else {
        return Ok(false);
    };

    let step_id: String = pending_plan_row.try_get(0).map_err(|e| e.to_string())?;
    let assignee_employee_id: String = pending_plan_row.try_get(1).map_err(|e| e.to_string())?;
    let step_input: String = pending_plan_row.try_get(2).map_err(|e| e.to_string())?;
    let revision_comment: String = pending_plan_row.try_get(3).map_err(|e| e.to_string())?;
    let reviewer_employee_id = get_group_run_reviewer_employee_id_with_pool(pool, run_id).await?;
    let now = chrono::Utc::now().to_rfc3339();
    let revision_output = if revision_comment.trim().is_empty() {
        "已重新整理计划，等待下一阶段推进".to_string()
    } else {
        format!("已根据审议意见修订计划：{}", revision_comment.trim())
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'completed',
             output = ?,
             output_summary = ?,
             review_status = ?,
             started_at = CASE
               WHEN TRIM(started_at) = '' THEN ?
               ELSE started_at
             END,
             finished_at = ?
         WHERE id = ?",
    )
    .bind(&revision_output)
    .bind(&revision_output)
    .bind(if reviewer_employee_id.is_some() {
        "pending"
    } else {
        "not_required"
    })
    .bind(&now)
    .bind(&now)
    .bind(&step_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
         VALUES (?, ?, ?, 'step_completed', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(run_id)
    .bind(&step_id)
    .bind(
        serde_json::json!({
            "phase": "plan",
            "step_type": "plan",
            "assignee_employee_id": assignee_employee_id,
            "status": "completed",
            "revision_comment": revision_comment,
        })
        .to_string(),
    )
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    if let Some(reviewer_employee_id) = reviewer_employee_id {
        let review_step_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO group_run_steps (
                id, run_id, round_no, parent_step_id, assignee_employee_id, phase, step_type, step_kind,
                input, input_summary, output, output_summary, status, requires_review, review_status,
                attempt_no, session_id, visibility, started_at, finished_at
             ) VALUES (?, ?, ?, ?, ?, 'review', 'review', 'review', ?, ?, '等待审核计划', '', 'pending', 0, 'pending', 0, '', 'internal', '', '')",
        )
        .bind(&review_step_id)
        .bind(run_id)
        .bind(0_i64)
        .bind(&step_id)
        .bind(&reviewer_employee_id)
        .bind(&step_input)
        .bind(&revision_output)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
             VALUES (?, ?, ?, 'step_created', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(run_id)
        .bind(&review_step_id)
        .bind(
            serde_json::json!({
                "phase": "review",
                "step_type": "review",
                "assignee_employee_id": reviewer_employee_id,
                "status": "pending",
            })
            .to_string(),
        )
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    sqlx::query(
        "UPDATE group_runs
         SET state = 'planning',
             current_phase = 'plan',
             waiting_for_employee_id = '',
             status_reason = '',
             updated_at = ?
         WHERE id = ?",
    )
    .bind(&now)
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(true)
}

pub(crate) async fn continue_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<EmployeeGroupRunSnapshot, String> {
    let normalized_run_id = run_id.trim();
    let (state, current_phase) =
        super::service::load_group_run_continue_state(pool, normalized_run_id).await?;

    if state == "paused" {
        return Err("group run is paused".to_string());
    }
    if state == "cancelled" || state == "done" {
        return get_employee_group_run_snapshot_by_run_id_with_pool(pool, normalized_run_id).await;
    }

    let _ = advance_pending_plan_revision_with_pool(pool, normalized_run_id).await?;

    if super::service::maybe_mark_group_run_waiting_review(pool, normalized_run_id)
        .await?
        .is_some()
    {
        return get_employee_group_run_snapshot_by_run_id_with_pool(pool, normalized_run_id).await;
    }

    let pending_execute_steps =
        super::service::list_pending_execute_steps_for_continue(pool, normalized_run_id).await?;

    if pending_execute_steps.is_empty() && current_phase == "review" {
        return get_employee_group_run_snapshot_by_run_id_with_pool(pool, normalized_run_id).await;
    }

    for step_id in pending_execute_steps {
        run_group_step_with_pool(pool, &step_id).await?;
    }
    maybe_finalize_group_run_with_pool(pool, normalized_run_id).await?;
    get_employee_group_run_snapshot_by_run_id_with_pool(pool, normalized_run_id).await
}

pub(crate) async fn run_group_step_with_pool(
    pool: &SqlitePool,
    step_id: &str,
) -> Result<GroupStepExecutionResult, String> {
    let (
        step_id,
        run_id,
        assignee_employee_id,
        dispatch_source_employee_id,
        existing_session_id,
        step_input,
        user_goal,
        _step_type,
    ) = super::service::load_group_run_execute_step_context(pool, step_id).await?;

    let now = chrono::Utc::now().to_rfc3339();
    let session_id = if existing_session_id.trim().is_empty() {
        ensure_group_step_session_with_pool(pool, &run_id, &assignee_employee_id, &now).await?
    } else {
        existing_session_id
    };

    super::service::mark_group_run_step_dispatched_with_pool(
        pool,
        &run_id,
        &step_id,
        &session_id,
        &assignee_employee_id,
        &dispatch_source_employee_id,
        &now,
    )
    .await?;

    let execution = execute_group_step_in_employee_context_with_pool(
        pool,
        &run_id,
        &step_id,
        &session_id,
        &assignee_employee_id,
        &user_goal,
        &step_input,
    )
    .await;

    let now = chrono::Utc::now().to_rfc3339();
    let output = match execution {
        Ok(output) => output,
        Err(error) => {
            super::service::mark_group_run_step_failed_with_pool(
                pool,
                &run_id,
                &step_id,
                &session_id,
                &assignee_employee_id,
                &dispatch_source_employee_id,
                &error,
                &now,
            )
            .await?;
            return Err(error);
        }
    };

    super::service::mark_group_run_step_completed_with_pool(
        pool,
        &run_id,
        &step_id,
        &session_id,
        &assignee_employee_id,
        &dispatch_source_employee_id,
        &output,
        &now,
    )
    .await?;
    maybe_finalize_group_run_with_pool(pool, &run_id).await?;

    Ok(GroupStepExecutionResult {
        step_id,
        run_id,
        assignee_employee_id,
        session_id,
        status: "completed".to_string(),
        output,
    })
}

pub(crate) async fn maybe_handle_team_entry_session_message_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    user_message: &str,
) -> Result<Option<EmployeeGroupRunResult>, String> {
    let normalized_session_id = session_id.trim();
    if normalized_session_id.is_empty() {
        return Ok(None);
    }
    let normalized_user_message = user_message.trim();
    if normalized_user_message.is_empty() {
        return Ok(None);
    }

    let session_row = sqlx::query(
        "SELECT COALESCE(session_mode, 'general'), COALESCE(team_id, '')
         FROM sessions
         WHERE id = ?",
    )
    .bind(normalized_session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some(session_row) = session_row else {
        return Ok(None);
    };
    let session_mode: String = session_row.try_get(0).map_err(|e| e.to_string())?;
    let team_id: String = session_row.try_get(1).map_err(|e| e.to_string())?;
    if !session_mode.trim().eq_ignore_ascii_case("team_entry") || team_id.trim().is_empty() {
        return Ok(None);
    }

    let Some(group) = list_employee_groups_with_pool(pool)
        .await?
        .into_iter()
        .find(|group| group.id.eq_ignore_ascii_case(team_id.trim()))
    else {
        return Ok(None);
    };

    let result = start_employee_group_run_internal_with_pool(
        pool,
        StartEmployeeGroupRunInput {
            group_id: group.id,
            user_goal: normalized_user_message.to_string(),
            execution_window: default_group_execution_window(),
            timeout_employee_ids: Vec::new(),
            max_retry_per_step: default_group_max_retry(),
        },
        Some(normalized_session_id),
        false,
    )
    .await?;

    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::extract_assistant_text;

    #[test]
    fn extract_assistant_text_prefers_latest_nonempty_assistant_content() {
        let messages = vec![
            serde_json::json!({"role":"assistant","content":"first"}),
            serde_json::json!({"role":"assistant","content":[
                {"type":"text","text":"  "},
                {"type":"text","text":"latest"}
            ]}),
        ];

        assert_eq!(extract_assistant_text(&messages), "latest");
    }
}
