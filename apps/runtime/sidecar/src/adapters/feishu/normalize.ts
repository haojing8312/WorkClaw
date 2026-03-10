import type { NormalizedImEvent } from "../types.js";
import type { FeishuWsEventRecord } from "../../feishu_ws.js";

function readWorkspaceId(raw: unknown): string {
  if (!raw || typeof raw !== "object") {
    return "";
  }
  const tenantKey = (raw as Record<string, unknown>).tenant_key;
  return typeof tenantKey === "string" ? tenantKey : "";
}

export function normalizeFeishuEvent(event: FeishuWsEventRecord): NormalizedImEvent {
  const workspaceId = readWorkspaceId(event.raw);
  return {
    channel: "feishu",
    workspace_id: workspaceId,
    account_id: workspaceId,
    thread_id: event.chat_id,
    message_id: event.message_id,
    sender_id: event.sender_open_id,
    sender_name: null,
    text: event.text,
    mentions: (event.mention_open_ids || [])
      .filter((id) => typeof id === "string" && id.trim().length > 0)
      .map((id) => ({
        type: "user",
        id,
      })),
    raw_event_type: event.event_type,
    occurred_at: event.received_at,
    reply_target: event.message_id || null,
    routing_context: {
      peer: {
        kind: "group",
        id: event.chat_id,
      },
      parent_peer: null,
      guild_id: null,
      team_id: null,
      member_role_ids: [],
      identity_links: [],
    },
    raw_payload: event.raw,
  };
}
