use chrono::Utc;
use reqwest::Client;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::agent::types::LLMResponse;
use crate::commands::models::resolve_default_usable_model_id_with_pool;
use crate::commands::runtime_preferences::get_runtime_preferences_with_pool;

#[derive(Debug, Clone)]
struct TranslationModelConfig {
    api_format: String,
    base_url: String,
    model_name: String,
    api_key: String,
}

impl TranslationModelConfig {
    fn cache_key(&self) -> String {
        format!(
            "model:{}:{}",
            self.api_format.trim().to_ascii_lowercase(),
            self.model_name.trim()
        )
    }
}

fn is_mostly_cjk(text: &str) -> bool {
    let mut total = 0usize;
    let mut cjk = 0usize;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        total += 1;
        if ('\u{4E00}'..='\u{9FFF}').contains(&ch)
            || ('\u{3400}'..='\u{4DBF}').contains(&ch)
            || ('\u{F900}'..='\u{FAFF}').contains(&ch)
        {
            cjk += 1;
        }
    }
    total > 0 && cjk * 100 / total >= 60
}

fn normalize_target_language(raw: &str) -> String {
    let normalized = raw.trim();
    if normalized.is_empty() {
        "zh-CN".to_string()
    } else {
        normalized.to_string()
    }
}

fn should_skip_translation(text: &str, target_lang: &str) -> bool {
    if target_lang.eq_ignore_ascii_case("zh-CN")
        || target_lang.to_ascii_lowercase().starts_with("zh")
    {
        return is_mostly_cjk(text);
    }
    false
}

fn parse_google_translate_text(body: &Value) -> Option<String> {
    let arr = body.as_array()?;
    let segments = arr.first()?.as_array()?;
    let mut out = String::new();
    for seg in segments {
        if let Some(piece) = seg.get(0).and_then(|v| v.as_str()) {
            out.push_str(piece);
        }
    }
    if out.trim().is_empty() {
        None
    } else {
        Some(out)
    }
}

async fn translate_text_via_google(
    client: &Client,
    text: &str,
    target_lang: &str,
) -> Result<String, String> {
    let resp = client
        .get("https://translate.googleapis.com/translate_a/single")
        .query(&[
            ("client", "gtx"),
            ("sl", "auto"),
            ("tl", target_lang),
            ("dt", "t"),
            ("q", text),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("翻译服务返回 HTTP {}", resp.status()));
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    parse_google_translate_text(&body).ok_or_else(|| "翻译结果解析失败".to_string())
}

async fn load_translation_model(
    pool: &SqlitePool,
    preferred_model_id: &str,
) -> Option<TranslationModelConfig> {
    let preferred = if !preferred_model_id.trim().is_empty() {
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT api_format, base_url, model_name, api_key
             FROM model_configs
             WHERE id = ? AND TRIM(api_key) != ''
             LIMIT 1",
        )
        .bind(preferred_model_id.trim())
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let primary = if preferred.is_none() {
        let resolved_id = resolve_default_usable_model_id_with_pool(pool)
            .await
            .ok()
            .flatten();
        if let Some(model_id) = resolved_id {
            sqlx::query_as::<_, (String, String, String, String)>(
                "SELECT api_format, base_url, model_name, api_key
                 FROM model_configs
                 WHERE id = ? AND TRIM(api_key) != ''
                 LIMIT 1",
            )
            .bind(model_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
        } else {
            None
        }
    } else {
        None
    };

    let fallback = if preferred.is_none() && primary.is_none() {
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT api_format, base_url, model_name, api_key
             FROM model_configs
             WHERE api_format NOT LIKE 'search_%' AND TRIM(api_key) != ''
             ORDER BY rowid ASC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    preferred
        .or(primary)
        .or(fallback)
        .map(
            |(api_format, base_url, model_name, api_key)| TranslationModelConfig {
                api_format,
                base_url,
                model_name,
                api_key,
            },
        )
}

async fn translate_text_via_model(
    model: &TranslationModelConfig,
    text: &str,
    target_lang: &str,
) -> Result<String, String> {
    let user_prompt = format!(
        "Translate the following text into {target_lang}. Return translation only, no explanation.\n\n{text}"
    );
    let messages = vec![serde_json::json!({
        "role": "user",
        "content": user_prompt
    })];
    let response = match model.api_format.trim().to_ascii_lowercase().as_str() {
        "anthropic" => crate::adapters::anthropic::chat_stream_with_tools(
            &model.base_url,
            &model.api_key,
            &model.model_name,
            "You are a professional translation assistant.",
            messages,
            vec![],
            |_| {},
        )
        .await
        .map_err(|e| e.to_string())?,
        "openai" => crate::adapters::openai::chat_stream_with_tools(
            &model.base_url,
            &model.api_key,
            &model.model_name,
            "You are a professional translation assistant.",
            messages,
            vec![],
            |_| {},
        )
        .await
        .map_err(|e| e.to_string())?,
        _ => {
            return Err("当前默认模型协议不支持翻译".to_string());
        }
    };

    let translated = match response {
        LLMResponse::Text(v) => v,
        LLMResponse::TextWithToolCalls(v, _) => v,
        LLMResponse::ToolCalls(_) => String::new(),
    };
    let clean = translated.trim().to_string();
    if clean.is_empty() {
        Err("模型翻译结果为空".to_string())
    } else {
        Ok(clean)
    }
}

pub async fn translate_texts_with_preferences_with_pool(
    pool: &SqlitePool,
    texts: Vec<String>,
) -> Result<Vec<String>, String> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    let prefs = get_runtime_preferences_with_pool(pool).await?;
    let target_lang = normalize_target_language(&prefs.default_language);
    if !prefs.immersive_translation_enabled {
        let mut passthrough = Vec::with_capacity(texts.len());
        for source in texts {
            passthrough.push(source.trim().to_string());
        }
        return Ok(passthrough);
    }

    let translation_engine = prefs.translation_engine.trim().to_ascii_lowercase();
    let allow_model = translation_engine != "free_only";
    let allow_free = translation_engine != "model_only";

    let client = Client::new();
    let model_cfg = if allow_model {
        load_translation_model(pool, &prefs.translation_model_id).await
    } else {
        None
    };
    let engine_cache_key = if let Some(cfg) = model_cfg.as_ref() {
        cfg.cache_key()
    } else if allow_free {
        "google-gtx".to_string()
    } else {
        "model-missing".to_string()
    };

    let mut out = Vec::with_capacity(texts.len());
    for source in texts {
        let clean = source.trim().to_string();
        if clean.is_empty() {
            out.push(String::new());
            continue;
        }
        if should_skip_translation(&clean, &target_lang) {
            out.push(clean);
            continue;
        }

        let cache_key = format!(
            "{}:{}:{}",
            target_lang,
            engine_cache_key,
            super::sha256_hex(&clean)
        );
        let cached: Option<(String,)> =
            sqlx::query_as("SELECT translated_text FROM skill_i18n_cache WHERE cache_key = ?")
                .bind(&cache_key)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;
        if let Some((translated,)) = cached {
            out.push(translated);
            continue;
        }

        let translated = if let Some(model) = model_cfg.as_ref() {
            match translate_text_via_model(model, &clean, &target_lang).await {
                Ok(v) if !v.trim().is_empty() => v,
                _ if allow_free => {
                    match translate_text_via_google(&client, &clean, &target_lang).await {
                        Ok(v) if !v.trim().is_empty() => v,
                        _ => clean.clone(),
                    }
                }
                _ => clean.clone(),
            }
        } else if allow_free {
            match translate_text_via_google(&client, &clean, &target_lang).await {
                Ok(v) if !v.trim().is_empty() => v,
                _ => clean.clone(),
            }
        } else {
            clean.clone()
        };
        let now = Utc::now().to_rfc3339();
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO skill_i18n_cache (cache_key, source_text, translated_text, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&cache_key)
        .bind(&clean)
        .bind(&translated)
        .bind(&now)
        .execute(pool)
        .await;
        out.push(translated);
    }

    Ok(out)
}
