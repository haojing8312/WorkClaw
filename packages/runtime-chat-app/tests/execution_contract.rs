use runtime_chat_app::{
    ChatExecutionPreparationRequest, ChatExecutionPreparationService, PreparedChatExecution,
};

#[test]
fn exposes_execution_preparation_contract() {
    let _service = ChatExecutionPreparationService::new();
    let request = ChatExecutionPreparationRequest {
        user_message: "please prepare route candidates".to_string(),
        session_id: Some("session-123".to_string()),
        permission_mode: Some("standard".to_string()),
        session_mode: Some("team_entry".to_string()),
        team_id: Some("team-42".to_string()),
        employee_id: Some("employee-9".to_string()),
        requested_capability: Some("chat".to_string()),
        work_dir: Some("E:/code/yzpd/workclaw".to_string()),
        imported_mcp_server_ids: vec!["mcp-filesystem".to_string()],
    };

    assert_eq!(request.session_id.as_deref(), Some("session-123"));
    assert_eq!(request.employee_id.as_deref(), Some("employee-9"));
    assert_eq!(request.imported_mcp_server_ids, vec!["mcp-filesystem"]);

    let prepared = PreparedChatExecution::default();
    assert_eq!(prepared.capability, "chat");
    assert_eq!(prepared.permission_mode_storage, "standard");
    assert_eq!(prepared.session_mode_storage, "general");
    assert_eq!(prepared.execution_context.employee_id, "");
    assert_eq!(prepared.execution_context.imported_mcp_server_ids.len(), 0);
}
