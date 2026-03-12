use async_trait::async_trait;
use runtime_chat_app::{
    ChatExecutionPreparationRequest, ChatExecutionPreparationService, ChatSessionContextRepository,
    SessionExecutionContextSnapshot,
};

struct FakeSessionContextRepo;

#[async_trait]
impl ChatSessionContextRepository for FakeSessionContextRepo {
    async fn load_session_execution_context(
        &self,
        session_id: Option<&str>,
    ) -> Result<SessionExecutionContextSnapshot, String> {
        Ok(SessionExecutionContextSnapshot {
            session_id: session_id.unwrap_or_default().to_string(),
            session_mode: "team_entry".to_string(),
            team_id: "repo-team".to_string(),
            employee_id: "repo-employee".to_string(),
            work_dir: "E:/repo-workdir".to_string(),
            imported_mcp_server_ids: vec!["repo-mcp".to_string()],
        })
    }
}

#[tokio::test]
async fn prepare_execution_context_prefers_explicit_request_values() {
    let prepared = ChatExecutionPreparationService::new()
        .prepare_execution_context(
            &FakeSessionContextRepo,
            &ChatExecutionPreparationRequest {
                user_message: "continue".to_string(),
                session_id: Some("session-7".to_string()),
                permission_mode: Some("standard".to_string()),
                session_mode: Some("team_entry".to_string()),
                team_id: Some("request-team".to_string()),
                employee_id: Some("request-employee".to_string()),
                requested_capability: Some("chat".to_string()),
                work_dir: Some("E:/request-workdir".to_string()),
                imported_mcp_server_ids: vec!["request-mcp".to_string()],
            },
        )
        .await
        .expect("execution context");

    assert_eq!(prepared.session_id, "session-7");
    assert_eq!(prepared.session_mode_storage, "team_entry");
    assert_eq!(prepared.normalized_team_id, "request-team");
    assert_eq!(prepared.employee_id, "request-employee");
    assert_eq!(prepared.work_dir, "E:/request-workdir");
    assert_eq!(prepared.imported_mcp_server_ids, vec!["request-mcp"]);
}
