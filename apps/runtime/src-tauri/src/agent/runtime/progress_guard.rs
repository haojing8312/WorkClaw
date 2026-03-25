use crate::agent::browser_progress::BrowserProgressSnapshot;
use crate::agent::run_guard::{ProgressEvaluation, ProgressFingerprint, ProgressGuard, RunBudgetPolicy};

pub(crate) fn evaluate_progress_guard(
    policy: &RunBudgetPolicy,
    history: &[ProgressFingerprint],
    latest_browser_progress: Option<&BrowserProgressSnapshot>,
) -> ProgressEvaluation {
    ProgressGuard::evaluate(policy, history)
        .with_last_completed_step(latest_browser_progress.and_then(
            BrowserProgressSnapshot::last_completed_step,
        ))
}

#[cfg(test)]
mod tests {
    use super::evaluate_progress_guard;
    use crate::agent::browser_progress::{BrowserProgressSnapshot, BrowserStageHints};
    use crate::agent::run_guard::{ProgressFingerprint, RunBudgetPolicy, RunBudgetScope};

    #[test]
    fn progress_guard_attaches_last_completed_step_to_warning() {
        let policy = RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat);
        let history = vec![
            ProgressFingerprint::tool_result("browser_snapshot", "input-a", "same-output"),
            ProgressFingerprint::tool_result("browser_snapshot", "input-b", "same-output"),
            ProgressFingerprint::tool_result("browser_snapshot", "input-c", "same-output"),
            ProgressFingerprint::tool_result("browser_snapshot", "input-d", "same-output"),
        ];
        let latest_browser_progress = BrowserProgressSnapshot {
            url: "https://example.com".to_string(),
            title: "draft".to_string(),
            page_signature: "page-1".to_string(),
            facts_signature: "facts-1".to_string(),
            stage_hints: BrowserStageHints {
                cover_filled: false,
                title_filled: false,
                body_segment_count: 1,
            },
        };

        let evaluation = evaluate_progress_guard(&policy, &history, Some(&latest_browser_progress));

        let warning = evaluation.warning.expect("warning");
        assert_eq!(
            warning.last_completed_step.as_deref(),
            Some("已填写正文")
        );
    }
}
