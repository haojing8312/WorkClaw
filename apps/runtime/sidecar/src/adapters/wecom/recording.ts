import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type { NormalizedImEvent } from "../types.js";
import type { WecomInboundEvent } from "./types.js";

const WECOM_CAPTURE_DIR_ENV = "WORKCLAW_WECOM_CAPTURE_DIR";

function sanitizePathSegment(value: string): string {
  const trimmed = String(value || "").trim();
  if (!trimmed) {
    return "unknown";
  }

  return trimmed.replace(/[^a-zA-Z0-9._-]+/g, "_").slice(0, 80) || "unknown";
}

export function resolveWecomCaptureDir(
  env: NodeJS.ProcessEnv = process.env,
): string | null {
  const value = env[WECOM_CAPTURE_DIR_ENV];
  if (typeof value !== "string") {
    return null;
  }

  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

export function buildWecomCaptureFileName(event: WecomInboundEvent): string {
  const occurredAt = sanitizePathSegment(
    event.occurred_at || new Date().toISOString(),
  );
  const conversationId = sanitizePathSegment(event.conversation_id);
  const messageId = sanitizePathSegment(event.message_id);
  return `${occurredAt}__${conversationId}__${messageId}.json`;
}

export async function recordWecomInboundSample(
  event: WecomInboundEvent,
  normalizedEvent: NormalizedImEvent,
  env: NodeJS.ProcessEnv = process.env,
): Promise<string | null> {
  const captureDir = resolveWecomCaptureDir(env);
  if (!captureDir) {
    return null;
  }

  await mkdir(captureDir, { recursive: true });
  const filePath = join(captureDir, buildWecomCaptureFileName(event));
  const payload = JSON.stringify(
    {
      captured_at: new Date().toISOString(),
      source: "wecom-adapter",
      inbound_event: event,
      normalized_event: normalizedEvent,
    },
    null,
    2,
  );
  await writeFile(filePath, payload, "utf8");
  return filePath;
}
