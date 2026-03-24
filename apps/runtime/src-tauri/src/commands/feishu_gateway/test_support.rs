use sqlx::SqlitePool;

pub async fn list_enabled_employee_feishu_connections_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<super::FeishuEmployeeConnectionInput>, String> {
    super::ingress_service::list_enabled_employee_feishu_connections_with_pool(pool).await
}
