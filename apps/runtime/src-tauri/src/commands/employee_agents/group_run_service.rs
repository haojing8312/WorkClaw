use super::super::repo::{
    cancel_group_run, find_group_run_state, insert_group_run_event, pause_group_run,
    resume_group_run,
};
use sqlx::SqlitePool;

pub(crate) async fn pause_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    reason: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let affected = pause_group_run(&mut tx, run_id, reason.trim(), &now).await?;
    if affected == 0 {
        return Err("group run is not pausable".to_string());
    }
    insert_group_run_event(
        &mut tx,
        run_id,
        "",
        "run_paused",
        &serde_json::json!({ "reason": reason.trim() }).to_string(),
        &now,
    )
    .await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn resume_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    let run_row = find_group_run_state(pool, run_id)
        .await?
        .ok_or_else(|| "group run not found".to_string())?;
    if run_row.state != "paused" {
        return Err("group run is not paused".to_string());
    }

    let resumed_state = match run_row.current_phase.as_str() {
        "execute" => "executing",
        "review" => "waiting_review",
        _ => "planning",
    };
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    resume_group_run(&mut tx, run_id, resumed_state, &now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        "",
        "run_resumed",
        &serde_json::json!({
            "state": resumed_state,
            "phase": run_row.current_phase,
        })
        .to_string(),
        &now,
    )
    .await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn cancel_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    cancel_group_run(pool, run_id, &now).await
}
