use crate::helpers::setup_test_db;
use runtime_lib::agent::runtime::{
    normalize_trace_for_fixture, RunRegistry, RuntimeObservability, RuntimeObservedEvent,
};
use runtime_lib::commands::session_runs::{
    append_session_run_event_with_pool, export_session_run_trace_with_pool,
    list_session_runs_with_pool, SessionRunProjection,
};
use runtime_lib::session_journal::{SessionJournalStore, SessionRunEvent};
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeContractFixtureParams<'a> {
    pub fixture_name: &'a str,
    pub record_admission_conflict: bool,
    pub record_compaction_run: bool,
    pub record_failover_error_kind: Option<&'a str>,
}

#[derive(Debug)]
pub struct RuntimeContractOutcome {
    pub session_runs: Vec<SessionRunProjection>,
    pub normalized_trace: Value,
    pub trace_final_status: String,
    pub trace_child_session_parent: Option<String>,
    pub observability_snapshot: Value,
    pub recent_events: Vec<RuntimeObservedEvent>,
}

#[derive(Debug, Deserialize)]
struct TraceFixtureCase {
    session_id: String,
    run_id: String,
    events: Vec<TraceFixtureEvent>,
    expected: Value,
}

#[derive(Debug, Deserialize)]
struct TraceFixtureEvent {
    #[allow(dead_code)]
    event_type: String,
    #[allow(dead_code)]
    created_at: String,
    payload: Value,
}

pub async fn run_runtime_contract_fixture(
    params: RuntimeContractFixtureParams<'_>,
) -> RuntimeContractOutcome {
    let fixture = load_fixture_case(params.fixture_name);
    let (pool, _tmp) = setup_test_db().await;
    let journal_root = tempfile::tempdir().expect("contract journal tempdir");
    let observability = Arc::new(RuntimeObservability::new(64));
    let journal = SessionJournalStore::with_registry_and_observability(
        journal_root.path().to_path_buf(),
        Arc::new(RunRegistry::default()),
        observability.clone(),
    );

    for seeded in &fixture.events {
        let event: SessionRunEvent = serde_json::from_value(seeded.payload.clone())
            .expect("parse fixture session run event");
        append_session_run_event_with_pool(&pool, &journal, &fixture.session_id, event)
            .await
            .expect("append fixture session run event");
    }

    if params.record_admission_conflict {
        observability.record_admission_conflict(&fixture.session_id);
    }

    if params.record_compaction_run {
        observability.record_compaction_run();
    }

    if let Some(error_kind) = params.record_failover_error_kind {
        observability.record_failover_error_kind(error_kind);
    }

    let trace = export_session_run_trace_with_pool(&pool, &fixture.session_id, &fixture.run_id)
        .await
        .expect("export contract session run trace");
    let normalized_trace = normalize_trace_for_fixture(&trace);
    assert_eq!(
        normalized_trace, fixture.expected,
        "runtime contract fixture {} drifted",
        params.fixture_name
    );

    let session_runs = list_session_runs_with_pool(&pool, &fixture.session_id)
        .await
        .expect("list contract session runs");
    if fixture.events.is_empty() {
        assert!(
            session_runs.is_empty(),
            "empty fixture should not create persisted session runs"
        );
    } else {
        assert!(
            !session_runs.is_empty(),
            "eventful fixture should create a session run projection"
        );
    }

    let observability_snapshot =
        serde_json::to_value(observability.snapshot()).expect("serialize observability snapshot");
    let recent_events = observability.recent_events();

    RuntimeContractOutcome {
        session_runs,
        normalized_trace,
        trace_final_status: trace.final_status,
        trace_child_session_parent: trace
            .child_session_link
            .map(|link| link.parent_session_key),
        observability_snapshot,
        recent_events,
    }
}

fn load_fixture_case(name: &str) -> TraceFixtureCase {
    let path = fixture_path(name);
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("read contract fixture {} failed: {}", path.display(), error)
    });
    serde_json::from_str(&raw).unwrap_or_else(|error| {
        panic!("parse contract fixture {} failed: {}", path.display(), error)
    })
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("run_traces")
        .join(format!("{name}.json"))
}
