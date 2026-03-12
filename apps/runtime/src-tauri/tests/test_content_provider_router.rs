use runtime_lib::content_providers::{
    route_content_request, ContentCapability, ContentRequest, ProviderAvailability, ProviderStatus,
};

fn provider(
    provider_id: &str,
    availability: ProviderAvailability,
    capabilities: Vec<ContentCapability>,
) -> ProviderStatus {
    ProviderStatus {
        provider_id: provider_id.to_string(),
        availability,
        capabilities,
        detail: None,
    }
}

#[test]
fn read_url_prefers_agent_reach_when_available() {
    let decision = route_content_request(
        &ContentRequest::ReadUrl {
            url: "https://example.com".to_string(),
        },
        &[
            provider(
                "builtin-web",
                ProviderAvailability::Available,
                vec![ContentCapability::ReadUrl],
            ),
            provider(
                "agent-reach",
                ProviderAvailability::Available,
                vec![
                    ContentCapability::ReadUrl,
                    ContentCapability::SearchContent,
                    ContentCapability::ExtractMediaContext,
                ],
            ),
        ],
    )
    .expect("should route");

    assert_eq!(decision.provider_id, "agent-reach");
    assert_eq!(decision.capability, ContentCapability::ReadUrl);
    assert_eq!(
        decision.fallback_provider_id.as_deref(),
        Some("builtin-web")
    );
}

#[test]
fn search_content_falls_back_to_builtin_when_agent_reach_unavailable() {
    let decision = route_content_request(
        &ContentRequest::SearchContent {
            query: "agent".to_string(),
            platform: None,
        },
        &[
            provider(
                "builtin-web",
                ProviderAvailability::Available,
                vec![ContentCapability::SearchContent],
            ),
            provider(
                "agent-reach",
                ProviderAvailability::NotFound,
                vec![ContentCapability::SearchContent],
            ),
        ],
    )
    .expect("should route");

    assert_eq!(decision.provider_id, "builtin-web");
    assert_eq!(decision.capability, ContentCapability::SearchContent);
    assert!(decision.fallback_provider_id.is_none());
}

#[test]
fn interaction_requests_are_not_handled_by_content_router() {
    let result = route_content_request(
        &ContentRequest::BrowserInteract {
            action: "click".to_string(),
        },
        &[provider(
            "builtin-web",
            ProviderAvailability::Available,
            vec![ContentCapability::ReadUrl],
        )],
    );

    let error = result.expect_err("browser interaction should be unsupported");
    assert!(
        error.contains("content provider"),
        "unexpected error: {error}"
    );
}
