use super::super::repo::{
    complete_failed_group_run_step, employee_exists_for_reassignment,
    find_group_run_review_state, find_group_run_step_reassign_row, find_plan_revision_seed,
    insert_group_run_event, insert_plan_revision_step, list_failed_execute_assignees,
    list_failed_group_run_steps, mark_group_run_done_after_retry, mark_group_run_review_approved,
    mark_group_run_review_rejected, mark_review_step_completed,
    reset_group_run_step_for_reassignment, update_group_run_after_reassignment,
};
use sqlx::SqlitePool;
use uuid::Uuid;

pub(crate) async fn retry_employee_group_run_failed_steps_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    let failed_rows = list_failed_group_run_steps(pool, run_id).await?;
    if failed_rows.is_empty() {
        return Err("no failed steps to retry".to_string());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    for row in failed_rows {
        let retried_output = if row.output.trim().is_empty() {
            "重试后完成".to_string()
        } else {
            format!("{}\n重试后完成", row.output)
        };
        complete_failed_group_run_step(&mut tx, &row.step_id, &retried_output, &now).await?;
    }
    mark_group_run_done_after_retry(&mut tx, run_id, &now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn reassign_group_run_step_with_pool(
    pool: &SqlitePool,
    step_id: &str,
    assignee_employee_id: &str,
) -> Result<(), String> {
    let new_assignee = assignee_employee_id.trim().to_lowercase();
    if new_assignee.is_empty() {
        return Err("assignee_employee_id is required".to_string());
    }

    let step_row = find_group_run_step_reassign_row(pool, step_id)
        .await?
        .ok_or_else(|| "group run step not found".to_string())?;
    if step_row.step_type != "execute" {
        return Err("only execute steps can be reassigned".to_string());
    }
    if step_row.status != "failed" && step_row.status != "pending" {
        return Err("only failed or pending steps can be reassigned".to_string());
    }

    if !employee_exists_for_reassignment(pool, &new_assignee).await? {
        return Err("target employee not found".to_string());
    }

    let (eligible_targets, has_execute_rules) = super::super::load_execute_reassignment_targets_with_pool(
        pool,
        &step_row.run_id,
        Some(step_row.dispatch_source_employee_id.as_str()),
    )
    .await?;
    if has_execute_rules && !eligible_targets.iter().any(|candidate| candidate == &new_assignee) {
        return Err("target employee is not eligible for execute reassignment".to_string());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    reset_group_run_step_for_reassignment(&mut tx, step_id, &new_assignee).await?;

    let remaining_failed_assignees = list_failed_execute_assignees(&mut tx, &step_row.run_id).await?;
    if remaining_failed_assignees.is_empty() {
        update_group_run_after_reassignment(
            &mut tx,
            &step_row.run_id,
            "executing",
            &new_assignee,
            "",
            &now,
        )
        .await?;
    } else {
        let waiting_for_employee_id = remaining_failed_assignees[0].clone();
        let status_reason = format!("{}执行失败", remaining_failed_assignees.join("、"));
        update_group_run_after_reassignment(
            &mut tx,
            &step_row.run_id,
            "failed",
            &waiting_for_employee_id,
            &status_reason,
            &now,
        )
        .await?;
    }

    let previous_output_summary = if step_row.previous_output_summary.trim().is_empty() {
        step_row.previous_output.chars().take(120).collect::<String>()
    } else {
        step_row.previous_output_summary
    };
    insert_group_run_event(
        &mut tx,
        &step_row.run_id,
        step_id,
        "step_reassigned",
        &serde_json::json!({
            "assignee_employee_id": new_assignee,
            "dispatch_source_employee_id": step_row.dispatch_source_employee_id,
            "previous_assignee_employee_id": step_row.previous_assignee_employee_id,
            "previous_output_summary": previous_output_summary,
        })
        .to_string(),
        &now,
    )
    .await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn review_group_run_step_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    action: &str,
    comment: &str,
) -> Result<(), String> {
    let normalized_action = action.trim().to_lowercase();
    if normalized_action != "approve" && normalized_action != "reject" {
        return Err("review action must be approve or reject".to_string());
    }

    let review_state = find_group_run_review_state(pool, run_id)
        .await?
        .ok_or_else(|| "group run not found".to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let review_status = if normalized_action == "approve" {
        "approved"
    } else {
        "rejected"
    };
    mark_review_step_completed(
        &mut tx,
        &review_state.review_step_id,
        comment,
        review_status,
        &now,
    )
    .await?;

    if normalized_action == "reject" {
        let next_review_round = review_state.review_round + 1;
        let revision_seed = find_plan_revision_seed(&mut tx, run_id)
            .await?
            .unwrap_or_else(|| super::super::repo::PlanRevisionSeedRow {
                input: String::new(),
                assignee_employee_id: review_state.main_employee_id.clone(),
            });
        let revision_assignee_employee_id = if revision_seed.assignee_employee_id.trim().is_empty() {
            review_state.main_employee_id.clone()
        } else {
            revision_seed.assignee_employee_id.trim().to_lowercase()
        };
        let revision_step_id = Uuid::new_v4().to_string();
        insert_plan_revision_step(
            &mut tx,
            &revision_step_id,
            run_id,
            &review_state.review_step_id,
            &revision_assignee_employee_id,
            &revision_seed.input,
            comment,
            next_review_round,
        )
        .await?;
        mark_group_run_review_rejected(
            &mut tx,
            run_id,
            next_review_round,
            comment,
            &revision_assignee_employee_id,
            &now,
        )
        .await?;
        insert_group_run_event(
            &mut tx,
            run_id,
            &review_state.review_step_id,
            "review_rejected",
            &serde_json::json!({
                "reason": comment,
                "review_round": next_review_round,
            })
            .to_string(),
            &now,
        )
        .await?;
        insert_group_run_event(
            &mut tx,
            run_id,
            &revision_step_id,
            "step_created",
            &serde_json::json!({
                "phase": "plan",
                "step_type": "plan",
                "status": "pending",
            })
            .to_string(),
            &now,
        )
        .await?;
    } else {
        mark_group_run_review_approved(&mut tx, run_id, &now).await?;
        insert_group_run_event(
            &mut tx,
            run_id,
            &review_state.review_step_id,
            "review_passed",
            &serde_json::json!({
                "comment": comment,
            })
            .to_string(),
            &now,
        )
        .await?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}
