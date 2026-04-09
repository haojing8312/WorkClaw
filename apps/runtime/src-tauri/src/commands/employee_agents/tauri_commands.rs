use super::*;
use crate::commands::skills::DbState;
use crate::session_journal::SessionJournalStateHandle;
use tauri::State;

pub async fn create_employee_group(
    input: CreateEmployeeGroupInput,
    db: State<'_, DbState>,
) -> Result<String, String> {
    create_employee_group_with_pool(&db.0, input).await
}

pub async fn create_employee_team(
    input: CreateEmployeeTeamInput,
    db: State<'_, DbState>,
) -> Result<String, String> {
    create_employee_team_with_pool(&db.0, input).await
}

pub async fn clone_employee_group_template(
    input: CloneEmployeeGroupTemplateInput,
    db: State<'_, DbState>,
) -> Result<String, String> {
    clone_employee_group_template_with_pool(&db.0, input).await
}

pub async fn list_employee_groups(db: State<'_, DbState>) -> Result<Vec<EmployeeGroup>, String> {
    list_employee_groups_with_pool(&db.0).await
}

pub async fn list_employee_group_runs(
    limit: Option<i64>,
    db: State<'_, DbState>,
) -> Result<Vec<EmployeeGroupRunSummary>, String> {
    list_employee_group_runs_with_pool(&db.0, limit).await
}

pub async fn list_employee_group_rules(
    group_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<EmployeeGroupRule>, String> {
    list_employee_group_rules_with_pool(&db.0, group_id.trim()).await
}

pub async fn delete_employee_group(group_id: String, db: State<'_, DbState>) -> Result<(), String> {
    delete_employee_group_with_pool(&db.0, &group_id).await
}

pub async fn start_employee_group_run(
    input: StartEmployeeGroupRunInput,
    db: State<'_, DbState>,
    journal: State<'_, SessionJournalStateHandle>,
) -> Result<EmployeeGroupRunResult, String> {
    start_employee_group_run_with_pool_and_journal(&db.0, journal.0.as_ref(), input).await
}

pub async fn continue_employee_group_run(
    run_id: String,
    db: State<'_, DbState>,
    journal: State<'_, SessionJournalStateHandle>,
) -> Result<EmployeeGroupRunSnapshot, String> {
    continue_employee_group_run_with_pool_and_journal(
        &db.0,
        Some(journal.0.as_ref()),
        run_id.trim(),
    )
    .await
}

pub async fn run_group_step(
    step_id: String,
    db: State<'_, DbState>,
    journal: State<'_, SessionJournalStateHandle>,
) -> Result<GroupStepExecutionResult, String> {
    run_group_step_with_pool_and_journal(&db.0, Some(journal.0.as_ref()), step_id.trim()).await
}

pub async fn get_employee_group_run_snapshot(
    session_id: String,
    db: State<'_, DbState>,
) -> Result<Option<EmployeeGroupRunSnapshot>, String> {
    get_employee_group_run_snapshot_with_pool(&db.0, session_id.trim()).await
}

pub async fn cancel_employee_group_run(
    run_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    cancel_employee_group_run_with_pool(&db.0, run_id.trim()).await
}

pub async fn retry_employee_group_run_failed_steps(
    run_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    retry_employee_group_run_failed_steps_with_pool(&db.0, run_id.trim()).await
}

pub async fn review_group_run_step(
    run_id: String,
    action: String,
    comment: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    review_group_run_step_with_pool(&db.0, run_id.trim(), action.trim(), comment.trim()).await
}

pub async fn pause_employee_group_run(
    run_id: String,
    reason: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    pause_employee_group_run_with_pool(&db.0, run_id.trim(), reason.as_deref().unwrap_or("")).await
}

pub async fn resume_employee_group_run(
    run_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    resume_employee_group_run_with_pool(&db.0, run_id.trim()).await
}

pub async fn reassign_group_run_step(
    step_id: String,
    assignee_employee_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    reassign_group_run_step_with_pool(&db.0, step_id.trim(), assignee_employee_id.trim()).await
}

pub async fn list_agent_employees(db: State<'_, DbState>) -> Result<Vec<AgentEmployee>, String> {
    list_agent_employees_with_pool(&db.0).await
}

pub async fn upsert_agent_employee(
    input: UpsertAgentEmployeeInput,
    db: State<'_, DbState>,
    relay: State<'_, crate::commands::feishu_gateway::FeishuEventRelayState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let id = upsert_agent_employee_with_pool(&db.0, input).await?;
    let _ = crate::commands::feishu_gateway::reconcile_feishu_employee_connections_with_pool(
        &db.0, None,
    )
    .await;
    let _ = crate::commands::feishu_gateway::start_feishu_event_relay_with_pool_and_app(
        &db.0,
        relay.inner().clone(),
        Some(app),
        None,
        Some(1500),
        Some(50),
    )
    .await;
    Ok(id)
}

pub async fn save_feishu_employee_association(
    input: SaveFeishuEmployeeAssociationInput,
    db: State<'_, DbState>,
    relay: State<'_, crate::commands::feishu_gateway::FeishuEventRelayState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    save_feishu_employee_association_with_pool(&db.0, input).await?;
    let _ = crate::commands::feishu_gateway::reconcile_feishu_employee_connections_with_pool(
        &db.0, None,
    )
    .await;
    let _ = crate::commands::feishu_gateway::start_feishu_event_relay_with_pool_and_app(
        &db.0,
        relay.inner().clone(),
        Some(app),
        None,
        Some(1500),
        Some(50),
    )
    .await;
    Ok(())
}

pub async fn delete_agent_employee(
    employee_id: String,
    db: State<'_, DbState>,
    relay: State<'_, crate::commands::feishu_gateway::FeishuEventRelayState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    delete_agent_employee_with_pool(&db.0, &employee_id).await?;
    let _ = crate::commands::feishu_gateway::reconcile_feishu_employee_connections_with_pool(
        &db.0, None,
    )
    .await;
    let _ = crate::commands::feishu_gateway::start_feishu_event_relay_with_pool_and_app(
        &db.0,
        relay.inner().clone(),
        Some(app),
        None,
        Some(1500),
        Some(50),
    )
    .await;
    Ok(())
}
