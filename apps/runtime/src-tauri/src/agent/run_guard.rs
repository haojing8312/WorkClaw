use serde::{Deserialize, Serialize};

const RUN_STOP_REASON_PREFIX: &str = "__WORKCLAW_RUN_STOP__:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunBudgetScope {
    GeneralChat,
    Skill,
    Employee,
    SubAgent,
    BrowserHeavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunBudgetPolicy {
    pub max_turns: usize,
    pub repeated_tool_call_limit: usize,
    pub no_progress_limit: usize,
}

impl RunBudgetPolicy {
    pub fn for_scope(scope: RunBudgetScope) -> Self {
        match scope {
            RunBudgetScope::GeneralChat => Self {
                max_turns: 100,
                repeated_tool_call_limit: 6,
                no_progress_limit: 5,
            },
            RunBudgetScope::Skill => Self {
                max_turns: 100,
                repeated_tool_call_limit: 6,
                no_progress_limit: 5,
            },
            RunBudgetScope::Employee => Self {
                max_turns: 100,
                repeated_tool_call_limit: 6,
                no_progress_limit: 5,
            },
            RunBudgetScope::SubAgent => Self {
                max_turns: 100,
                repeated_tool_call_limit: 5,
                no_progress_limit: 4,
            },
            RunBudgetScope::BrowserHeavy => Self {
                max_turns: 100,
                repeated_tool_call_limit: 8,
                no_progress_limit: 6,
            },
        }
    }

    pub fn resolve(scope: RunBudgetScope, override_max_turns: Option<usize>) -> Self {
        let mut default_policy = Self::for_scope(scope);
        match override_max_turns {
            Some(max_turns) => {
                default_policy.max_turns = max_turns.max(1);
                default_policy
            }
            None => default_policy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStopReasonKind {
    GoalReached,
    Cancelled,
    MaxTurns,
    MaxSessionTurns,
    Timeout,
    LoopDetected,
    NoProgress,
    ToolFailureCircuitBreaker,
    ProtocolViolation,
}

impl RunStopReasonKind {
    pub fn as_key(self) -> &'static str {
        match self {
            RunStopReasonKind::GoalReached => "goal_reached",
            RunStopReasonKind::Cancelled => "cancelled",
            RunStopReasonKind::MaxTurns => "max_turns",
            RunStopReasonKind::MaxSessionTurns => "max_session_turns",
            RunStopReasonKind::Timeout => "timeout",
            RunStopReasonKind::LoopDetected => "loop_detected",
            RunStopReasonKind::NoProgress => "no_progress",
            RunStopReasonKind::ToolFailureCircuitBreaker => "tool_failure_circuit_breaker",
            RunStopReasonKind::ProtocolViolation => "protocol_violation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStopReason {
    pub kind: RunStopReasonKind,
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_completed_step: Option<String>,
}

impl RunStopReason {
    pub fn new(
        kind: RunStopReasonKind,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            message: message.into(),
            detail: None,
            last_completed_step: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_last_completed_step(mut self, step: impl Into<String>) -> Self {
        self.last_completed_step = Some(step.into());
        self
    }

    pub fn max_turns(max_turns: usize) -> Self {
        Self::new(
            RunStopReasonKind::MaxTurns,
            "任务达到执行步数上限",
            "已达到执行步数上限，系统已自动停止。",
        )
        .with_detail(format!("达到最大迭代次数 {}", max_turns))
    }

    pub fn loop_detected(detail: impl Into<String>) -> Self {
        Self::new(
            RunStopReasonKind::LoopDetected,
            "任务疑似卡住，已自动停止",
            "系统检测到连续重复步骤，已自动停止本轮任务。",
        )
        .with_detail(detail)
    }

    pub fn no_progress(detail: impl Into<String>) -> Self {
        Self::new(
            RunStopReasonKind::NoProgress,
            "任务长时间没有进展",
            "系统检测到连续多轮没有有效进展，已自动停止本轮任务。",
        )
        .with_detail(detail)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunGuardWarning {
    pub kind: RunStopReasonKind,
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_completed_step: Option<String>,
}

impl RunGuardWarning {
    pub fn new(
        kind: RunStopReasonKind,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            message: message.into(),
            detail: None,
            last_completed_step: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_last_completed_step(mut self, step: impl Into<String>) -> Self {
        self.last_completed_step = Some(step.into());
        self
    }

    pub fn loop_detected(detail: impl Into<String>) -> Self {
        Self::new(
            RunStopReasonKind::LoopDetected,
            "任务可能即将卡住",
            "系统检测到连续重复步骤，若继续无变化将自动停止。",
        )
        .with_detail(detail)
    }

    pub fn no_progress(detail: impl Into<String>) -> Self {
        Self::new(
            RunStopReasonKind::NoProgress,
            "任务进展缓慢",
            "系统检测到多轮没有明显进展，若继续无变化将自动停止。",
        )
        .with_detail(detail)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressFingerprint {
    pub tool_name: String,
    pub input_signature: Option<String>,
    pub output_signature: Option<String>,
}

impl ProgressFingerprint {
    pub fn tool(tool_name: impl Into<String>, input_signature: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            input_signature: Some(input_signature.into()),
            output_signature: None,
        }
    }

    pub fn tool_result(
        tool_name: impl Into<String>,
        input_signature: impl Into<String>,
        output_signature: impl Into<String>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            input_signature: Some(input_signature.into()),
            output_signature: Some(output_signature.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressEvaluation {
    pub warning: Option<RunGuardWarning>,
    pub stop_reason: Option<RunStopReason>,
}

impl ProgressEvaluation {
    fn continue_running() -> Self {
        Self {
            warning: None,
            stop_reason: None,
        }
    }
}

pub struct ProgressGuard;

impl ProgressGuard {
    pub fn evaluate(
        policy: &RunBudgetPolicy,
        history: &[ProgressFingerprint],
    ) -> ProgressEvaluation {
        if let Some((tool_name, streak)) =
            Self::repeated_identical_tool_call_warning(policy, history)
        {
            return ProgressEvaluation {
                warning: Some(RunGuardWarning::loop_detected(format!(
                    "工具 {tool_name} 已连续 {streak} 次使用相同输入执行。"
                ))),
                stop_reason: None,
            };
        }

        if let Some((tool_name, streak)) = Self::repeated_identical_tool_calls(policy, history) {
            return ProgressEvaluation {
                warning: None,
                stop_reason: Some(RunStopReason::loop_detected(format!(
                    "工具 {tool_name} 已连续 {streak} 次使用相同输入执行。"
                ))),
            };
        }

        if let Some((tool_name, streak)) = Self::repeated_identical_output_warning(policy, history)
        {
            return ProgressEvaluation {
                warning: Some(RunGuardWarning::no_progress(format!(
                    "工具 {tool_name} 已连续 {streak} 次返回相同结果。"
                ))),
                stop_reason: None,
            };
        }

        if let Some((tool_name, streak)) = Self::repeated_identical_outputs(policy, history) {
            return ProgressEvaluation {
                warning: None,
                stop_reason: Some(RunStopReason::no_progress(format!(
                    "工具 {tool_name} 已连续 {streak} 次返回相同结果。"
                ))),
            };
        }

        ProgressEvaluation::continue_running()
    }

    fn repeated_identical_tool_call_warning(
        policy: &RunBudgetPolicy,
        history: &[ProgressFingerprint],
    ) -> Option<(String, usize)> {
        let threshold = policy.repeated_tool_call_limit.checked_sub(1)?;
        if threshold == 0 {
            return None;
        }
        let last = history.last()?;
        let last_input = last.input_signature.as_ref()?;
        let streak = history
            .iter()
            .rev()
            .take_while(|item| {
                item.tool_name == last.tool_name
                    && item.input_signature.as_ref() == Some(last_input)
            })
            .count();
        if streak == threshold {
            Some((last.tool_name.clone(), streak))
        } else {
            None
        }
    }

    fn repeated_identical_tool_calls(
        policy: &RunBudgetPolicy,
        history: &[ProgressFingerprint],
    ) -> Option<(String, usize)> {
        let last = history.last()?;
        let last_input = last.input_signature.as_ref()?;
        let streak = history
            .iter()
            .rev()
            .take_while(|item| {
                item.tool_name == last.tool_name
                    && item.input_signature.as_ref() == Some(last_input)
            })
            .count();
        if streak >= policy.repeated_tool_call_limit {
            Some((last.tool_name.clone(), streak))
        } else {
            None
        }
    }

    fn repeated_identical_output_warning(
        policy: &RunBudgetPolicy,
        history: &[ProgressFingerprint],
    ) -> Option<(String, usize)> {
        let threshold = policy.no_progress_limit.checked_sub(1)?;
        if threshold == 0 {
            return None;
        }
        let last = history.last()?;
        let last_output = last.output_signature.as_ref()?;
        let streak = history
            .iter()
            .rev()
            .take_while(|item| {
                item.tool_name == last.tool_name
                    && item.output_signature.as_ref() == Some(last_output)
            })
            .count();
        if streak == threshold {
            Some((last.tool_name.clone(), streak))
        } else {
            None
        }
    }

    fn repeated_identical_outputs(
        policy: &RunBudgetPolicy,
        history: &[ProgressFingerprint],
    ) -> Option<(String, usize)> {
        let last = history.last()?;
        let last_output = last.output_signature.as_ref()?;
        let streak = history
            .iter()
            .rev()
            .take_while(|item| {
                item.tool_name == last.tool_name
                    && item.output_signature.as_ref() == Some(last_output)
            })
            .count();
        if streak >= policy.no_progress_limit {
            Some((last.tool_name.clone(), streak))
        } else {
            None
        }
    }
}

pub fn encode_run_stop_reason(reason: &RunStopReason) -> String {
    match serde_json::to_string(reason) {
        Ok(payload) => format!("{RUN_STOP_REASON_PREFIX}{payload}"),
        Err(_) => reason
            .detail
            .clone()
            .unwrap_or_else(|| reason.message.clone()),
    }
}

pub fn parse_run_stop_reason(raw: &str) -> Option<RunStopReason> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(payload) = trimmed.strip_prefix(RUN_STOP_REASON_PREFIX) {
        return serde_json::from_str(payload).ok();
    }

    if trimmed.contains("达到最大迭代次数") || trimmed.contains("最大迭代次数") {
        let max_turns = extract_first_number(trimmed).unwrap_or(0);
        return Some(RunStopReason::max_turns(max_turns.max(1)).with_detail(trimmed.to_string()));
    }

    None
}

fn extract_first_number(raw: &str) -> Option<usize> {
    let digits = raw
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<usize>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_budget_policy_defaults_general_chat_to_100_turns() {
        let policy = RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat);
        assert_eq!(policy.max_turns, 100);
    }

    #[test]
    fn run_budget_policy_defaults_skill_to_100_turns() {
        let policy = RunBudgetPolicy::for_scope(RunBudgetScope::Skill);
        assert_eq!(policy.max_turns, 100);
    }

    #[test]
    fn run_stop_reason_kind_serializes_to_snake_case() {
        let value = serde_json::to_string(&RunStopReasonKind::MaxTurns).unwrap();
        assert_eq!(value, "\"max_turns\"");
    }

    #[test]
    fn run_stop_reason_round_trips_through_encoded_payload() {
        let reason = RunStopReason::max_turns(12);
        let encoded = encode_run_stop_reason(&reason);
        let decoded = parse_run_stop_reason(&encoded).expect("run stop reason should decode");

        assert_eq!(decoded.kind, RunStopReasonKind::MaxTurns);
        assert_eq!(decoded.title, "任务达到执行步数上限");
        assert_eq!(decoded.message, "已达到执行步数上限，系统已自动停止。");
        assert_eq!(decoded.detail.as_deref(), Some("达到最大迭代次数 100"));
    }

    #[test]
    fn legacy_max_turn_error_text_can_be_upgraded_to_structured_stop_reason() {
        let decoded = parse_run_stop_reason("执行异常：达到最大迭代次数 24")
            .expect("legacy error should parse");

        assert_eq!(decoded.kind, RunStopReasonKind::MaxTurns);
        assert_eq!(decoded.title, "任务达到执行步数上限");
        assert_eq!(decoded.message, "已达到执行步数上限，系统已自动停止。");
        assert_eq!(
            decoded.detail.as_deref(),
            Some("执行异常：达到最大迭代次数 24")
        );
    }

    #[test]
    fn repeated_identical_tool_calls_trigger_loop_detected_stop() {
        let policy = RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat);
        let history = vec![
            ProgressFingerprint::tool("browser_click", "same-input"),
            ProgressFingerprint::tool("browser_click", "same-input"),
            ProgressFingerprint::tool("browser_click", "same-input"),
            ProgressFingerprint::tool("browser_click", "same-input"),
            ProgressFingerprint::tool("browser_click", "same-input"),
            ProgressFingerprint::tool("browser_click", "same-input"),
        ];

        let evaluation = ProgressGuard::evaluate(&policy, &history);

        assert_eq!(
            evaluation.stop_reason.unwrap().kind,
            RunStopReasonKind::LoopDetected
        );
    }

    #[test]
    fn repeated_identical_tool_calls_emit_warning_before_loop_stop() {
        let policy = RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat);
        let history = vec![
            ProgressFingerprint::tool("browser_click", "same-input"),
            ProgressFingerprint::tool("browser_click", "same-input"),
            ProgressFingerprint::tool("browser_click", "same-input"),
            ProgressFingerprint::tool("browser_click", "same-input"),
            ProgressFingerprint::tool("browser_click", "same-input"),
        ];

        let evaluation = ProgressGuard::evaluate(&policy, &history);

        assert!(evaluation.stop_reason.is_none());
        assert_eq!(
            evaluation.warning.unwrap().kind,
            RunStopReasonKind::LoopDetected
        );
    }

    #[test]
    fn repeated_identical_tool_outputs_trigger_no_progress_stop() {
        let policy = RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat);
        let history = vec![
            ProgressFingerprint::tool_result("browser_snapshot", "hash-a", "same-page"),
            ProgressFingerprint::tool_result("browser_snapshot", "hash-b", "same-page"),
            ProgressFingerprint::tool_result("browser_snapshot", "hash-c", "same-page"),
            ProgressFingerprint::tool_result("browser_snapshot", "hash-d", "same-page"),
            ProgressFingerprint::tool_result("browser_snapshot", "hash-e", "same-page"),
        ];

        let evaluation = ProgressGuard::evaluate(&policy, &history);

        assert_eq!(
            evaluation.stop_reason.unwrap().kind,
            RunStopReasonKind::NoProgress
        );
    }

    #[test]
    fn repeated_identical_tool_outputs_emit_warning_before_no_progress_stop() {
        let policy = RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat);
        let history = vec![
            ProgressFingerprint::tool_result("browser_snapshot", "hash-a", "same-page"),
            ProgressFingerprint::tool_result("browser_snapshot", "hash-b", "same-page"),
            ProgressFingerprint::tool_result("browser_snapshot", "hash-c", "same-page"),
            ProgressFingerprint::tool_result("browser_snapshot", "hash-d", "same-page"),
        ];

        let evaluation = ProgressGuard::evaluate(&policy, &history);

        assert!(evaluation.stop_reason.is_none());
        assert_eq!(
            evaluation.warning.unwrap().kind,
            RunStopReasonKind::NoProgress
        );
    }
}
