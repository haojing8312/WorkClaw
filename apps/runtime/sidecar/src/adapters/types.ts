export type AdapterState = "starting" | "running" | "degraded" | "stopped" | "error";
export type ConnectorStatus =
  | "needs_configuration"
  | "connected"
  | "degraded"
  | "authentication_error"
  | "connection_error"
  | "stopped";
export type ConnectorCapability =
  | "receive_text"
  | "send_text"
  | "group_route"
  | "direct_route";

export interface ConnectorIssue {
  code: string;
  category: Exclude<ConnectorStatus, "connected" | "needs_configuration" | "stopped">;
  user_message: string;
  technical_message: string;
  retryable: boolean;
  occurred_at: string | null;
}

export interface ConnectorDescriptor {
  channel: string;
  display_name: string;
  capabilities: ConnectorCapability[];
}

export interface RoutingContextPeer {
  kind: string;
  id: string;
}

export interface IdentityLink {
  source: string;
  target: string;
}

export interface RoutingContext {
  peer: RoutingContextPeer;
  topic?: RoutingContextPeer | null;
  parent_peer: RoutingContextPeer | null;
  guild_id: string | null;
  team_id: string | null;
  member_role_ids: string[];
  identity_links: IdentityLink[];
}

export interface MentionRef {
  type: string;
  id: string;
  name?: string | null;
}

export interface NormalizedImEvent {
  channel: string;
  workspace_id: string;
  account_id: string;
  thread_id: string;
  chat_type?: string;
  topic_id?: string | null;
  root_id?: string | null;
  message_id: string;
  sender_id: string;
  sender_name: string | null;
  text: string;
  mentions: MentionRef[];
  raw_event_type: string;
  occurred_at: string;
  reply_target: string | null;
  routing_context: RoutingContext;
  raw_payload: unknown;
}

export interface AdapterConfig {
  adapter_name: string;
  connector_id: string;
  settings: Record<string, unknown>;
}

export interface AdapterHandle {
  instance_id: string;
}

export interface AdapterHealth {
  adapter_name: string;
  instance_id: string;
  state: AdapterState;
  last_ok_at: string | null;
  last_error: string | null;
  reconnect_attempts: number;
  queue_depth: number;
  issue?: ConnectorIssue | null;
}

export interface SendMessageRequest {
  channel: string;
  thread_id: string;
  reply_target: string | null;
  text: string;
  format?: string;
  attachments?: unknown[];
  mentions?: MentionRef[];
  idempotency_key?: string;
}

export interface SendMessageResult {
  message_id: string;
  delivered_at: string | null;
  raw_response: unknown;
}

export interface AckRequest {
  message_id: string;
  status?: string;
}

export interface ConnectorReplayStats {
  retained_events: number;
  acked_events: number;
}

export interface ConnectorDiagnostics {
  connector: ConnectorDescriptor;
  status: ConnectorStatus;
  health: AdapterHealth;
  replay: ConnectorReplayStats;
}

export function classifyConnectorIssue(
  technicalMessage: string | null | undefined,
  occurredAt: string | null = null,
): ConnectorIssue | null {
  const message = String(technicalMessage || "").trim();
  if (!message) {
    return null;
  }

  const lower = message.toLowerCase();
  if (lower.includes("signature mismatch")) {
    return {
      code: "signature_mismatch",
      category: "authentication_error",
      user_message: "签名校验失败",
      technical_message: message,
      retryable: false,
      occurred_at: occurredAt,
    };
  }

  if (lower.includes("token") || lower.includes("secret") || lower.includes("credential")) {
    return {
      code: "invalid_credentials",
      category: "authentication_error",
      user_message: "凭据配置异常",
      technical_message: message,
      retryable: false,
      occurred_at: occurredAt,
    };
  }

  if (lower.includes("timeout")) {
    return {
      code: "connection_timeout",
      category: "connection_error",
      user_message: "连接超时",
      technical_message: message,
      retryable: true,
      occurred_at: occurredAt,
    };
  }

  if (lower.includes("refused") || lower.includes("unreachable") || lower.includes("network")) {
    return {
      code: "connection_unavailable",
      category: "connection_error",
      user_message: "连接不可用",
      technical_message: message,
      retryable: true,
      occurred_at: occurredAt,
    };
  }

  return {
    code: "connector_error",
    category: "connection_error",
    user_message: "连接异常",
    technical_message: message,
    retryable: true,
    occurred_at: occurredAt,
  };
}

export function normalizeConnectorStatus(health: AdapterHealth): ConnectorStatus {
  if (health.issue?.category === "authentication_error") {
    return "authentication_error";
  }
  if (health.issue?.category === "connection_error") {
    return health.state === "degraded" ? "degraded" : "connection_error";
  }
  if (health.state === "running") {
    return "connected";
  }
  if (health.state === "degraded") {
    return "degraded";
  }
  return "stopped";
}

export interface ChannelAdapter {
  describe(): Promise<ConnectorDescriptor> | ConnectorDescriptor;
  start(config: AdapterConfig): Promise<AdapterHandle>;
  stop(instanceId: string): Promise<void>;
  health(instanceId: string): Promise<AdapterHealth>;
  drainEvents(instanceId: string, limit: number): Promise<NormalizedImEvent[]>;
  sendMessage(instanceId: string, req: SendMessageRequest): Promise<SendMessageResult>;
  ack(instanceId: string, req: AckRequest): Promise<void>;
}
