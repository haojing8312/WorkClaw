pub mod anthropic_compat;
pub mod capability_router;
pub mod deepseek;
pub mod moonshot;
pub mod openai_compat;
pub mod qwen;
pub mod registry;
pub mod traits;

pub use capability_router::{route_with_fallback, RouteFailureKind, RouteTarget, RoutingPolicy};
pub use registry::ProviderRegistry;
pub use traits::ProviderPlugin;
