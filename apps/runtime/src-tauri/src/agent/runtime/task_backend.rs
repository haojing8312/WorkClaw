use crate::agent::run_guard::parse_run_stop_reason;
use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
use crate::agent::runtime::events::ToolConfirmResponder;
use crate::agent::runtime::kernel::execution_plan::{
    ExecutionContext, ExecutionLane, ExecutionOutcome, TurnContext,
};
use crate::agent::runtime::kernel::lane_executor::{execute_execution_lane, LaneExecutionParams};
use crate::agent::runtime::kernel::session_profile::SessionSurfaceKind;
use crate::agent::runtime::kernel::turn_preparation::{
    prepare_employee_step_turn, prepare_hidden_child_turn, prepare_local_turn,
    PrepareLocalTurnParams,
};
use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
use crate::agent::runtime::runtime_io as chat_io;
use crate::agent::runtime::session_runtime::SessionRuntime;
use crate::agent::runtime::skill_routing::runner::plan_implicit_route_with_observation;
use crate::agent::runtime::task_state::{TaskBackendKind, TaskState};
use crate::agent::types::StreamDelta;
use crate::agent::AgentExecutor;
use crate::model_transport::resolve_model_transport;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::AppHandle;

pub(crate) type TaskBackendTokenCallback = Arc<dyn Fn(StreamDelta) + Send + Sync + 'static>;

pub(crate) struct InteractiveChatTaskBackendPreparationRequest<'a> {
    pub app: &'a AppHandle,
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub db: &'a sqlx::SqlitePool,
    pub session_id: &'a str,
    pub user_message: &'a str,
    pub user_message_parts: &'a [Value],
    pub max_iterations_override: Option<usize>,
}

pub(crate) struct InteractiveChatTaskBackendRequest<'a> {
    pub app: AppHandle,
    pub agent_executor: Arc<AgentExecutor>,
    pub db: &'a sqlx::SqlitePool,
    pub journal: &'a crate::session_journal::SessionJournalStore,
    pub session_id: &'a str,
    pub run_id: &'a str,
    pub user_message: &'a str,
    pub turn_context: &'a TurnContext,
    pub execution_context: &'a ExecutionContext,
    pub cancel_flag: Arc<AtomicBool>,
    pub tool_confirm_responder: ToolConfirmResponder,
}

pub(crate) struct PreparedSurfaceTaskBackendRequest<'a> {
    pub app_handle: Option<AppHandle>,
    pub agent_executor: Arc<AgentExecutor>,
    pub session_id: &'a str,
    pub turn_context: &'a TurnContext,
    pub execution_context: &'a ExecutionContext,
    pub on_token: TaskBackendTokenCallback,
}

pub(crate) struct HiddenChildTaskBackendPreparationRequest<'a> {
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub prompt: &'a str,
    pub agent_type: &'a str,
    pub delegate_display_name: &'a str,
    pub api_format: &'a str,
    pub base_url: &'a str,
    pub api_key: &'a str,
    pub model: &'a str,
    pub allowed_tools: Option<Vec<String>>,
    pub max_iterations: usize,
    pub work_dir: Option<String>,
}

pub(crate) struct EmployeeStepTaskBackendPreparationRequest<'a> {
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub user_prompt: &'a str,
    pub employee_step_system_prompt: &'a str,
    pub api_format: &'a str,
    pub base_url: &'a str,
    pub api_key: &'a str,
    pub model: &'a str,
    pub allowed_tools: Option<Vec<String>>,
    pub max_iterations: usize,
    pub work_dir: Option<String>,
}

pub(crate) enum TaskBackendPreparationRequest<'a> {
    InteractiveChat(InteractiveChatTaskBackendPreparationRequest<'a>),
    HiddenChild(HiddenChildTaskBackendPreparationRequest<'a>),
    EmployeeStep(EmployeeStepTaskBackendPreparationRequest<'a>),
}

pub(crate) struct PreparedTaskBackendSurface {
    pub contract: TaskBackendContract,
    pub turn_context: TurnContext,
    pub execution_context: ExecutionContext,
}

pub(crate) enum TaskBackendExecutionContext<'a> {
    InteractiveChat {
        app: AppHandle,
        agent_executor: Arc<AgentExecutor>,
        db: &'a sqlx::SqlitePool,
        journal: &'a crate::session_journal::SessionJournalStore,
        session_id: &'a str,
        run_id: &'a str,
        user_message: &'a str,
        cancel_flag: Arc<AtomicBool>,
        tool_confirm_responder: ToolConfirmResponder,
    },
    Delegated {
        app_handle: Option<AppHandle>,
        agent_executor: Arc<AgentExecutor>,
        session_id: &'a str,
        on_token: TaskBackendTokenCallback,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskBackendContract {
    InteractiveChat,
    HiddenChild,
    EmployeeStep,
}

impl TaskBackendContract {
    pub(crate) fn backend_kind(self) -> TaskBackendKind {
        match self {
            TaskBackendContract::InteractiveChat => TaskBackendKind::InteractiveChatBackend,
            TaskBackendContract::HiddenChild => TaskBackendKind::HiddenChildBackend,
            TaskBackendContract::EmployeeStep => TaskBackendKind::EmployeeStepBackend,
        }
    }

    pub(crate) fn generic_error_kind(self) -> &'static str {
        self.backend_kind().generic_error_kind()
    }

    pub(crate) fn session_surface(self) -> SessionSurfaceKind {
        match self {
            TaskBackendContract::InteractiveChat => SessionSurfaceKind::LocalChat,
            TaskBackendContract::HiddenChild => SessionSurfaceKind::HiddenChildSession,
            TaskBackendContract::EmployeeStep => SessionSurfaceKind::EmployeeStepSession,
        }
    }

    pub(crate) fn missing_candidate_message(self) -> &'static str {
        match self {
            TaskBackendContract::InteractiveChat => "本地聊天缺少可执行模型候选",
            TaskBackendContract::HiddenChild => "隐藏子会话缺少可执行模型候选",
            TaskBackendContract::EmployeeStep => "员工步骤会话缺少可执行模型候选",
        }
    }
}

pub(crate) enum TaskBackendRunRequest<'a> {
    InteractiveChat(InteractiveChatTaskBackendRequest<'a>),
    HiddenChild(PreparedSurfaceTaskBackendRequest<'a>),
    EmployeeStep(PreparedSurfaceTaskBackendRequest<'a>),
}

pub(crate) fn attach_active_task_state_to_execution_context(
    execution_context: &mut ExecutionContext,
    task_state: &TaskState,
) {
    execution_context.active_task_identity = Some(task_state.task_identity.clone());
    execution_context.active_task_kind = Some(task_state.task_kind);
    execution_context.active_task_surface = Some(task_state.surface_kind);
    execution_context.active_task_backend = Some(task_state.backend_kind);
    execution_context.active_task_continuation_mode = task_state.continuation_mode;
    execution_context.active_task_continuation_source = task_state.continuation_source;
    execution_context.active_task_continuation_reason = task_state.continuation_reason.clone();
}

impl TaskBackendRunRequest<'_> {
    pub(crate) fn contract(&self) -> TaskBackendContract {
        match self {
            TaskBackendRunRequest::InteractiveChat(_) => TaskBackendContract::InteractiveChat,
            TaskBackendRunRequest::HiddenChild(_) => TaskBackendContract::HiddenChild,
            TaskBackendRunRequest::EmployeeStep(_) => TaskBackendContract::EmployeeStep,
        }
    }

    pub(crate) fn backend_kind(&self) -> TaskBackendKind {
        self.contract().backend_kind()
    }

    pub(crate) fn session_surface(&self) -> SessionSurfaceKind {
        self.contract().session_surface()
    }
}

impl TaskBackendPreparationRequest<'_> {
    pub(crate) fn contract(&self) -> TaskBackendContract {
        match self {
            TaskBackendPreparationRequest::InteractiveChat(_) => {
                TaskBackendContract::InteractiveChat
            }
            TaskBackendPreparationRequest::HiddenChild(_) => TaskBackendContract::HiddenChild,
            TaskBackendPreparationRequest::EmployeeStep(_) => TaskBackendContract::EmployeeStep,
        }
    }

    pub(crate) fn generic_error_kind(&self) -> &'static str {
        self.contract().generic_error_kind()
    }
}

pub(crate) async fn prepare_task_backend(
    request: TaskBackendPreparationRequest<'_>,
) -> Result<PreparedTaskBackendSurface, String> {
    let contract = request.contract();
    let (turn_context, execution_context) = match request {
        TaskBackendPreparationRequest::InteractiveChat(request) => {
            prepare_local_turn(PrepareLocalTurnParams {
                app: request.app,
                db: request.db,
                agent_executor: request.agent_executor,
                session_id: request.session_id,
                user_message: request.user_message,
                user_message_parts: request.user_message_parts,
                max_iterations_override: request.max_iterations_override,
            })
            .await?
        }
        TaskBackendPreparationRequest::HiddenChild(request) => prepare_hidden_child_turn(
            request.agent_executor,
            request.prompt,
            request.agent_type,
            request.delegate_display_name,
            request.api_format,
            request.base_url,
            request.api_key,
            request.model,
            request.allowed_tools,
            request.max_iterations,
            request.work_dir,
        ),
        TaskBackendPreparationRequest::EmployeeStep(request) => prepare_employee_step_turn(
            request.agent_executor,
            request.user_prompt,
            request.employee_step_system_prompt,
            request.api_format,
            request.base_url,
            request.api_key,
            request.model,
            request.allowed_tools,
            request.max_iterations,
            request.work_dir,
        ),
    };

    Ok(PreparedTaskBackendSurface {
        contract,
        turn_context,
        execution_context,
    })
}

pub(crate) async fn run_task_backend(
    request: TaskBackendRunRequest<'_>,
) -> Result<ExecutionOutcome, String> {
    debug_assert_eq!(request.contract().backend_kind(), request.backend_kind());
    debug_assert_eq!(
        request.contract().session_surface(),
        request.session_surface()
    );

    match request {
        TaskBackendRunRequest::InteractiveChat(request) => {
            run_interactive_chat_backend(request).await
        }
        TaskBackendRunRequest::HiddenChild(request) => {
            run_delegated_surface_backend(TaskBackendContract::HiddenChild, request).await
        }
        TaskBackendRunRequest::EmployeeStep(request) => {
            run_delegated_surface_backend(TaskBackendContract::EmployeeStep, request).await
        }
    }
}

async fn run_interactive_chat_backend(
    request: InteractiveChatTaskBackendRequest<'_>,
) -> Result<ExecutionOutcome, String> {
    debug_assert_eq!(
        request.execution_context.session_profile.surface,
        SessionSurfaceKind::LocalChat
    );

    match SessionRuntime::maybe_execute_user_skill_command(
        &request.app,
        &request.agent_executor,
        request.session_id,
        request.run_id,
        request.user_message,
        request.execution_context,
        request.cancel_flag.clone(),
        request.tool_confirm_responder.clone(),
    )
    .await
    {
        Ok(Some(dispatch_outcome)) => {
            let turn_state = TurnStateSnapshot::new(
                request
                    .execution_context
                    .allowed_tools()
                    .map(|tools| tools.to_vec()),
            )
            .with_session_surface(request.execution_context.session_profile.surface)
            .with_execution_lane(ExecutionLane::DirectDispatch)
            .with_invoked_skill(dispatch_outcome.skill_id);
            return Ok(ExecutionOutcome::DirectDispatch {
                output: dispatch_outcome.output,
                turn_state,
            });
        }
        Ok(None) => {}
        Err(dispatch_error) => {
            let error = dispatch_error.error;
            let turn_state = TurnStateSnapshot::new(
                request
                    .execution_context
                    .allowed_tools()
                    .map(|tools| tools.to_vec()),
            )
            .with_session_surface(request.execution_context.session_profile.surface)
            .with_execution_lane(ExecutionLane::DirectDispatch)
            .with_invoked_skill(dispatch_error.skill_id);
            return Ok(match parse_run_stop_reason(&error) {
                Some(stop_reason) => ExecutionOutcome::SkillCommandStopped {
                    turn_state: turn_state.with_stop_reason(stop_reason.clone()),
                    stop_reason,
                    error,
                },
                None => ExecutionOutcome::SkillCommandFailed { error, turn_state },
            });
        }
    }

    let planned_route = plan_implicit_route_with_observation(
        &request.execution_context.route_index,
        &request.execution_context.workspace_skill_entries,
        request.execution_context.skill_command_specs(),
        request.user_message,
        request.turn_context.continuation_preference.as_ref(),
    );
    let execution_plan = planned_route.execution_plan.clone();
    chat_io::append_skill_route_recorded_with_pool(
        request.db,
        request.journal,
        request.session_id,
        request.run_id,
        &planned_route.observation,
        request.execution_context.tool_plan_record(),
    )
    .await?;

    let mut turn_state = TurnStateSnapshot::default()
        .with_session_surface(request.execution_context.session_profile.surface)
        .with_route_observation(planned_route.observation.clone())
        .with_execution_lane(execution_plan.lane);
    if let Some(skill_id) = planned_route.observation.selected_skill.as_deref() {
        turn_state = turn_state.with_invoked_skill(skill_id);
    }

    execute_execution_lane(LaneExecutionParams {
        app: &request.app,
        agent_executor: &request.agent_executor,
        db: request.db,
        session_id: request.session_id,
        run_id: request.run_id,
        turn_context: request.turn_context,
        execution_context: request.execution_context,
        execution_plan: &execution_plan,
        turn_state,
        cancel_flag: request.cancel_flag,
        tool_confirm_responder: request.tool_confirm_responder,
    })
    .await
}

async fn run_delegated_surface_backend(
    contract: TaskBackendContract,
    request: PreparedSurfaceTaskBackendRequest<'_>,
) -> Result<ExecutionOutcome, String> {
    let expected_surface = contract.session_surface();
    debug_assert_eq!(
        request.execution_context.session_profile.surface,
        expected_surface
    );

    let Some((provider_key, api_format, base_url, model_name, api_key)) =
        request.turn_context.primary_route_candidate()
    else {
        return Err(contract.missing_candidate_message().to_string());
    };

    let transport = resolve_model_transport(
        api_format,
        base_url,
        Some(provider_key.as_str()).filter(|value| !value.trim().is_empty()),
    );
    let streamed_text = Arc::new(Mutex::new(String::new()));
    let streamed_text_for_callback = Arc::clone(&streamed_text);
    let callback = Arc::clone(&request.on_token);

    let route_execution = match request
        .agent_executor
        .execute_turn_with_transport_outcome(
            transport,
            api_format,
            base_url,
            api_key,
            model_name,
            &request.execution_context.system_prompt,
            request.turn_context.messages.clone(),
            move |delta| {
                if let StreamDelta::Text(token) = &delta {
                    if let Ok(mut buffer) = streamed_text_for_callback.lock() {
                        buffer.push_str(token);
                    }
                }
                callback(delta);
            },
            request.app_handle.as_ref(),
            Some(request.session_id),
            request.execution_context.allowed_tools(),
            request.execution_context.permission_mode,
            None,
            request.execution_context.executor_work_dir.clone(),
            request.execution_context.max_iterations,
            None,
            Some(request.execution_context.node_timeout_seconds),
            Some(request.execution_context.route_retry_count),
        )
        .await
    {
        Ok(outcome) => RouteExecutionOutcome {
            final_messages: Some(outcome.messages),
            last_error: None,
            last_error_kind: None,
            last_stop_reason: None,
            partial_text: streamed_text
                .lock()
                .map(|buffer| buffer.clone())
                .unwrap_or_default(),
            reasoning_text: String::new(),
            reasoning_duration_ms: None,
            tool_exposure_expanded: false,
            tool_exposure_expansion_reason: None,
            compaction_boundary: outcome.compaction_outcome.as_ref().map(Into::into),
        },
        Err(error) => {
            let error_text = error.error.to_string();
            let stop_reason = parse_run_stop_reason(&error_text);
            RouteExecutionOutcome {
                final_messages: None,
                last_error: Some(error_text),
                last_error_kind: Some(
                    stop_reason
                        .as_ref()
                        .map(|reason| reason.kind.as_key().to_string())
                        .unwrap_or_else(|| contract.generic_error_kind().to_string()),
                ),
                last_stop_reason: stop_reason,
                partial_text: streamed_text
                    .lock()
                    .map(|buffer| buffer.clone())
                    .unwrap_or_default(),
                reasoning_text: String::new(),
                reasoning_duration_ms: None,
                tool_exposure_expanded: false,
                tool_exposure_expansion_reason: None,
                compaction_boundary: error.compaction_outcome.as_ref().map(Into::into),
            }
        }
    };

    let reconstructed_history_len = request.turn_context.messages.len();
    let turn_state = TurnStateSnapshot::new(
        request
            .execution_context
            .allowed_tools()
            .map(|tools| tools.to_vec()),
    )
    .with_session_surface(request.execution_context.session_profile.surface)
    .with_execution_lane(ExecutionLane::OpenTask)
    .with_route_execution(&route_execution, reconstructed_history_len);

    Ok(ExecutionOutcome::RouteExecution {
        route_execution,
        reconstructed_history_len,
        turn_state,
    })
}

pub(crate) async fn execute_prepared_task_backend_with_context(
    prepared_surface: &PreparedTaskBackendSurface,
    context: TaskBackendExecutionContext<'_>,
) -> Result<ExecutionOutcome, String> {
    match context {
        TaskBackendExecutionContext::InteractiveChat {
            app,
            agent_executor,
            db,
            journal,
            session_id,
            run_id,
            user_message,
            cancel_flag,
            tool_confirm_responder,
        } => {
            if prepared_surface.contract != TaskBackendContract::InteractiveChat {
                return Err(
                    "interactive chat backend requires interactive prepared surface".to_string(),
                );
            }
            run_task_backend(TaskBackendRunRequest::InteractiveChat(
                InteractiveChatTaskBackendRequest {
                    app,
                    agent_executor,
                    db,
                    journal,
                    session_id,
                    run_id,
                    user_message,
                    turn_context: &prepared_surface.turn_context,
                    execution_context: &prepared_surface.execution_context,
                    cancel_flag,
                    tool_confirm_responder,
                },
            ))
            .await
        }
        TaskBackendExecutionContext::Delegated {
            app_handle,
            agent_executor,
            session_id,
            on_token,
        } => {
            if prepared_surface.contract == TaskBackendContract::InteractiveChat {
                return Err(
                    "interactive chat backend requires interactive execution params".to_string(),
                );
            }
            let run_request = match prepared_surface.contract {
                TaskBackendContract::HiddenChild => {
                    TaskBackendRunRequest::HiddenChild(PreparedSurfaceTaskBackendRequest {
                        app_handle,
                        agent_executor,
                        session_id,
                        turn_context: &prepared_surface.turn_context,
                        execution_context: &prepared_surface.execution_context,
                        on_token,
                    })
                }
                TaskBackendContract::EmployeeStep => {
                    TaskBackendRunRequest::EmployeeStep(PreparedSurfaceTaskBackendRequest {
                        app_handle,
                        agent_executor,
                        session_id,
                        turn_context: &prepared_surface.turn_context,
                        execution_context: &prepared_surface.execution_context,
                        on_token,
                    })
                }
                TaskBackendContract::InteractiveChat => {
                    debug_assert!(
                        false,
                        "interactive chat prepared surfaces should use interactive execution"
                    );
                    return Err(
                        "interactive chat backend requires interactive execution params"
                            .to_string(),
                    );
                }
            };
            run_task_backend(run_request).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        execute_prepared_task_backend_with_context, prepare_task_backend,
        EmployeeStepTaskBackendPreparationRequest, HiddenChildTaskBackendPreparationRequest,
        TaskBackendContract, TaskBackendExecutionContext, TaskBackendPreparationRequest,
        TaskBackendTokenCallback,
    };
    use crate::agent::runtime::kernel::session_profile::SessionSurfaceKind;
    use crate::agent::runtime::task_state::TaskBackendKind;
    use crate::agent::{AgentExecutor, ToolRegistry};
    use std::sync::Arc;

    #[test]
    fn backend_kind_reports_local_chat_contract() {
        assert_eq!(
            TaskBackendContract::InteractiveChat.backend_kind(),
            TaskBackendKind::InteractiveChatBackend
        );
        assert_eq!(
            TaskBackendContract::InteractiveChat.session_surface(),
            SessionSurfaceKind::LocalChat
        );
    }

    #[test]
    fn backend_kind_reports_delegated_surface_contracts() {
        assert_eq!(
            TaskBackendContract::HiddenChild.backend_kind(),
            TaskBackendKind::HiddenChildBackend
        );
        assert_eq!(
            TaskBackendContract::HiddenChild.session_surface(),
            SessionSurfaceKind::HiddenChildSession
        );
        assert_eq!(
            TaskBackendContract::EmployeeStep.backend_kind(),
            TaskBackendKind::EmployeeStepBackend
        );
        assert_eq!(
            TaskBackendContract::EmployeeStep.session_surface(),
            SessionSurfaceKind::EmployeeStepSession
        );
    }

    #[test]
    fn hidden_child_prepare_projects_hidden_child_surface_contract() {
        let agent_executor = Arc::new(AgentExecutor::new(Arc::new(ToolRegistry::new())));

        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let prepared = runtime
            .block_on(prepare_task_backend(
                TaskBackendPreparationRequest::HiddenChild(
                    HiddenChildTaskBackendPreparationRequest {
                        agent_executor: &agent_executor,
                        prompt: "summarize",
                        agent_type: "default",
                        delegate_display_name: "delegate",
                        api_format: "openai",
                        base_url: "http://localhost",
                        api_key: "test-key",
                        model: "gpt-test",
                        allowed_tools: None,
                        max_iterations: 2,
                        work_dir: Some("C:/tmp".to_string()),
                    },
                ),
            ))
            .expect("prepare hidden child backend");

        assert_eq!(prepared.contract, TaskBackendContract::HiddenChild);
        assert_eq!(
            prepared.execution_context.session_profile.surface,
            SessionSurfaceKind::HiddenChildSession
        );
        assert_eq!(prepared.turn_context.user_message, "summarize");
    }

    #[test]
    fn employee_step_prepare_projects_employee_surface_contract() {
        let agent_executor = Arc::new(AgentExecutor::new(Arc::new(ToolRegistry::new())));

        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let prepared = runtime
            .block_on(prepare_task_backend(
                TaskBackendPreparationRequest::EmployeeStep(
                    EmployeeStepTaskBackendPreparationRequest {
                        agent_executor: &agent_executor,
                        user_prompt: "review this",
                        employee_step_system_prompt: "act like reviewer",
                        api_format: "openai",
                        base_url: "http://localhost",
                        api_key: "test-key",
                        model: "gpt-test",
                        allowed_tools: None,
                        max_iterations: 2,
                        work_dir: Some("C:/tmp".to_string()),
                    },
                ),
            ))
            .expect("prepare employee backend");

        assert_eq!(prepared.contract, TaskBackendContract::EmployeeStep);
        assert_eq!(
            prepared.execution_context.session_profile.surface,
            SessionSurfaceKind::EmployeeStepSession
        );
        assert_eq!(prepared.turn_context.user_message, "review this");
    }

    #[test]
    fn delegated_execution_rejects_interactive_contract() {
        let prepared_surface = super::PreparedTaskBackendSurface {
            contract: TaskBackendContract::InteractiveChat,
            turn_context: crate::agent::runtime::kernel::execution_plan::TurnContext::default(),
            execution_context:
                crate::agent::runtime::kernel::execution_plan::ExecutionContext::default(),
        };
        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let result = runtime.block_on(execute_prepared_task_backend_with_context(
            &prepared_surface,
            TaskBackendExecutionContext::Delegated {
                app_handle: None,
                agent_executor: Arc::new(AgentExecutor::new(Arc::new(ToolRegistry::new()))),
                session_id: "session-1",
                on_token: Arc::new(|_| {}) as TaskBackendTokenCallback,
            },
        ));

        assert!(
            matches!(result, Err(message) if message.contains("interactive chat backend requires interactive execution params"))
        );
    }
}
