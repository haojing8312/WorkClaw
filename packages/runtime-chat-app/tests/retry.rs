use runtime_chat_app::{
    classify_model_route_error, retry_backoff_ms, retry_budget_for_error,
    should_retry_same_candidate, ModelRouteErrorKind,
};

#[test]
fn classifies_route_errors_and_retry_policy() {
    assert_eq!(
        classify_model_route_error("rate limit exceeded"),
        ModelRouteErrorKind::RateLimit
    );
    assert_eq!(
        classify_model_route_error("connection timed out"),
        ModelRouteErrorKind::Timeout
    );
    assert_eq!(
        classify_model_route_error("dns connection failed"),
        ModelRouteErrorKind::Network
    );
    assert_eq!(
        classify_model_route_error("permission denied"),
        ModelRouteErrorKind::Auth
    );
    assert!(should_retry_same_candidate(ModelRouteErrorKind::Network));
    assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Network, 0), 5);
    assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Network, 2), 5);
    assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Auth, 2), 2);
    assert_eq!(retry_backoff_ms(ModelRouteErrorKind::RateLimit, 0), 1200);
    assert_eq!(retry_backoff_ms(ModelRouteErrorKind::Timeout, 1), 1400);
}
