export interface WecomMention {
  type: string;
  id: string;
  name?: string | null;
}

export interface WecomInboundEvent {
  connector_id: string;
  corp_id: string;
  agent_id: string;
  event_type: string;
  conversation_type: "direct" | "group";
  conversation_id: string;
  topic_id?: string | null;
  root_id?: string | null;
  message_id: string;
  sender_id: string;
  sender_name: string | null;
  text: string;
  mentions: WecomMention[];
  occurred_at: string;
  reply_target: string | null;
  raw_payload: unknown;
}

export interface WecomAdapterHealthSnapshot {
  running: boolean;
  started_at: string | null;
  last_event_at: string | null;
  last_error: string | null;
  reconnect_attempts: number;
  queued_events: number;
}
