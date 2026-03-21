use runtime_routing_core::{
    cache_row_is_fresh, default_model_for_protocol, filter_models_by_capability,
    list_capability_route_templates_for, recommended_models_for_provider,
};

#[test]
fn route_templates_can_be_listed_for_chat() {
    let templates = list_capability_route_templates_for(Some("chat"));
    assert!(!templates.is_empty());
    assert!(templates
        .iter()
        .any(|t| t.template_id == "china-first-p0" && t.capability == "chat"));
}

#[test]
fn default_model_depends_on_protocol() {
    assert_eq!(
        default_model_for_protocol("anthropic"),
        "claude-3-5-haiku-20241022"
    );
    assert_eq!(default_model_for_protocol("openai"), "gpt-4o-mini");
}

#[test]
fn recommended_models_cover_known_providers() {
    let qwen = recommended_models_for_provider("qwen");
    assert!(qwen.iter().any(|m| m == "qwen-max"));

    let deepseek = recommended_models_for_provider("deepseek");
    assert!(deepseek.iter().any(|m| m == "deepseek-chat"));

    let doubao = recommended_models_for_provider("doubao");
    assert!(doubao.iter().any(|m| m == "doubao-seed-1.6"));
}

#[test]
fn capability_filter_prefers_matching_models() {
    let models = vec![
        "qwen-max".to_string(),
        "qwen-vl-max".to_string(),
        "gpt-4o-mini".to_string(),
    ];
    let filtered = filter_models_by_capability(models, Some("vision"));
    assert!(filtered
        .iter()
        .any(|m| m.contains("vl") || m.contains("4o")));
}

#[test]
fn cache_freshness_accepts_recent_rows() {
    let ts = chrono::Utc::now().to_rfc3339();
    assert!(cache_row_is_fresh(&ts, 3600));
}

#[test]
fn cache_freshness_rejects_invalid_rows() {
    assert!(!cache_row_is_fresh("not-a-date", 3600));
}
