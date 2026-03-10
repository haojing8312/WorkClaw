export type AdapterState = "starting" | "running" | "degraded" | "stopped" | "error";

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

export interface ChannelAdapter {
  start(config: AdapterConfig): Promise<AdapterHandle>;
  stop(instanceId: string): Promise<void>;
  health(instanceId: string): Promise<AdapterHealth>;
  drainEvents(instanceId: string, limit: number): Promise<NormalizedImEvent[]>;
  sendMessage(instanceId: string, req: SendMessageRequest): Promise<SendMessageResult>;
  ack(instanceId: string, req: AckRequest): Promise<void>;
}
