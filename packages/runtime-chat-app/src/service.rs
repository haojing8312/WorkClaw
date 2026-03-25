use crate::preparation;
use crate::routing;
use crate::traits::{ChatEmployeeDirectory, ChatSessionContextRepository, ChatSettingsRepository};
use crate::types::{
    ChatExecutionContext, ChatExecutionGuidance, ChatExecutionPreparationRequest,
    ChatPreparationRequest, PreparedChatExecution, PreparedChatExecutionAssembly,
    PreparedSessionCreation, SessionCreationRequest,
};

pub struct ChatPreparationService;
pub struct ChatExecutionPreparationService;

impl ChatPreparationService {
    pub fn new() -> Self {
        Self
    }

    pub fn prepare_session_creation(
        &self,
        request: SessionCreationRequest,
    ) -> PreparedSessionCreation {
        preparation::prepare_session_creation(request)
    }

    pub async fn prepare_chat_execution<R: ChatSettingsRepository>(
        &self,
        repo: &R,
        request: ChatPreparationRequest,
    ) -> Result<PreparedChatExecution, String> {
        preparation::prepare_chat_execution(repo, request).await
    }

    pub async fn prepare_route_candidates<R: ChatSettingsRepository>(
        &self,
        repo: &R,
        model_id: &str,
        request: &ChatPreparationRequest,
    ) -> Result<crate::types::PreparedRouteCandidates, String> {
        routing::prepare_route_candidates(repo, model_id, request).await
    }
}

impl ChatExecutionPreparationService {
    pub fn new() -> Self {
        Self
    }

    pub async fn prepare_execution<R>(
        &self,
        repo: &R,
        model_id: &str,
        request: &ChatExecutionPreparationRequest,
    ) -> Result<PreparedChatExecutionAssembly, String>
    where
        R: ChatSettingsRepository + ChatSessionContextRepository,
    {
        preparation::prepare_execution(repo, model_id, request).await
    }

    pub async fn prepare_execution_with_directory<R, D>(
        &self,
        repo: &R,
        directory: &D,
        model_id: &str,
        request: &ChatExecutionPreparationRequest,
    ) -> Result<PreparedChatExecutionAssembly, String>
    where
        R: ChatSettingsRepository + ChatSessionContextRepository,
        D: ChatEmployeeDirectory,
    {
        preparation::prepare_execution_with_directory(repo, directory, model_id, request).await
    }

    pub async fn prepare_employee_collaboration_guidance<D: ChatEmployeeDirectory>(
        &self,
        directory: &D,
        execution_context: &ChatExecutionContext,
    ) -> Result<Option<String>, String> {
        preparation::prepare_employee_collaboration_guidance(directory, execution_context).await
    }

    pub fn resolve_memory_bucket_employee_id<'a>(
        &self,
        execution_context: &'a ChatExecutionContext,
    ) -> &'a str {
        preparation::resolve_memory_bucket_employee_id(execution_context)
    }

    pub fn resolve_skill_root_work_dir<'a>(&self, guidance: &'a ChatExecutionGuidance) -> &'a str {
        preparation::resolve_skill_root_work_dir(guidance)
    }

    pub fn resolve_executor_work_dir(&self, guidance: &ChatExecutionGuidance) -> Option<String> {
        preparation::resolve_executor_work_dir(guidance)
    }

    pub async fn prepare_execution_context<R: ChatSessionContextRepository>(
        &self,
        repo: &R,
        request: &ChatExecutionPreparationRequest,
    ) -> Result<ChatExecutionContext, String> {
        preparation::prepare_execution_context(repo, request).await
    }

    pub async fn prepare_execution_guidance<R: ChatSettingsRepository>(
        &self,
        repo: &R,
        request: &ChatExecutionPreparationRequest,
    ) -> Result<ChatExecutionGuidance, String> {
        preparation::prepare_execution_guidance(repo, request).await
    }

    pub async fn prepare_route_decisions<R: ChatSettingsRepository>(
        &self,
        repo: &R,
        model_id: &str,
        request: &ChatExecutionPreparationRequest,
    ) -> Result<crate::types::PreparedRouteCandidates, String> {
        routing::prepare_route_decisions(repo, model_id, request).await
    }
}
