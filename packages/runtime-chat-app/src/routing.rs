use crate::preparation::{infer_capability_from_message_parts, infer_capability_from_user_message};
use crate::traits::ChatSettingsRepository;
use crate::types::{
    ChatExecutionPreparationRequest, ChatPreparationRequest, ModelRouteErrorKind,
    PreparedRouteCandidate, PreparedRouteCandidates, SessionModelSnapshot,
};
use serde_json::Value;

pub(crate) async fn prepare_route_candidates<R: ChatSettingsRepository>(
    repo: &R,
    model_id: &str,
    request: &ChatPreparationRequest,
) -> Result<PreparedRouteCandidates, String> {
    prepare_route_candidates_with_capability(repo, model_id, request, None).await
}

pub(crate) async fn prepare_route_candidates_with_capability<R: ChatSettingsRepository>(
    repo: &R,
    model_id: &str,
    request: &ChatPreparationRequest,
    requested_capability: Option<&str>,
) -> Result<PreparedRouteCandidates, String> {
    let user_message_parts = request.user_message_parts.as_deref();
    let requested_capability = requested_capability
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            infer_capability_from_message_parts(
                user_message_parts.unwrap_or(&[]),
                &request.user_message,
            )
        });
    let requires_explicit_vision_route =
        requested_capability == "vision" && has_image_message_parts(user_message_parts);
    build_route_candidates(
        repo,
        model_id,
        requested_capability,
        user_message_parts,
        true,
        requested_capability != "chat" && !requires_explicit_vision_route,
        !requires_explicit_vision_route,
    )
    .await
}

pub(crate) async fn prepare_route_decisions<R: ChatSettingsRepository>(
    repo: &R,
    model_id: &str,
    request: &ChatExecutionPreparationRequest,
) -> Result<PreparedRouteCandidates, String> {
    let requested_capability = request
        .requested_capability
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| infer_capability_from_user_message(&request.user_message));
    build_route_candidates(
        repo,
        model_id,
        requested_capability,
        None,
        false,
        true,
        true,
    )
    .await
}

pub fn classify_model_route_error(error_message: &str) -> ModelRouteErrorKind {
    let lower = error_message.to_ascii_lowercase();
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
    ModelRouteErrorKind::Unknown
}

pub fn should_retry_same_candidate(kind: ModelRouteErrorKind) -> bool {
    matches!(
        kind,
        ModelRouteErrorKind::RateLimit
            | ModelRouteErrorKind::Timeout
            | ModelRouteErrorKind::Network
    )
}

pub fn retry_budget_for_error(kind: ModelRouteErrorKind, configured_retry_count: usize) -> usize {
    if kind == ModelRouteErrorKind::Network {
        configured_retry_count.max(5)
    } else {
        configured_retry_count
    }
}

pub fn retry_backoff_ms(kind: ModelRouteErrorKind, attempt_idx: usize) -> u64 {
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

pub fn parse_fallback_chain_targets(raw: &str) -> Vec<(String, String)> {
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

async fn build_route_candidates<R: ChatSettingsRepository>(
    repo: &R,
    model_id: &str,
    requested_capability: &str,
    user_message_parts: Option<&[serde_json::Value]>,
    allow_image_gating: bool,
    allow_chat_fallback: bool,
    allow_session_model_fallback: bool,
) -> Result<PreparedRouteCandidates, String> {
    let session_model = resolve_session_model_with_fallback(repo, model_id).await?;
    let requires_explicit_vision_route = allow_image_gating
        && requested_capability == "vision"
        && has_image_message_parts(user_message_parts);

    let mut retry_count_per_candidate = 0usize;
    let mut route_policy = repo
        .load_route_policy(requested_capability)
        .await?
        .filter(|policy| policy.enabled);
    if route_policy.is_none()
        && requested_capability != "chat"
        && (allow_chat_fallback && !requires_explicit_vision_route)
    {
        route_policy = repo
            .load_route_policy("chat")
            .await?
            .filter(|policy| policy.enabled);
    }

    let mut candidates = Vec::new();
    if let Some(policy) = route_policy {
        retry_count_per_candidate = policy.retry_count.clamp(0, 3) as usize;
        candidates.extend(build_candidates_from_policy(repo, policy, &session_model).await?);
    }

    if allow_session_model_fallback && !session_model.api_key.trim().is_empty() {
        candidates.push(PreparedRouteCandidate {
            provider_key: String::new(),
            protocol_type: session_model.api_format,
            base_url: session_model.base_url,
            model_name: session_model.model_name,
            api_key: session_model.api_key,
        });
    }

    Ok(PreparedRouteCandidates {
        candidates,
        retry_count_per_candidate,
    })
}

async fn build_candidates_from_policy<R: ChatSettingsRepository>(
    repo: &R,
    policy: crate::types::ChatRoutePolicySnapshot,
    session_model: &SessionModelSnapshot,
) -> Result<Vec<PreparedRouteCandidate>, String> {
    let mut candidates = Vec::new();
    let mut provider_targets = vec![(policy.primary_provider_id, policy.primary_model.clone())];
    provider_targets.extend(parse_fallback_chain_targets(&policy.fallback_chain_json));

    for (provider_id, preferred_model) in provider_targets {
        if let Some(provider) = repo.get_provider_connection(&provider_id).await? {
            if is_supported_protocol(&provider.protocol_type) && !provider.api_key.trim().is_empty()
            {
                candidates.push(PreparedRouteCandidate {
                    provider_key: provider.provider_key,
                    protocol_type: provider.protocol_type,
                    base_url: provider.base_url,
                    model_name: if preferred_model.trim().is_empty() {
                        session_model.model_name.clone()
                    } else {
                        preferred_model
                    },
                    api_key: provider.api_key,
                });
            }
        }
    }

    Ok(candidates)
}

async fn resolve_session_model_with_fallback<R: ChatSettingsRepository>(
    repo: &R,
    model_id: &str,
) -> Result<SessionModelSnapshot, String> {
    match repo.load_session_model(model_id).await {
        Ok(model) => Ok(model),
        Err(primary_err) => {
            let normalized_error = primary_err.to_ascii_lowercase();
            let is_missing_model = primary_err.contains("模型配置不存在")
                || normalized_error.contains("no rows returned")
                || normalized_error.contains("rownotfound");
            if !is_missing_model {
                return Err(primary_err);
            }
            let fallback_model_id = repo
                .resolve_default_usable_model_id()
                .await?
                .filter(|fallback_id| fallback_id != model_id)
                .ok_or_else(|| primary_err.clone())?;
            repo.load_session_model(&fallback_model_id)
                .await
                .map_err(|_| primary_err)
        }
    }
}

fn has_image_message_parts(parts: Option<&[Value]>) -> bool {
    parts.unwrap_or(&[]).iter().any(|part| {
        part.get("type")
            .and_then(Value::as_str)
            .map(|part_type| part_type == "image")
            .unwrap_or(false)
    })
}

fn is_supported_protocol(protocol: &str) -> bool {
    matches!(protocol, "openai" | "anthropic")
}
