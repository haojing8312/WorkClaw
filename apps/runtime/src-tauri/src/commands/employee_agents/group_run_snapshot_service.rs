use super::super::repo::{
    find_group_run_snapshot_row, find_latest_assistant_message_content, get_group_run_session_id,
    list_group_run_event_snapshot_rows, list_group_run_step_snapshot_rows,
    GroupRunEventSnapshotRow, GroupRunStepSnapshotRow,
};
use sqlx::SqlitePool;

pub(crate) async fn get_group_run_session_id_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<String, String> {
    get_group_run_session_id(pool, run_id)
        .await?
        .ok_or_else(|| "group run not found".to_string())
}

pub(crate) async fn get_employee_group_run_snapshot_by_run_id_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<super::super::EmployeeGroupRunSnapshot, String> {
    let session_id = get_group_run_session_id_with_pool(pool, run_id).await?;
    get_employee_group_run_snapshot_with_pool(pool, &session_id)
        .await?
        .ok_or_else(|| "group run snapshot not found".to_string())
}

pub(crate) async fn get_employee_group_run_snapshot_with_pool(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<super::super::EmployeeGroupRunSnapshot>, String> {
    let Some(run_row) = find_group_run_snapshot_row(pool, session_id).await? else {
        return Ok(None);
    };

    let steps = list_group_run_step_snapshot_rows(pool, &run_row.run_id)
        .await?
        .into_iter()
        .map(map_group_run_step_snapshot)
        .collect::<Vec<_>>();
    let events = list_group_run_event_snapshot_rows(pool, &run_row.run_id)
        .await?
        .into_iter()
        .map(map_group_run_event_snapshot)
        .collect::<Vec<_>>();
    let completed = steps.iter().filter(|step| step.status == "completed").count();
    let final_report = find_latest_assistant_message_content(pool, &run_row.session_id)
        .await?
        .filter(|content| !content.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "计划：围绕“{}”共 {} 步。\n执行：已完成 {} 步。\n汇报：当前状态={}",
                run_row.user_goal,
                steps.len(),
                completed,
                run_row.state
            )
        });

    Ok(Some(super::super::EmployeeGroupRunSnapshot {
        run_id: run_row.run_id,
        group_id: run_row.group_id,
        session_id: run_row.session_id,
        state: run_row.state,
        current_round: run_row.current_round,
        current_phase: run_row.current_phase,
        review_round: run_row.review_round,
        status_reason: run_row.status_reason,
        waiting_for_employee_id: run_row.waiting_for_employee_id,
        waiting_for_user: run_row.waiting_for_user,
        final_report,
        steps,
        events,
    }))
}

fn map_group_run_step_snapshot(
    row: GroupRunStepSnapshotRow,
) -> super::super::EmployeeGroupRunStep {
    super::super::EmployeeGroupRunStep {
        id: row.id,
        round_no: row.round_no,
        step_type: row.step_type,
        assignee_employee_id: row.assignee_employee_id,
        dispatch_source_employee_id: row.dispatch_source_employee_id,
        session_id: row.session_id,
        attempt_no: row.attempt_no,
        status: row.status,
        output_summary: row.output_summary,
        output: row.output,
    }
}

fn map_group_run_event_snapshot(
    row: GroupRunEventSnapshotRow,
) -> super::super::EmployeeGroupRunEvent {
    super::super::EmployeeGroupRunEvent {
        id: row.id,
        step_id: row.step_id,
        event_type: row.event_type,
        payload_json: row.payload_json,
        created_at: row.created_at,
    }
}
