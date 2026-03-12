mod helpers;

use runtime_lib::commands::session_runs::{
    append_session_run_event_with_pool, attach_assistant_message_to_run_with_pool,
    list_session_runs_with_pool,
};
use runtime_lib::session_journal::{SessionJournalStore, SessionRunEvent};

#[tokio::test]
async fn run_started_is_projected_before_assistant_message_exists() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let journal_dir = tempfile::tempdir().expect("create journal dir");
    let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-1",
        SessionRunEvent::RunStarted {
            run_id: "run-1".into(),
            user_message_id: "user-1".into(),
        },
    )
    .await
    .expect("append run started");

    let runs = list_session_runs_with_pool(&pool, "sess-1")
        .await
        .expect("list session runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, "run-1");
    assert_eq!(runs[0].user_message_id, "user-1");
    assert_eq!(runs[0].status, "thinking");
    assert_eq!(runs[0].buffered_text, "");

    let journal_state = journal.read_state("sess-1").await.expect("read state");
    assert_eq!(journal_state.current_run_id.as_deref(), Some("run-1"));
}

#[tokio::test]
async fn failed_run_remains_visible_without_assistant_message_row() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let journal_dir = tempfile::tempdir().expect("create journal dir");
    let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-err",
        SessionRunEvent::RunStarted {
            run_id: "run-err".into(),
            user_message_id: "user-err".into(),
        },
    )
    .await
    .expect("append run started");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-err",
        SessionRunEvent::RunFailed {
            run_id: "run-err".into(),
            error_kind: "insufficient_balance".into(),
            error_message: "insufficient_balance: account balance too low".into(),
        },
    )
    .await
    .expect("append run failed");

    let runs = list_session_runs_with_pool(&pool, "sess-err")
        .await
        .expect("list session runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "failed");
    assert_eq!(runs[0].error_kind.as_deref(), Some("insufficient_balance"));
    assert!(runs[0]
        .error_message
        .as_deref()
        .unwrap_or_default()
        .contains("insufficient_balance"));

    let (message_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM messages WHERE session_id = ?")
            .bind("sess-err")
            .fetch_one(&pool)
            .await
            .expect("count messages");
    assert_eq!(message_count, 0);

    let (event_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM session_run_events WHERE run_id = ?")
            .bind("run-err")
            .fetch_one(&pool)
            .await
            .expect("count run events");
    assert_eq!(event_count, 2);
}

#[tokio::test]
async fn assistant_message_binding_is_projected_with_run() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let journal_dir = tempfile::tempdir().expect("create journal dir");
    let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-bind",
        SessionRunEvent::RunStarted {
            run_id: "run-bind".into(),
            user_message_id: "user-bind".into(),
        },
    )
    .await
    .expect("append run started");

    attach_assistant_message_to_run_with_pool(&pool, "run-bind", "assistant-bind")
        .await
        .expect("attach assistant message");

    let runs = list_session_runs_with_pool(&pool, "sess-bind")
        .await
        .expect("list session runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, "run-bind");
    assert_eq!(
        runs[0].assistant_message_id.as_deref(),
        Some("assistant-bind")
    );
}
