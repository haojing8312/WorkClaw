use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelErrorKind {
    Billing,
    Auth,
    RateLimit,
    Timeout,
    Network,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct NormalizedModelError {
    pub kind: ModelErrorKind,
    pub raw_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelConnectionTestResult {
    pub ok: bool,
    pub kind: ModelErrorKind,
    pub title: String,
    pub message: String,
    pub raw_message: Option<String>,
}

pub(crate) fn normalize_model_error(raw_message: &str) -> NormalizedModelError {
    let lower = normalized_error_search_text(raw_message);
    let kind = if lower.contains("insufficient_balance")
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
        ModelErrorKind::Billing
    } else if lower.contains("api key")
        || lower.contains("unauthorized")
        || lower.contains("invalid_api_key")
        || lower.contains("authentication")
        || lower.contains("permission denied")
        || lower.contains("forbidden")
    {
        ModelErrorKind::Auth
    } else if lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("429")
        || lower.contains("quota")
    {
        ModelErrorKind::RateLimit
    } else if lower.contains("timeout") || lower.contains("timed out") || lower.contains("deadline")
    {
        ModelErrorKind::Timeout
    } else if is_retryable_minimax_gateway_error(&lower) {
        ModelErrorKind::Network
    } else if lower.contains("connection")
        || lower.contains("network")
        || lower.contains("dns")
        || lower.contains("connect")
        || lower.contains("socket")
        || lower.contains("decoding response body")
        || lower.contains("decode response body")
        || lower.contains("error decoding response body")
        || lower.contains("error sending request for url")
        || lower.contains("sending request for url")
    {
        ModelErrorKind::Network
    } else {
        ModelErrorKind::Unknown
    };

    NormalizedModelError {
        kind,
        raw_message: raw_message.to_string(),
    }
}

pub(crate) fn model_error_title(kind: ModelErrorKind) -> &'static str {
    match kind {
        ModelErrorKind::Billing => "模型余额不足",
        ModelErrorKind::Auth => "鉴权失败",
        ModelErrorKind::RateLimit => "请求过于频繁",
        ModelErrorKind::Timeout => "请求超时",
        ModelErrorKind::Network => "网络连接失败",
        ModelErrorKind::Unknown => "连接失败",
    }
}

pub(crate) fn model_error_message(kind: ModelErrorKind) -> &'static str {
    match kind {
        ModelErrorKind::Billing => {
            "当前模型平台返回余额或额度不足，请到对应服务商控制台充值或检查套餐额度。"
        }
        ModelErrorKind::Auth => "请检查 API Key、组织权限或接口访问范围是否正确。",
        ModelErrorKind::RateLimit => "模型平台当前触发限流，请稍后重试或降低并发频率。",
        ModelErrorKind::Timeout => "模型平台响应超时，请稍后重试，或检查网络和所选模型是否可用。",
        ModelErrorKind::Network => "无法连接到模型接口，请检查 Base URL、网络环境或代理配置。",
        ModelErrorKind::Unknown => "模型平台返回了未识别错误，可查看详细信息进一步排查。",
    }
}

pub(crate) fn build_failed_connection_test_result(raw_message: &str) -> ModelConnectionTestResult {
    let normalized = normalize_model_error(raw_message);
    ModelConnectionTestResult {
        ok: false,
        kind: normalized.kind,
        title: model_error_title(normalized.kind).to_string(),
        message: model_error_message(normalized.kind).to_string(),
        raw_message: Some(normalized.raw_message),
    }
}

pub(crate) fn build_success_connection_test_result() -> ModelConnectionTestResult {
    ModelConnectionTestResult {
        ok: true,
        kind: ModelErrorKind::Unknown,
        title: "连接成功".to_string(),
        message: "模型连接测试成功。".to_string(),
        raw_message: None,
    }
}

fn normalized_error_search_text(raw_message: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<Value>(raw_message) {
        let mut parts = Vec::new();
        collect_error_strings(&parsed, &mut parts);
        if !parts.is_empty() {
            return parts.join(" ").to_ascii_lowercase();
        }
    }
    raw_message.to_ascii_lowercase()
}

fn is_retryable_minimax_gateway_error(lower: &str) -> bool {
    lower.contains("unknown error, 794") && lower.contains("(1000)")
}

fn collect_error_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_error_strings(item, out);
            }
        }
        Value::Object(map) => {
            for key in ["message", "code", "type", "error", "detail"] {
                if let Some(value) = map.get(key) {
                    collect_error_strings(value, out);
                }
            }
            for value in map.values() {
                collect_error_strings(value, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_model_error_detects_billing_from_balance_text() {
        let result = normalize_model_error("insufficient_balance: account balance too low");
        assert_eq!(result.kind, ModelErrorKind::Billing);
    }

    #[test]
    fn normalize_model_error_detects_auth_from_invalid_key_text() {
        let result = normalize_model_error("Unauthorized: invalid_api_key");
        assert_eq!(result.kind, ModelErrorKind::Auth);
    }

    #[test]
    fn normalize_model_error_extracts_openai_json_error_message() {
        let raw = r#"{"error":{"message":"insufficient_quota","code":"insufficient_quota"}}"#;
        let result = normalize_model_error(raw);
        assert_eq!(result.kind, ModelErrorKind::Billing);
    }

    #[test]
    fn normalize_model_error_handles_plain_text_gateway_errors() {
        let raw = "error sending request for url (https://provider.example/v1/chat/completions)";
        let result = normalize_model_error(raw);
        assert_eq!(result.kind, ModelErrorKind::Network);
    }

    #[test]
    fn normalize_model_error_treats_response_body_decode_failures_as_network_errors() {
        let result = normalize_model_error("error decoding response body");
        assert_eq!(result.kind, ModelErrorKind::Network);
    }

    #[test]
    fn normalize_model_error_treats_minimax_794_gateway_failures_as_network_errors() {
        let raw = r#"{"type":"error","error":{"type":"api_error","message":"unknown error, 794 (1000)"},"request_id":"0619614fa6873d3861ed0c9dfe062551"}"#;
        let result = normalize_model_error(raw);
        assert_eq!(result.kind, ModelErrorKind::Network);
    }

    #[test]
    fn connection_test_failure_maps_billing_to_shared_copy() {
        let result =
            build_failed_connection_test_result("insufficient_balance: account balance too low");

        assert!(!result.ok);
        assert_eq!(result.kind, ModelErrorKind::Billing);
        assert_eq!(result.title, "模型余额不足");
        assert_eq!(
            result.message,
            "当前模型平台返回余额或额度不足，请到对应服务商控制台充值或检查套餐额度。"
        );
    }
}
