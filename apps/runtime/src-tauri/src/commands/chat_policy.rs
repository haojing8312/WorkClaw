use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{parse_run_stop_reason, RunStopReasonKind};
use crate::model_errors::normalize_model_error;
#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
pub(crate) fn normalize_permission_mode_for_storage(permission_mode: Option<&str>) -> &'static str {
    match permission_mode.unwrap_or("").trim() {
        "standard" | "default" | "accept_edits" => "standard",
        "full_access" | "unrestricted" => "full_access",
        _ => "standard",
    }
}

#[cfg(test)]
pub(crate) fn normalize_session_mode_for_storage(session_mode: Option<&str>) -> &'static str {
    match session_mode.unwrap_or("").trim() {
        "employee_direct" => "employee_direct",
        "team_entry" => "team_entry",
        "general" => "general",
        _ => "general",
    }
}

#[cfg(test)]
pub(crate) fn normalize_team_id_for_storage(session_mode: &str, team_id: Option<&str>) -> String {
    if session_mode == "team_entry" {
        team_id.unwrap_or("").trim().to_string()
    } else {
        String::new()
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_permission_mode_for_runtime(permission_mode: &str) -> PermissionMode {
    match permission_mode {
        "standard" | "default" | "accept_edits" => PermissionMode::AcceptEdits,
        "full_access" | "unrestricted" => PermissionMode::Unrestricted,
        _ => PermissionMode::AcceptEdits,
    }
}

pub(crate) fn permission_mode_label_for_display(permission_mode: &str) -> &'static str {
    match permission_mode {
        "standard" => "标准模式",
        "full_access" => "全自动模式",
        "default" => "标准模式",
        "unrestricted" => "全自动模式",
        _ => "标准模式",
    }
}

#[cfg(test)]
pub(crate) fn is_supported_protocol(protocol: &str) -> bool {
    matches!(protocol, "openai" | "anthropic")
}

#[cfg(test)]
pub(crate) fn infer_capability_from_user_message(message: &str) -> &'static str {
    let m = message.to_ascii_lowercase();
    if m.contains("识图")
        || m.contains("看图")
        || m.contains("图片理解")
        || m.contains("vision")
        || m.contains("analyze image")
    {
        return "vision";
    }
    if m.contains("生图")
        || m.contains("画图")
        || m.contains("生成图片")
        || m.contains("image generation")
        || m.contains("generate image")
    {
        return "image_gen";
    }
    if m.contains("语音转文字")
        || m.contains("语音识别")
        || m.contains("stt")
        || m.contains("transcribe")
        || m.contains("speech to text")
    {
        return "audio_stt";
    }
    if m.contains("文字转语音")
        || m.contains("tts")
        || m.contains("text to speech")
        || m.contains("语音合成")
    {
        return "audio_tts";
    }
    "chat"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum ModelRouteErrorKind {
    Billing,
    Auth,
    RateLimit,
    Timeout,
    Network,
    PolicyBlocked,
    MaxTurns,
    LoopDetected,
    NoProgress,
    Unknown,
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn model_route_error_kind_for_stop_reason_kind(
    kind: RunStopReasonKind,
) -> ModelRouteErrorKind {
    match kind {
        RunStopReasonKind::Timeout => ModelRouteErrorKind::Timeout,
        RunStopReasonKind::PolicyBlocked => ModelRouteErrorKind::PolicyBlocked,
        RunStopReasonKind::MaxTurns | RunStopReasonKind::MaxSessionTurns => {
            ModelRouteErrorKind::MaxTurns
        }
        RunStopReasonKind::LoopDetected | RunStopReasonKind::ToolFailureCircuitBreaker => {
            ModelRouteErrorKind::LoopDetected
        }
        RunStopReasonKind::NoProgress => ModelRouteErrorKind::NoProgress,
        _ => ModelRouteErrorKind::Unknown,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn classify_model_route_error(error_message: &str) -> ModelRouteErrorKind {
    if let Some(reason) = parse_run_stop_reason(error_message) {
        return model_route_error_kind_for_stop_reason_kind(reason.kind);
    }

    let lower = error_message.to_ascii_lowercase();
    if lower.contains("达到最大迭代次数") || lower.contains("最大迭代次数") {
        return ModelRouteErrorKind::MaxTurns;
    }
    if lower.contains("loop_detected") {
        return ModelRouteErrorKind::LoopDetected;
    }
    if lower.contains("no_progress") || lower.contains("没有进展") {
        return ModelRouteErrorKind::NoProgress;
    }
    if lower.contains("insufficient_balance")
        || lower.contains("insufficient balance")
        || lower.contains("balance too low")
        || lower.contains("account balance too low")
        || lower.contains("insufficient_quota")
        || lower.contains("insufficient quota")
        || lower.contains("billing")
        || lower.contains("payment required")
        || lower.contains("credit balance")
        || lower.contains("余额不足")
        || lower.contains("欠费")
    {
        return ModelRouteErrorKind::Billing;
    }
    if lower.contains("api key")
        || lower.contains("unauthorized")
        || lower.contains("invalid_api_key")
        || lower.contains("authentication")
        || lower.contains("permission denied")
        || lower.contains("forbidden")
    {
        return ModelRouteErrorKind::Auth;
    }
    if lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("429")
        || lower.contains("529")
        || lower.contains("overloaded_error")
        || lower.contains("high traffic detected")
        || lower.contains("quota")
    {
        return ModelRouteErrorKind::RateLimit;
    }
    if lower.contains("timeout") || lower.contains("timed out") || lower.contains("deadline") {
        return ModelRouteErrorKind::Timeout;
    }
    if lower.contains("connection")
        || lower.contains("network")
        || lower.contains("dns")
        || lower.contains("connect")
        || lower.contains("socket")
        || lower.contains("error sending request for url")
        || lower.contains("sending request for url")
    {
        return ModelRouteErrorKind::Network;
    }

    match normalize_model_error(error_message).kind {
        crate::model_errors::ModelErrorKind::Billing => ModelRouteErrorKind::Billing,
        crate::model_errors::ModelErrorKind::Auth => ModelRouteErrorKind::Auth,
        crate::model_errors::ModelErrorKind::RateLimit => ModelRouteErrorKind::RateLimit,
        crate::model_errors::ModelErrorKind::Timeout => ModelRouteErrorKind::Timeout,
        crate::model_errors::ModelErrorKind::Network => ModelRouteErrorKind::Network,
        crate::model_errors::ModelErrorKind::Unknown => ModelRouteErrorKind::Unknown,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn should_retry_same_candidate(kind: ModelRouteErrorKind) -> bool {
    matches!(
        kind,
        ModelRouteErrorKind::RateLimit
            | ModelRouteErrorKind::Timeout
            | ModelRouteErrorKind::Network
    )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn retry_budget_for_error(
    kind: ModelRouteErrorKind,
    configured_retry_count: usize,
) -> usize {
    if kind == ModelRouteErrorKind::Network {
        configured_retry_count.max(5)
    } else {
        configured_retry_count
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn retry_backoff_ms(kind: ModelRouteErrorKind, attempt_idx: usize) -> u64 {
    let base_ms = match kind {
        ModelRouteErrorKind::RateLimit => 1200u64,
        ModelRouteErrorKind::Timeout => 700u64,
        ModelRouteErrorKind::Network => 400u64,
        _ => 0u64,
    };
    if base_ms == 0 {
        return 0;
    }
    let exp = attempt_idx.min(3) as u32;
    base_ms.saturating_mul(1u64 << exp).min(5000)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn model_route_error_kind_key(kind: ModelRouteErrorKind) -> &'static str {
    match kind {
        ModelRouteErrorKind::Billing => "billing",
        ModelRouteErrorKind::Auth => "auth",
        ModelRouteErrorKind::RateLimit => "rate_limit",
        ModelRouteErrorKind::Timeout => "timeout",
        ModelRouteErrorKind::Network => "network",
        ModelRouteErrorKind::PolicyBlocked => "policy_blocked",
        ModelRouteErrorKind::MaxTurns => "max_turns",
        ModelRouteErrorKind::LoopDetected => "loop_detected",
        ModelRouteErrorKind::NoProgress => "no_progress",
        ModelRouteErrorKind::Unknown => "unknown",
    }
}

#[cfg(test)]
pub(crate) fn parse_fallback_chain_targets(raw: &str) -> Vec<(String, String)> {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|item| {
            let provider_id = item.get("provider_id")?.as_str()?.to_string();
            let model = item
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            Some((provider_id, model))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::permissions::PermissionMode;

    #[test]
    fn normalize_permission_mode_defaults_to_standard() {
        assert_eq!(normalize_permission_mode_for_storage(None), "standard");
        assert_eq!(normalize_permission_mode_for_storage(Some("")), "standard");
        assert_eq!(
            normalize_permission_mode_for_storage(Some("invalid")),
            "standard"
        );
    }

    #[test]
    fn normalize_permission_mode_maps_legacy_values_to_modern_storage() {
        assert_eq!(
            normalize_permission_mode_for_storage(Some("standard")),
            "standard"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("full_access")),
            "full_access"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("default")),
            "standard"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("accept_edits")),
            "standard"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("unrestricted")),
            "full_access"
        );
    }

    #[test]
    fn normalize_session_mode_defaults_to_general() {
        assert_eq!(normalize_session_mode_for_storage(None), "general");
        assert_eq!(normalize_session_mode_for_storage(Some("")), "general");
        assert_eq!(
            normalize_session_mode_for_storage(Some("invalid")),
            "general"
        );
    }

    #[test]
    fn normalize_session_mode_keeps_supported_values() {
        assert_eq!(
            normalize_session_mode_for_storage(Some("general")),
            "general"
        );
        assert_eq!(
            normalize_session_mode_for_storage(Some("employee_direct")),
            "employee_direct"
        );
        assert_eq!(
            normalize_session_mode_for_storage(Some("team_entry")),
            "team_entry"
        );
    }

    #[test]
    fn normalize_team_id_only_keeps_team_entry_values() {
        assert_eq!(
            normalize_team_id_for_storage("general", Some("group-1")),
            ""
        );
        assert_eq!(
            normalize_team_id_for_storage("employee_direct", Some("group-1")),
            ""
        );
        assert_eq!(
            normalize_team_id_for_storage("team_entry", Some(" group-1 ")),
            "group-1"
        );
    }

    #[test]
    fn parse_permission_mode_for_runtime_defaults_to_standard_behavior() {
        assert_eq!(
            parse_permission_mode_for_runtime(""),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            parse_permission_mode_for_runtime("invalid"),
            PermissionMode::AcceptEdits
        );
    }

    #[test]
    fn parse_permission_mode_for_runtime_supports_modern_and_legacy_values() {
        assert_eq!(
            parse_permission_mode_for_runtime("standard"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            parse_permission_mode_for_runtime("full_access"),
            PermissionMode::Unrestricted
        );
        assert_eq!(
            parse_permission_mode_for_runtime("default"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            parse_permission_mode_for_runtime("accept_edits"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            parse_permission_mode_for_runtime("unrestricted"),
            PermissionMode::Unrestricted
        );
    }

    #[test]
    fn permission_mode_label_is_user_friendly() {
        assert_eq!(permission_mode_label_for_display("standard"), "标准模式");
        assert_eq!(
            permission_mode_label_for_display("full_access"),
            "全自动模式"
        );
        assert_eq!(
            permission_mode_label_for_display("accept_edits"),
            "标准模式"
        );
        assert_eq!(permission_mode_label_for_display("default"), "标准模式");
        assert_eq!(
            permission_mode_label_for_display("unrestricted"),
            "全自动模式"
        );
    }

    #[test]
    fn supported_protocols_are_openai_and_anthropic_only() {
        assert!(is_supported_protocol("openai"));
        assert!(is_supported_protocol("anthropic"));
        assert!(!is_supported_protocol("gemini"));
        assert!(!is_supported_protocol(""));
    }

    #[test]
    fn parse_fallback_chain_targets_handles_json_array() {
        let raw = r#"[{"provider_id":"p1","model":"m1"},{"provider_id":"p2","model":"m2"}]"#;
        let parsed = parse_fallback_chain_targets(raw);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "p1");
        assert_eq!(parsed[0].1, "m1");
        assert_eq!(parsed[1].0, "p2");
        assert_eq!(parsed[1].1, "m2");
    }

    #[test]
    fn classify_model_route_error_detects_auth() {
        let kind = classify_model_route_error("Unauthorized: invalid_api_key");
        assert_eq!(kind, ModelRouteErrorKind::Auth);
        assert!(!should_retry_same_candidate(kind));
    }

    #[test]
    fn classify_model_route_error_detects_billing() {
        let kind = classify_model_route_error("insufficient_balance: account balance too low");
        assert_eq!(kind, ModelRouteErrorKind::Billing);
        assert_eq!(model_route_error_kind_key(kind), "billing");
        assert!(!should_retry_same_candidate(kind));
    }

    #[test]
    fn classify_model_route_error_detects_retryable_kinds() {
        let rate = classify_model_route_error("429 Too Many Requests");
        let overloaded = classify_model_route_error(
            r#"{"type":"error","error":{"type":"overloaded_error","message":"High traffic detected. (2064) (529)"}}"#,
        );
        let timeout = classify_model_route_error("request timeout while calling provider");
        let network = classify_model_route_error("network connection reset");
        assert_eq!(rate, ModelRouteErrorKind::RateLimit);
        assert_eq!(overloaded, ModelRouteErrorKind::RateLimit);
        assert_eq!(timeout, ModelRouteErrorKind::Timeout);
        assert_eq!(network, ModelRouteErrorKind::Network);
        assert!(should_retry_same_candidate(rate));
        assert!(should_retry_same_candidate(overloaded));
        assert!(should_retry_same_candidate(timeout));
        assert!(should_retry_same_candidate(network));
    }

    #[test]
    fn classify_model_route_error_detects_transport_send_failures_as_network() {
        let kind = classify_model_route_error(
            "error sending request for url (https://api.minimax.io/anthropic/v1/messages)",
        );
        assert_eq!(kind, ModelRouteErrorKind::Network);
    }

    #[test]
    fn classify_model_route_error_detects_structured_run_stop_reason() {
        let kind = classify_model_route_error(
            "__WORKCLAW_RUN_STOP__:{\"kind\":\"max_turns\",\"title\":\"任务达到执行步数上限\",\"message\":\"已达到执行步数上限，系统已自动停止。\",\"detail\":\"达到最大迭代次数 100\"}",
        );
        assert_eq!(kind, ModelRouteErrorKind::MaxTurns);
        assert_eq!(model_route_error_kind_key(kind), "max_turns");
        assert!(!should_retry_same_candidate(kind));
    }

    #[test]
    fn classify_model_route_error_detects_policy_blocked_stop_reason() {
        let kind = classify_model_route_error(
            "__WORKCLAW_RUN_STOP__:{\"kind\":\"policy_blocked\",\"title\":\"当前任务无法继续执行\",\"message\":\"本次请求触发了安全或工作区限制，系统已停止继续尝试。\",\"detail\":\"目标路径不在当前工作目录范围内\"}",
        );
        assert_eq!(kind, ModelRouteErrorKind::PolicyBlocked);
        assert_eq!(model_route_error_kind_key(kind), "policy_blocked");
        assert!(!should_retry_same_candidate(kind));
    }

    #[test]
    fn retry_budget_for_error_guarantees_one_retry_for_network() {
        assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Network, 0), 5);
        assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Network, 2), 5);
        assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Network, 7), 7);
        assert_eq!(retry_budget_for_error(ModelRouteErrorKind::RateLimit, 0), 0);
    }

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::Network, 0), 400);
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::Network, 2), 1600);
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::RateLimit, 3), 5000);
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::Unknown, 1), 0);
    }

    #[test]
    fn infer_capability_from_user_message_detects_modalities() {
        assert_eq!(infer_capability_from_user_message("请帮我识图"), "vision");
        assert_eq!(
            infer_capability_from_user_message("帮我生成图片"),
            "image_gen"
        );
        assert_eq!(
            infer_capability_from_user_message("这段音频做语音转文字"),
            "audio_stt"
        );
        assert_eq!(
            infer_capability_from_user_message("这段文案做文字转语音"),
            "audio_tts"
        );
        assert_eq!(infer_capability_from_user_message("解释这个报错"), "chat");
    }
}
