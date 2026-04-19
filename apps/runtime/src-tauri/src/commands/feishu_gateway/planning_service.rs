use crate::commands::openclaw_gateway::{
    plan_role_dispatch_requests_for_openclaw, plan_role_events_for_openclaw,
};
use crate::im::runtime_bridge::{ImRoleDispatchRequest, ImRoleEventPayload};
use crate::im::types::ImEvent;
use sqlx::SqlitePool;

pub async fn plan_role_events_for_feishu(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<ImRoleEventPayload>, String> {
    plan_role_events_for_openclaw(pool, event).await
}

pub async fn plan_role_dispatch_requests_for_feishu(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<ImRoleDispatchRequest>, String> {
    plan_role_dispatch_requests_for_openclaw(pool, event).await
}
