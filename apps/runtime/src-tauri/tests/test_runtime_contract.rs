mod helpers;
mod support;

use runtime_lib::agent::runtime::RuntimeObservedEvent;
use support::runtime_contract_testkit::{run_runtime_contract_fixture, RuntimeContractFixtureParams};

#[tokio::test]
async fn runtime_contract_success_fixture_remains_stable() {
    let outcome = run_runtime_contract_fixture(RuntimeContractFixtureParams {
        fixture_name: "success",
        record_admission_conflict: false,
        record_compaction_run: false,
        record_failover_error_kind: None,
    })
    .await;

    assert_eq!(outcome.session_runs.len(), 1);
    assert_eq!(outcome.session_runs[0].status, "completed");
    assert_eq!(outcome.trace_final_status, "completed");
    assert_eq!(outcome.normalized_trace["final_status"], "completed");
}

#[tokio::test]
async fn runtime_contract_admission_conflict_fixture_remains_stable() {
    let outcome = run_runtime_contract_fixture(RuntimeContractFixtureParams {
        fixture_name: "admission_conflict",
        record_admission_conflict: true,
        record_compaction_run: false,
        record_failover_error_kind: None,
    })
    .await;

    assert_eq!(outcome.observability_snapshot["admissions"]["conflicts"], 1);
    assert!(matches!(
        outcome.recent_events.first(),
        Some(RuntimeObservedEvent::AdmissionConflict(_))
    ));
}

#[tokio::test]
async fn runtime_contract_loop_intercepted_fixture_remains_stable() {
    let outcome = run_runtime_contract_fixture(RuntimeContractFixtureParams {
        fixture_name: "loop_intercepted",
        record_admission_conflict: false,
        record_compaction_run: false,
        record_failover_error_kind: None,
    })
    .await;

    assert_eq!(
        outcome.observability_snapshot["guard"]["warnings_by_kind"]["loop_detected"],
        1
    );
}

#[tokio::test]
async fn runtime_contract_approval_resume_fixture_remains_stable() {
    let outcome = run_runtime_contract_fixture(RuntimeContractFixtureParams {
        fixture_name: "approval_resume",
        record_admission_conflict: false,
        record_compaction_run: false,
        record_failover_error_kind: None,
    })
    .await;

    assert_eq!(outcome.observability_snapshot["approvals"]["requested_total"], 1);
}

#[tokio::test]
async fn runtime_contract_compaction_overflow_fixture_remains_stable() {
    let outcome = run_runtime_contract_fixture(RuntimeContractFixtureParams {
        fixture_name: "compaction_overflow",
        record_admission_conflict: false,
        record_compaction_run: true,
        record_failover_error_kind: None,
    })
    .await;

    assert_eq!(outcome.observability_snapshot["compaction"]["runs"], 1);
    assert!(matches!(
        outcome.trace_final_status.as_str(),
        "completed" | "failed" | "stopped"
    ));
}

#[tokio::test]
async fn runtime_contract_failover_recovery_fixture_remains_stable() {
    let outcome = run_runtime_contract_fixture(RuntimeContractFixtureParams {
        fixture_name: "failover_recovery",
        record_admission_conflict: false,
        record_compaction_run: false,
        record_failover_error_kind: Some("network"),
    })
    .await;

    assert_eq!(
        outcome.observability_snapshot["failover"]["errors_by_kind"]["network"],
        1
    );
    assert!(matches!(
        outcome.trace_final_status.as_str(),
        "completed" | "failed" | "stopped"
    ));
}

#[tokio::test]
async fn runtime_contract_approval_reject_fixture_remains_stable() {
    let outcome = run_runtime_contract_fixture(RuntimeContractFixtureParams {
        fixture_name: "approval_reject",
        record_admission_conflict: false,
        record_compaction_run: false,
        record_failover_error_kind: None,
    })
    .await;

    assert_eq!(outcome.observability_snapshot["approvals"]["requested_total"], 1);
    assert_eq!(outcome.trace_final_status, "stopped");
}

#[tokio::test]
async fn runtime_contract_child_session_success_fixture_remains_stable() {
    let outcome = run_runtime_contract_fixture(RuntimeContractFixtureParams {
        fixture_name: "child_session_success",
        record_admission_conflict: false,
        record_compaction_run: false,
        record_failover_error_kind: None,
    })
    .await;

    assert!(outcome.trace_child_session_parent.is_some());
}

#[tokio::test]
async fn runtime_contract_child_session_failure_fixture_remains_stable() {
    let outcome = run_runtime_contract_fixture(RuntimeContractFixtureParams {
        fixture_name: "child_session_failure",
        record_admission_conflict: false,
        record_compaction_run: false,
        record_failover_error_kind: None,
    })
    .await;

    assert!(outcome.trace_child_session_parent.is_some());
    assert!(matches!(outcome.trace_final_status.as_str(), "failed" | "stopped"));
}
