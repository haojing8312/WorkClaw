pub(crate) use crate::agent::runtime::runtime_io::*;

use crate::commands::employee_agents::maybe_handle_team_entry_session_message_with_pool;
use crate::session_journal::SessionJournalStore;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

pub(crate) async fn maybe_handle_team_entry_pre_execution_with_pool(
    app: &AppHandle,
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    user_message_id: &str,
    user_message: &str,
) -> Result<bool, String> {
    let Some(group_run) =
        maybe_handle_team_entry_session_message_with_pool(pool, session_id, user_message).await?
    else {
        return Ok(false);
    };

    let run_id = Uuid::new_v4().to_string();
    append_run_started_with_pool(pool, journal, session_id, &run_id, user_message_id).await?;
    finalize_run_success_with_pool(
        pool,
        journal,
        session_id,
        &run_id,
        &group_run.final_report,
        false,
        &group_run.final_report,
        "",
        None,
        None,
    )
    .await?;

    let _ = app.emit(
        "stream-token",
        crate::agent::runtime::StreamToken {
            session_id: session_id.to_string(),
            token: group_run.final_report.clone(),
            done: false,
            sub_agent: false,
        },
    );
    let _ = app.emit(
        "stream-token",
        crate::agent::runtime::StreamToken {
            session_id: session_id.to_string(),
            token: String::new(),
            done: true,
            sub_agent: false,
        },
    );

    Ok(true)
}
