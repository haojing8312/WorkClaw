import type { NormalizedImEvent } from "../types.js";
import type { WecomInboundEvent } from "./types.js";

function readRawStringField(
  rawPayload: unknown,
  ...keys: string[]
): string | null {
  if (!rawPayload || typeof rawPayload !== "object") {
    return null;
  }

  const record = rawPayload as Record<string, unknown>;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }

  return null;
}

export function normalizeWecomEvent(event: WecomInboundEvent): NormalizedImEvent {
  const topicId =
    event.topic_id?.trim() ||
    readRawStringField(event.raw_payload, "topic_id", "topicId", "thread_id", "threadId");
  const rootId =
    event.root_id?.trim() ||
    readRawStringField(event.raw_payload, "root_id", "rootId", "thread_id", "threadId");

  return {
    channel: "wecom",
    workspace_id: event.corp_id,
    account_id: event.agent_id,
    thread_id: event.conversation_id,
    chat_type: event.conversation_type,
    topic_id: topicId || undefined,
    root_id: rootId || undefined,
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
      topic: topicId
        ? {
            kind: "topic",
            id: topicId,
          }
        : null,
      parent_peer: null,
      guild_id: null,
      team_id: null,
      member_role_ids: [],
      identity_links: [],
    },
    raw_payload: event.raw_payload,
  };
}
