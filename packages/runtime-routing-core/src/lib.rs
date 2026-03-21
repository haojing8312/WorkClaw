use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRouteTemplateInfo {
    pub template_id: String,
    pub name: String,
    pub description: String,
    pub capability: String,
}

#[derive(Debug, Clone)]
pub struct TemplateFallbackDef {
    pub provider_keys: &'static [&'static str],
    pub model: &'static str,
}

#[derive(Debug, Clone)]
pub struct CapabilityRouteTemplateDef {
    pub template_id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub capability: &'static str,
    pub primary_provider_keys: &'static [&'static str],
    pub primary_model: &'static str,
    pub fallback: &'static [TemplateFallbackDef],
    pub timeout_ms: i64,
    pub retry_count: i64,
}

pub fn builtin_capability_route_templates() -> Vec<CapabilityRouteTemplateDef> {
    vec![
        CapabilityRouteTemplateDef {
            template_id: "china-first-p0",
            name: "中国优先 P0",
            description: "优先使用 DeepSeek/Qwen/Kimi，兼顾成本与可用性",
            capability: "chat",
            primary_provider_keys: &["deepseek", "qwen", "moonshot"],
            primary_model: "deepseek-chat",
            fallback: &[
                TemplateFallbackDef {
                    provider_keys: &["qwen"],
                    model: "qwen-plus",
                },
                TemplateFallbackDef {
                    provider_keys: &["moonshot"],
                    model: "kimi-k2",
                },
            ],
            timeout_ms: 60000,
            retry_count: 1,
        },
        CapabilityRouteTemplateDef {
            template_id: "china-first-p0",
            name: "中国优先 P0",
            description: "视觉任务优先走国内多模态",
            capability: "vision",
            primary_provider_keys: &["qwen"],
            primary_model: "qwen-vl-max",
            fallback: &[TemplateFallbackDef {
                provider_keys: &["deepseek"],
                model: "deepseek-chat",
            }],
            timeout_ms: 90000,
            retry_count: 1,
        },
        CapabilityRouteTemplateDef {
            template_id: "china-first-p0",
            name: "中国优先 P0",
            description: "生图优先国内，海外作为兜底",
            capability: "image_gen",
            primary_provider_keys: &["qwen"],
            primary_model: "wanx2.1-t2i-plus",
            fallback: &[TemplateFallbackDef {
                provider_keys: &["openai"],
                model: "gpt-image-1",
            }],
            timeout_ms: 120000,
            retry_count: 1,
        },
        CapabilityRouteTemplateDef {
            template_id: "china-first-p0",
            name: "中国优先 P0",
            description: "语音转写优先国内 ASR",
            capability: "audio_stt",
            primary_provider_keys: &["qwen"],
            primary_model: "paraformer-v2",
            fallback: &[TemplateFallbackDef {
                provider_keys: &["openai"],
                model: "gpt-4o-mini-transcribe",
            }],
            timeout_ms: 90000,
            retry_count: 1,
        },
        CapabilityRouteTemplateDef {
            template_id: "china-first-p0",
            name: "中国优先 P0",
            description: "语音合成优先国内 TTS",
            capability: "audio_tts",
            primary_provider_keys: &["qwen"],
            primary_model: "cosyvoice-v1",
            fallback: &[TemplateFallbackDef {
                provider_keys: &["openai"],
                model: "gpt-4o-mini-tts",
            }],
            timeout_ms: 60000,
            retry_count: 1,
        },
    ]
}

pub fn list_capability_route_templates_for(
    capability: Option<&str>,
) -> Vec<CapabilityRouteTemplateInfo> {
    builtin_capability_route_templates()
        .into_iter()
        .filter(|t| capability.map(|c| c == t.capability).unwrap_or(true))
        .map(|t| CapabilityRouteTemplateInfo {
            template_id: t.template_id.to_string(),
            name: t.name.to_string(),
            description: t.description.to_string(),
            capability: t.capability.to_string(),
        })
        .collect()
}

pub fn default_model_for_protocol(protocol_type: &str) -> &'static str {
    if protocol_type == "anthropic" {
        "claude-3-5-haiku-20241022"
    } else {
        "gpt-4o-mini"
    }
}

pub fn recommended_models_for_provider(provider_key: &str) -> Vec<String> {
    match provider_key {
        "doubao" => vec!["doubao-seed-1.6".to_string()],
        "deepseek" => vec!["deepseek-chat".to_string(), "deepseek-reasoner".to_string()],
        "qwen" => vec![
            "qwen-max".to_string(),
            "qwen-plus".to_string(),
            "qwen-vl-max".to_string(),
            "qwen-vl-plus".to_string(),
            "qwen-tts".to_string(),
            "qwen-omni".to_string(),
        ],
        "moonshot" => vec![
            "kimi-k2".to_string(),
            "moonshot-v1-32k".to_string(),
            "moonshot-v1-128k".to_string(),
        ],
        "anthropic" => vec![
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-sonnet-4-5-20250929".to_string(),
        ],
        _ => vec![
            "gpt-4o-mini".to_string(),
            "gpt-4.1-mini".to_string(),
            "gpt-image-1".to_string(),
            "whisper-1".to_string(),
            "tts-1".to_string(),
        ],
    }
}

pub fn filter_models_by_capability(models: Vec<String>, capability: Option<&str>) -> Vec<String> {
    let Some(capability) = capability else {
        return models;
    };
    let original = models.clone();
    let filtered = if capability == "vision" {
        models
            .into_iter()
            .filter(|m| {
                m.contains("vl") || m.contains("4o") || m.contains("claude") || m.contains("omni")
            })
            .collect::<Vec<_>>()
    } else if capability == "reasoning" {
        models
            .into_iter()
            .filter(|m| m.contains("reasoner") || m.contains("k2") || m.contains("sonnet"))
            .collect::<Vec<_>>()
    } else if capability == "image_gen" {
        models
            .into_iter()
            .filter(|m| {
                m.contains("image")
                    || m.contains("vl")
                    || m.contains("omni")
                    || m.contains("cogview")
            })
            .collect::<Vec<_>>()
    } else if capability == "audio_stt" {
        models
            .into_iter()
            .filter(|m| m.contains("whisper") || m.contains("stt") || m.contains("omni"))
            .collect::<Vec<_>>()
    } else if capability == "audio_tts" {
        models
            .into_iter()
            .filter(|m| m.contains("tts") || m.contains("omni"))
            .collect::<Vec<_>>()
    } else {
        models
    };
    if filtered.is_empty() {
        original
    } else {
        filtered
    }
}

pub fn cache_row_is_fresh(fetched_at: &str, ttl_seconds: i64) -> bool {
    let Ok(parsed) = DateTime::parse_from_rfc3339(fetched_at) else {
        return false;
    };
    let age = Utc::now()
        .signed_duration_since(parsed.with_timezone(&Utc))
        .num_seconds();
    age >= 0 && age < ttl_seconds
}
