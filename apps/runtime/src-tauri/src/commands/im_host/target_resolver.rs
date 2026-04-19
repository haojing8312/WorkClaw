#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImDirectRouteTargetOptions<'a> {
    pub direct_sender_prefix: &'a str,
    pub reply_to_param_key: &'a str,
    pub thread_id_param_key: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImDispatchTargetResolution {
    pub thread_id: String,
    pub used_explicit_chat_id: bool,
}

pub(crate) fn build_direct_reply_route_target(
    thread_id: &str,
    reply_to_message_id: Option<&str>,
    options: ImDirectRouteTargetOptions<'_>,
) -> String {
    let normalized_thread_id = thread_id.trim();
    let normalized_reply_to = reply_to_message_id
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if normalized_thread_id.is_empty() {
        return String::new();
    }

    if normalized_thread_id.starts_with(options.direct_sender_prefix) {
        if let Some(reply_to_message_id) = normalized_reply_to {
            return format!(
                "{thread}#{reply_key}={reply}&{thread_key}={thread}",
                thread = normalized_thread_id,
                reply_key = options.reply_to_param_key,
                reply = reply_to_message_id,
                thread_key = options.thread_id_param_key,
            );
        }
    }

    normalized_thread_id.to_string()
}

pub(crate) fn resolve_dispatch_thread_target(
    raw_thread_id: &str,
    explicit_chat_id: Option<&str>,
    chat_type: Option<&str>,
    direct_sender_prefix: &str,
    mapped_chat_id: Option<&str>,
) -> Result<ImDispatchTargetResolution, String> {
    let normalized_thread_id = raw_thread_id.trim();
    if normalized_thread_id.is_empty() {
        return Err("dispatch_request missing threadId".to_string());
    }

    let normalized_explicit_chat_id = explicit_chat_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_mapped_chat_id = mapped_chat_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_chat_type = chat_type.map(str::trim).filter(|value| !value.is_empty());
    let is_direct = matches!(normalized_chat_type, Some("direct") | None);

    if is_direct {
        if let Some(chat_id) = normalized_explicit_chat_id {
            return Ok(ImDispatchTargetResolution {
                thread_id: chat_id.to_string(),
                used_explicit_chat_id: true,
            });
        }
    }

    if !is_direct || !normalized_thread_id.starts_with(direct_sender_prefix) {
        return Ok(ImDispatchTargetResolution {
            thread_id: normalized_thread_id.to_string(),
            used_explicit_chat_id: false,
        });
    }

    Ok(ImDispatchTargetResolution {
        thread_id: normalized_mapped_chat_id
            .unwrap_or(normalized_thread_id)
            .to_string(),
        used_explicit_chat_id: false,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_direct_reply_route_target, resolve_dispatch_thread_target, ImDirectRouteTargetOptions,
    };

    #[test]
    fn direct_reply_route_target_embeds_reply_to_when_sender_id_matches_prefix() {
        let target = build_direct_reply_route_target(
            "ou_sender_1",
            Some("om_123"),
            ImDirectRouteTargetOptions {
                direct_sender_prefix: "ou_",
                reply_to_param_key: "__feishu_reply_to",
                thread_id_param_key: "__feishu_thread_id",
            },
        );

        assert_eq!(
            target,
            "ou_sender_1#__feishu_reply_to=om_123&__feishu_thread_id=ou_sender_1"
        );
    }

    #[test]
    fn resolve_dispatch_target_prefers_explicit_chat_id_for_direct_events() {
        let resolution = resolve_dispatch_thread_target(
            "ou_sender_1",
            Some("oc_chat_1"),
            Some("direct"),
            "ou_",
            Some("oc_chat_fallback"),
        )
        .expect("resolve dispatch target");

        assert_eq!(resolution.thread_id, "oc_chat_1");
        assert!(resolution.used_explicit_chat_id);
    }

    #[test]
    fn resolve_dispatch_target_falls_back_to_mapped_chat_id_for_direct_sender_ids() {
        let resolution = resolve_dispatch_thread_target(
            "ou_sender_1",
            None,
            Some("direct"),
            "ou_",
            Some("oc_chat_1"),
        )
        .expect("resolve dispatch target");

        assert_eq!(resolution.thread_id, "oc_chat_1");
        assert!(!resolution.used_explicit_chat_id);
    }

    #[test]
    fn resolve_dispatch_target_keeps_group_thread_id() {
        let resolution = resolve_dispatch_thread_target(
            "oc_group_1",
            Some("oc_group_explicit"),
            Some("group"),
            "ou_",
            Some("oc_group_mapped"),
        )
        .expect("resolve dispatch target");

        assert_eq!(resolution.thread_id, "oc_group_1");
        assert!(!resolution.used_explicit_chat_id);
    }
}
