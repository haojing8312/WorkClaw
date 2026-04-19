#[path = "im_host/channel_registry.rs"]
pub(crate) mod channel_registry;
#[path = "im_host/channel_runtime_state.rs"]
pub(crate) mod channel_runtime_state;
#[path = "im_host/chunk_planner.rs"]
pub(crate) mod chunk_planner;
#[path = "im_host/contract.rs"]
pub(crate) mod contract;
#[path = "im_host/delivery_trace.rs"]
pub(crate) mod delivery_trace;
#[path = "im_host/host_metadata.rs"]
pub(crate) mod host_metadata;
#[path = "im_host/inbound_bridge.rs"]
pub(crate) mod inbound_bridge;
#[path = "im_host/interactive_messages.rs"]
pub(crate) mod interactive_messages;
#[path = "im_host/interactive_dispatch.rs"]
pub(crate) mod interactive_dispatch;
#[path = "im_host/lifecycle.rs"]
pub(crate) mod lifecycle;
#[path = "im_host/outbound_transport.rs"]
pub(crate) mod outbound_transport;
#[path = "im_host/runtime_adapter.rs"]
pub(crate) mod runtime_adapter;
#[path = "im_host/runtime_commands.rs"]
pub(crate) mod runtime_commands;
#[path = "im_host/runtime_events.rs"]
pub(crate) mod runtime_events;
#[path = "im_host/runtime_observability.rs"]
pub(crate) mod runtime_observability;
#[path = "im_host/runtime_registry.rs"]
pub(crate) mod runtime_registry;
#[path = "im_host/runtime_router.rs"]
pub(crate) mod runtime_router;
#[path = "im_host/runtime_status.rs"]
pub(crate) mod runtime_status;
#[path = "im_host/runtime_waiters.rs"]
pub(crate) mod runtime_waiters;
#[path = "im_host/sidecar_channel.rs"]
pub(crate) mod sidecar_channel;
#[path = "im_host/startup_restore.rs"]
pub(crate) mod startup_restore;
#[path = "im_host/target_resolver.rs"]
pub(crate) mod target_resolver;

pub(crate) use channel_runtime_state::{
    get_im_channel_host_runtime_snapshot_in_state, get_im_channel_runtime_status_in_state,
    record_im_channel_host_action, record_im_channel_restore_report, record_im_channel_runtime_status,
    ImChannelHostRuntimeSnapshot, ImChannelHostRuntimeState,
};
pub(crate) use chunk_planner::plan_text_chunks;
pub(crate) use contract::{
    ImReplyDeliveryPlan, ImReplyDeliveryState, ImReplyLifecycleEvent, ImReplyLifecyclePhase,
};
pub(crate) use delivery_trace::ReplyDeliveryTrace;
pub(crate) use host_metadata::{
    resolve_im_host_metadata_with_pool, ImChannelAccountMetadata, ImHostMetadata,
};
pub(crate) use inbound_bridge::{
    dispatch_im_inbound_to_workclaw_with_pool_and_app, emit_inbound_dispatch_sessions,
    maybe_handle_registered_approval_command_with_pool_and_app, parse_normalized_im_event_value,
};
pub(crate) use interactive_messages::{
    build_im_approval_request_text, build_im_approval_resolution_text,
    build_im_approval_resolved_notice_text, build_im_ask_user_request_text,
    load_approval_resolution_notification_with_pool,
};
pub(crate) use interactive_dispatch::{
    maybe_notify_registered_approval_requested_with_pool,
    maybe_notify_registered_approval_resolved_with_pool,
    maybe_notify_registered_ask_user_requested_with_pool,
    prepare_channel_interactive_approval_notice_with_pool,
    prepare_channel_interactive_session_thread_with_pool,
};
pub(crate) use lifecycle::{
    emit_registered_lifecycle_phase_for_session_with_pool,
    maybe_dispatch_registered_im_session_reply_with_pool,
    maybe_emit_registered_host_lifecycle_phase_for_session_with_pool,
    maybe_stop_registered_host_processing_for_session_with_pool,
    lookup_channel_source_for_session_with_pool, lookup_channel_thread_for_session_with_pool,
    lookup_latest_inbox_message_id_for_thread_with_pool,
    stop_registered_processing_for_session_with_pool,
};
pub(crate) use outbound_transport::{
    execute_reply_plan_with_transport, ImReplyPlanTransport,
};
pub(crate) use runtime_adapter::{
    handle_runtime_stdout_line_with_adapter, ImRuntimeStdoutAdapter,
};
pub(crate) use runtime_commands::{
    build_runtime_lifecycle_event_command_payload, build_runtime_processing_stop_command_payload,
    build_runtime_text_command_payload, ImRuntimeLifecycleEventCommandPayload,
    ImRuntimeProcessingStopCommandPayload, ImRuntimeTextCommandPayload,
};
pub(crate) use runtime_events::parse_runtime_event;
pub(crate) use runtime_observability::{merge_runtime_reply_lifecycle_event, trim_recent_entries};
pub(crate) use runtime_registry::{
    ensure_runtime_stdin_for_commands, write_runtime_command_json,
};
pub(crate) use runtime_status::merge_runtime_status_event;
pub(crate) use runtime_waiters::{
    deliver_runtime_command_error, deliver_runtime_result_with_status,
    drop_pending_runtime_request_with_status, fail_pending_runtime_requests_with_status,
    register_pending_runtime_request_with_status,
};
pub(crate) use sidecar_channel::{
    build_sidecar_channel_instance_id, build_sidecar_text_message_request, parse_sidecar_channel_health,
};
pub(crate) use startup_restore::restore_im_channels_with_pool;
pub(crate) use target_resolver::{
    build_direct_reply_route_target, resolve_dispatch_thread_target, ImDirectRouteTargetOptions,
};
