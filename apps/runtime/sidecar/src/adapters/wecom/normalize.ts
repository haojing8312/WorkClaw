import type { NormalizedImEvent } from "../types.js";
import type { WecomInboundEvent } from "./types.js";

export function normalizeWecomEvent(event: WecomInboundEvent): NormalizedImEvent {
  return {
    channel: "wecom",
    workspace_id: event.corp_id,
    account_id: event.agent_id,
    thread_id: event.conversation_id,
    message_id: event.message_id,
    sender_id: event.sender_id,
    sender_name: event.sender_name,
    text: event.text,
    mentions: event.mentions,
    raw_event_type: event.event_type,
    occurred_at: event.occurred_at,
    reply_target: event.reply_target,
    routing_context: {
      peer: {
        kind: event.conversation_type,
        id: event.conversation_id,
      },
      parent_peer: null,
      guild_id: null,
      team_id: null,
      member_role_ids: [],
      identity_links: [],
    },
    raw_payload: event.raw_payload,
  };
}
