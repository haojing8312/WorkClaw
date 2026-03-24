use super::{
    CloneEmployeeGroupTemplateInput, CreateEmployeeGroupInput, CreateEmployeeTeamInput,
    EmployeeGroup, EmployeeGroupRule, EmployeeGroupRunResult, EmployeeGroupRunSnapshot,
    StartEmployeeGroupRunInput,
};
use sqlx::SqlitePool;

pub async fn create_employee_group_with_pool(
    pool: &SqlitePool,
    input: CreateEmployeeGroupInput,
) -> Result<String, String> {
    super::group_management::create_employee_group_with_pool(pool, input).await
}

pub async fn create_employee_team_with_pool(
    pool: &SqlitePool,
    input: CreateEmployeeTeamInput,
) -> Result<String, String> {
    super::group_management::create_employee_team_with_pool(pool, input).await
}

pub async fn clone_employee_group_template_with_pool(
    pool: &SqlitePool,
    input: CloneEmployeeGroupTemplateInput,
) -> Result<String, String> {
    super::group_management::clone_employee_group_template_with_pool(pool, input).await
}

pub async fn delete_employee_group_with_pool(
    pool: &SqlitePool,
    group_id: &str,
) -> Result<(), String> {
    super::group_management::delete_employee_group_with_pool(pool, group_id).await
}

pub async fn list_employee_groups_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<EmployeeGroup>, String> {
    super::group_management::list_employee_groups_with_pool(pool).await
}

pub async fn list_employee_group_rules_with_pool(
    pool: &SqlitePool,
    group_id: &str,
) -> Result<Vec<EmployeeGroupRule>, String> {
    super::group_management::list_employee_group_rules_with_pool(pool, group_id).await
}

pub async fn list_employee_group_runs_with_pool(
    pool: &SqlitePool,
    limit: Option<i64>,
) -> Result<Vec<super::EmployeeGroupRunSummary>, String> {
    super::group_management::list_employee_group_runs_with_pool(pool, limit).await
}

pub async fn continue_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<EmployeeGroupRunSnapshot, String> {
    super::group_run_entry::continue_employee_group_run_with_pool(pool, run_id).await
}

pub async fn start_employee_group_run_with_pool(
    pool: &SqlitePool,
    input: StartEmployeeGroupRunInput,
) -> Result<EmployeeGroupRunResult, String> {
    super::group_run_entry::start_employee_group_run_with_pool(pool, input).await
}

pub async fn run_group_step_with_pool(
    pool: &SqlitePool,
    step_id: &str,
) -> Result<super::GroupStepExecutionResult, String> {
    super::group_run_entry::run_group_step_with_pool(pool, step_id).await
}

pub async fn maybe_handle_team_entry_session_message_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    user_message: &str,
) -> Result<Option<EmployeeGroupRunResult>, String> {
    super::group_run_entry::maybe_handle_team_entry_session_message_with_pool(
        pool,
        session_id,
        user_message,
    )
    .await
}
