#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteTarget {
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct RoutingPolicy {
    pub capability: String,
    pub primary: RouteTarget,
    pub fallbacks: Vec<RouteTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteFailureKind {
    Auth,
    RateLimit,
    Timeout,
    Network,
    Unknown,
}

impl RoutingPolicy {
    pub fn ordered_targets(&self) -> Vec<RouteTarget> {
        let mut targets = Vec::with_capacity(1 + self.fallbacks.len());
        targets.push(self.primary.clone());
        targets.extend(self.fallbacks.clone());
        targets
    }
}

pub fn route_with_fallback(
    policy: &RoutingPolicy,
    failure_kind: Option<RouteFailureKind>,
) -> Option<RouteTarget> {
    let ordered = policy.ordered_targets();
    match failure_kind {
        None => ordered.first().cloned(),
        Some(RouteFailureKind::Auth)
        | Some(RouteFailureKind::RateLimit)
        | Some(RouteFailureKind::Timeout)
        | Some(RouteFailureKind::Network)
        | Some(RouteFailureKind::Unknown) => ordered.get(1).cloned(),
    }
}
