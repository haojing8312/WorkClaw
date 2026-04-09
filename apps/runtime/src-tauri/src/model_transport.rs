use reqwest::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTransportKind {
    AnthropicMessages,
    OpenAiCompletions,
    OpenAiResponses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAiCompatFeatures {
    pub supports_developer_role: bool,
    pub supports_usage_in_streaming: bool,
    pub supports_strict_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModelTransport {
    pub kind: ModelTransportKind,
    pub openai_compat: Option<OpenAiCompatFeatures>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenAiCompatEndpointFamily {
    NativeOpenAi,
    OpenRouter,
    ModelStudioNative,
    MoonshotNative,
    Generic,
}

fn is_anthropic_base_url(base_url: &str) -> bool {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return false;
    }

    let Ok(url) = Url::parse(trimmed) else {
        return false;
    };

    let host = url.host_str().unwrap_or_default();
    if host.eq_ignore_ascii_case("api.anthropic.com") {
        return true;
    }

    url.path()
        .split('/')
        .any(|segment| segment.eq_ignore_ascii_case("anthropic"))
}

fn is_openai_api_base_url(base_url: &str) -> bool {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return false;
    }

    let Ok(url) = Url::parse(trimmed) else {
        return false;
    };

    if detect_openai_compat_endpoint_family(base_url) != OpenAiCompatEndpointFamily::NativeOpenAi {
        return false;
    }

    let path = url.path().trim_end_matches('/');
    path.is_empty() || path.eq_ignore_ascii_case("/v1")
}

fn detect_openai_compat_endpoint_family(base_url: &str) -> OpenAiCompatEndpointFamily {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return OpenAiCompatEndpointFamily::Generic;
    }

    let Ok(url) = Url::parse(trimmed) else {
        return OpenAiCompatEndpointFamily::Generic;
    };

    let host = url.host_str().unwrap_or_default();
    if host.eq_ignore_ascii_case("api.openai.com") {
        OpenAiCompatEndpointFamily::NativeOpenAi
    } else if host.eq_ignore_ascii_case("openrouter.ai") {
        OpenAiCompatEndpointFamily::OpenRouter
    } else if host.eq_ignore_ascii_case("dashscope.aliyuncs.com")
        || host.eq_ignore_ascii_case("dashscope-intl.aliyuncs.com")
    {
        OpenAiCompatEndpointFamily::ModelStudioNative
    } else if host.eq_ignore_ascii_case("api.moonshot.ai") {
        OpenAiCompatEndpointFamily::MoonshotNative
    } else {
        OpenAiCompatEndpointFamily::Generic
    }
}

fn build_openai_compat_features(base_url: &str) -> OpenAiCompatFeatures {
    match detect_openai_compat_endpoint_family(base_url) {
        OpenAiCompatEndpointFamily::NativeOpenAi => OpenAiCompatFeatures {
            supports_developer_role: true,
            supports_usage_in_streaming: true,
            supports_strict_mode: true,
        },
        OpenAiCompatEndpointFamily::ModelStudioNative
        | OpenAiCompatEndpointFamily::MoonshotNative => OpenAiCompatFeatures {
            supports_developer_role: false,
            supports_usage_in_streaming: true,
            supports_strict_mode: false,
        },
        OpenAiCompatEndpointFamily::OpenRouter | OpenAiCompatEndpointFamily::Generic => {
            OpenAiCompatFeatures {
                supports_developer_role: false,
                supports_usage_in_streaming: false,
                supports_strict_mode: false,
            }
        }
    }
}

fn is_openai_provider_key(provider_key: Option<&str>) -> bool {
    provider_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.eq_ignore_ascii_case("openai"))
        .unwrap_or(false)
}

fn has_non_openai_provider_key(provider_key: Option<&str>) -> bool {
    provider_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| !value.eq_ignore_ascii_case("openai"))
        .unwrap_or(false)
}

pub fn resolve_model_transport(
    api_format: &str,
    base_url: &str,
    provider_key: Option<&str>,
) -> ResolvedModelTransport {
    if api_format.trim().eq_ignore_ascii_case("anthropic") || is_anthropic_base_url(base_url) {
        return ResolvedModelTransport {
            kind: ModelTransportKind::AnthropicMessages,
            openai_compat: None,
        };
    }

    let kind = if has_non_openai_provider_key(provider_key) {
        ModelTransportKind::OpenAiCompletions
    } else if is_openai_provider_key(provider_key) && is_openai_api_base_url(base_url) {
        ModelTransportKind::OpenAiResponses
    } else if provider_key.is_none() && is_openai_api_base_url(base_url) {
        ModelTransportKind::OpenAiResponses
    } else {
        ModelTransportKind::OpenAiCompletions
    };

    ResolvedModelTransport {
        kind,
        openai_compat: Some(build_openai_compat_features(base_url)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_openai_uses_responses_transport() {
        let resolved =
            resolve_model_transport("openai", "https://api.openai.com/v1", Some("openai"));

        assert_eq!(resolved.kind, ModelTransportKind::OpenAiResponses);
        assert_eq!(
            resolved.openai_compat,
            Some(OpenAiCompatFeatures {
                supports_developer_role: true,
                supports_usage_in_streaming: true,
                supports_strict_mode: true,
            })
        );
    }

    #[test]
    fn qwen_dashscope_uses_chat_completions_transport() {
        let resolved = resolve_model_transport(
            "openai",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            Some("qwen"),
        );

        assert_eq!(resolved.kind, ModelTransportKind::OpenAiCompletions);
        assert_eq!(
            resolved.openai_compat,
            Some(OpenAiCompatFeatures {
                supports_developer_role: false,
                supports_usage_in_streaming: true,
                supports_strict_mode: false,
            })
        );
    }

    #[test]
    fn native_dashscope_host_enables_streaming_usage_even_without_qwen_provider_key() {
        let resolved = resolve_model_transport(
            "openai",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            None,
        );

        assert_eq!(resolved.kind, ModelTransportKind::OpenAiCompletions);
        assert_eq!(
            resolved.openai_compat,
            Some(OpenAiCompatFeatures {
                supports_developer_role: false,
                supports_usage_in_streaming: true,
                supports_strict_mode: false,
            })
        );
    }

    #[test]
    fn openrouter_host_keeps_proxy_compat_defaults() {
        let resolved =
            resolve_model_transport("openai", "https://openrouter.ai/api/v1", Some("openrouter"));

        assert_eq!(resolved.kind, ModelTransportKind::OpenAiCompletions);
        assert_eq!(
            resolved.openai_compat,
            Some(OpenAiCompatFeatures {
                supports_developer_role: false,
                supports_usage_in_streaming: false,
                supports_strict_mode: false,
            })
        );
    }

    #[test]
    fn moonshot_uses_chat_completions_transport() {
        let resolved =
            resolve_model_transport("openai", "https://api.moonshot.ai/v1", Some("moonshot"));

        assert_eq!(resolved.kind, ModelTransportKind::OpenAiCompletions);
    }

    #[test]
    fn generic_custom_openai_compatible_endpoint_uses_chat_completions_transport() {
        let resolved = resolve_model_transport("openai", "https://llm.example.com/v1", None);

        assert_eq!(resolved.kind, ModelTransportKind::OpenAiCompletions);
        assert_eq!(
            resolved.openai_compat,
            Some(OpenAiCompatFeatures {
                supports_developer_role: false,
                supports_usage_in_streaming: false,
                supports_strict_mode: false,
            })
        );
    }

    #[test]
    fn anthropic_keeps_anthropic_transport() {
        let resolved = resolve_model_transport(
            "anthropic",
            "https://api.anthropic.com/v1",
            Some("anthropic"),
        );

        assert_eq!(resolved.kind, ModelTransportKind::AnthropicMessages);
        assert_eq!(resolved.openai_compat, None);
    }

    #[test]
    fn non_openai_provider_key_prevents_responses_upgrade_even_on_openai_host() {
        let resolved = resolve_model_transport("openai", "https://api.openai.com/v1", Some("qwen"));

        assert_eq!(resolved.kind, ModelTransportKind::OpenAiCompletions);
    }

    #[test]
    fn empty_api_format_with_anthropic_compat_path_uses_anthropic_transport() {
        let resolved =
            resolve_model_transport("", "https://api.minimax.io/anthropic", Some("minimax"));

        assert_eq!(resolved.kind, ModelTransportKind::AnthropicMessages);
        assert_eq!(resolved.openai_compat, None);
    }

    #[test]
    fn empty_api_format_with_native_anthropic_host_uses_anthropic_transport() {
        let resolved = resolve_model_transport("", "https://api.anthropic.com/v1", None);

        assert_eq!(resolved.kind, ModelTransportKind::AnthropicMessages);
        assert_eq!(resolved.openai_compat, None);
    }
}
