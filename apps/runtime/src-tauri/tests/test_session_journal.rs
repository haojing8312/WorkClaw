use runtime_lib::session_journal::{SessionJournalStore, SessionRunEvent, SessionRunStatus};

#[tokio::test]
async fn append_event_persists_jsonl_and_updates_snapshot() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = SessionJournalStore::new(dir.path().to_path_buf());

    store
        .append_event(
            "sess-1",
            SessionRunEvent::RunStarted {
                run_id: "run-1".into(),
                user_message_id: "user-1".into(),
            },
        )
        .await
        .expect("append run started");

    store
        .append_event(
            "sess-1",
            SessionRunEvent::AssistantChunkAppended {
                run_id: "run-1".into(),
                chunk: "hello".into(),
            },
        )
        .await
        .expect("append assistant chunk");

    let snapshot = store.read_state("sess-1").await.expect("read snapshot");
    assert_eq!(snapshot.session_id, "sess-1");
    assert_eq!(snapshot.current_run_id.as_deref(), Some("run-1"));
    assert_eq!(snapshot.runs.len(), 1);
    assert_eq!(snapshot.runs[0].run_id, "run-1");
    assert_eq!(snapshot.runs[0].user_message_id, "user-1");
    assert_eq!(snapshot.runs[0].status, SessionRunStatus::Thinking);
    assert_eq!(snapshot.runs[0].buffered_text, "hello");

    let events_path = dir.path().join("sess-1").join("events.jsonl");
    let raw = tokio::fs::read_to_string(events_path)
        .await
        .expect("read events");
    let lines: Vec<&str> = raw.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"type\":\"run_started\""));
    assert!(lines[1].contains("\"type\":\"assistant_chunk_appended\""));

    let transcript_path = dir.path().join("sess-1").join("transcript.md");
    let transcript = tokio::fs::read_to_string(transcript_path)
        .await
        .expect("read transcript");
    assert!(transcript.contains("# Session sess-1"));
    assert!(transcript.contains("## Run run-1"));
    assert!(transcript.contains("hello"));
}

#[tokio::test]
async fn completed_event_updates_terminal_status() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = SessionJournalStore::new(dir.path().to_path_buf());

    store
        .append_event(
            "sess-2",
            SessionRunEvent::RunStarted {
                run_id: "run-9".into(),
                user_message_id: "user-9".into(),
            },
        )
        .await
        .expect("append run started");

    store
        .append_event(
            "sess-2",
            SessionRunEvent::RunCompleted {
                run_id: "run-9".into(),
            },
        )
        .await
        .expect("append run completed");

    let snapshot = store.read_state("sess-2").await.expect("read snapshot");
    assert_eq!(snapshot.runs[0].status, SessionRunStatus::Completed);
}
