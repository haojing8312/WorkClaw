use super::types::{ContentCapability, ContentRequest, ProviderStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDecision {
    pub provider_id: String,
    pub capability: ContentCapability,
    pub fallback_provider_id: Option<String>,
}

pub fn route_content_request(
    request: &ContentRequest,
    providers: &[ProviderStatus],
) -> Result<RouteDecision, String> {
    let capability = request
        .capability()
        .ok_or_else(|| "request is not handled by the content provider router".to_string())?;

    let preferred_order: &[&str] = match capability {
        ContentCapability::ReadUrl
        | ContentCapability::SearchContent
        | ContentCapability::ExtractMediaContext => &["agent-reach", "builtin-web"],
    };

    let selected = preferred_order.iter().find_map(|provider_id| {
        providers
            .iter()
            .find(|status| status.provider_id == **provider_id && status.supports(&capability))
    });

    let selected = selected.ok_or_else(|| {
        format!(
            "no content provider available for capability {:?}",
            capability
        )
    })?;

    let fallback_provider_id = preferred_order
        .iter()
        .skip_while(|provider_id| **provider_id != selected.provider_id)
        .skip(1)
        .find_map(|provider_id| {
            providers
                .iter()
                .find(|status| status.provider_id == *provider_id && status.supports(&capability))
                .map(|status| status.provider_id.clone())
        });

    Ok(RouteDecision {
        provider_id: selected.provider_id.clone(),
        capability,
        fallback_provider_id,
    })
}
