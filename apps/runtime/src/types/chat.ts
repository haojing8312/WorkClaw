import type { ChatMessagePart } from "./preferences";
/// 有序的流式输出项：文字和工具调用按发生顺序排列
export interface StreamItem {
  type: "text" | "tool_call";
  content?: string;          // type === "text" 时的文字内容
  toolCall?: ToolCallInfo;   // type === "tool_call" 时的工具信息
}

export interface Message {
  id?: string;
  role: "user" | "assistant" | "system";
  content: string;
  contentParts?: ChatMessagePart[];
  created_at: string;
  runId?: string | null;
  toolCalls?: ToolCallInfo[];
  reasoning?: {
    status: "thinking" | "completed" | "interrupted";
    duration_ms?: number;
    content: string;
  };
  /// 有序的展示项（新格式），优先使用此字段渲染
  streamItems?: StreamItem[];
}

export interface ToolCallInfo {
  id: string;
  name: string;
  input: Record<string, unknown>;
  output?: string;
  status: "running" | "completed" | "error";
}

export interface SessionToolManifestEntry {
  name: string;
  description: string;
  display_name: string;
  category: string;
  read_only: boolean;
  destructive: boolean;
  concurrency_safe: boolean;
  open_world: boolean;
  requires_approval: boolean;
  source: string;
}

export interface ChatRuntimeAgentState {
  state: string;
  detail?: string;
  iteration: number;
  stopReasonKind?: string;
  stopReasonTitle?: string;
  stopReasonMessage?: string;
  stopReasonLastCompletedStep?: string;
}

export interface ChatRuntimeCompactionStatus {
  phase: "started" | "completed" | "failed";
  detail?: string;
  originalTokens?: number;
  compactedTokens?: number;
  summary?: string;
}

export interface ChatDelegationCardState {
  id: string;
  fromRole: string;
  toRole: string;
  status: "running" | "completed" | "failed";
  taskId?: string;
}

export interface PersistedChatRuntimeState {
  streaming: boolean;
  streamItems: StreamItem[];
  toolManifest: SessionToolManifestEntry[];
  streamReasoning: {
    status: "thinking" | "completed" | "interrupted";
    content: string;
    durationMs?: number;
  } | null;
  agentState: ChatRuntimeAgentState | null;
  compactionStatus?: ChatRuntimeCompactionStatus | null;
  subAgentBuffer: string;
  subAgentRoleName: string;
  mainRoleName: string;
  mainSummaryDelivered: boolean;
  delegationCards: ChatDelegationCardState[];
}

export interface SessionRunProjection {
  id: string;
  session_id: string;
  user_message_id: string;
  assistant_message_id?: string | null;
  status: string;
  buffered_text: string;
  error_kind?: string | null;
  error_message?: string | null;
  created_at: string;
  updated_at: string;
  task_identity?: SessionRunTaskIdentitySnapshot | null;
  turn_state?: SessionRunTurnStateSnapshot | null;
  task_path?: string | null;
  task_status?: string | null;
  task_record?: SessionRunTaskRecordProjection | null;
  task_continuation_mode?: string | null;
  task_continuation_source?: string | null;
  task_continuation_reason?: string | null;
}

export interface SessionRunTaskIdentitySnapshot {
  task_id: string;
  parent_task_id?: string | null;
  root_task_id: string;
  task_kind: string;
  surface_kind: string;
  backend_kind?: string | null;
}

export interface SessionRunTaskRecordProjection {
  task_id: string;
  parent_task_id?: string | null;
  root_task_id: string;
  task_kind: string;
  surface_kind: string;
  backend_kind: string;
  status: string;
  created_at: string;
  updated_at: string;
  started_at?: string | null;
  completed_at?: string | null;
  terminal_reason?: string | null;
}

export interface SessionRunTurnStateCompactionBoundary {
  transcript_path: string;
  original_tokens: number;
  compacted_tokens: number;
  summary: string;
}

export interface SessionRunTurnStateSnapshot {
  task_identity?: SessionRunTaskIdentitySnapshot | null;
  execution_lane?: string | null;
  selected_runner?: string | null;
  selected_skill?: string | null;
  fallback_reason?: string | null;
  allowed_tools?: string[];
  invoked_skills?: string[];
  partial_assistant_text?: string;
  tool_failure_streak?: number;
  reconstructed_history_len?: number | null;
  compaction_boundary?: SessionRunTurnStateCompactionBoundary | null;
}

export interface SessionInfo {
  id: string;
  skill_id?: string;
  title: string;
  display_title?: string;
  created_at: string;
  model_id: string;
  work_dir?: string;
  employee_id?: string;
  employee_name?: string;
  optimistic?: boolean;
  permission_mode?: "standard" | "full_access" | "default" | "accept_edits" | "unrestricted";
  session_mode?: "general" | "employee_direct" | "team_entry";
  team_id?: string;
  permission_mode_label?: string;
  source_channel?: "local" | "app" | "feishu" | "wecom" | string;
  source_label?: string;
  runtime_status?: "running" | "waiting_approval" | "completed" | "failed" | string | null;
}

