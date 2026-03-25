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
