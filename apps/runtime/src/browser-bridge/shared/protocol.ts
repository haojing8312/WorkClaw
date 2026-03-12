export type BridgeRequest =
  | { type: "bridge.hello" }
  | { type: "session.start"; provider: "feishu" }
  | { type: "session.resume"; sessionId: string }
  | { type: "page.report"; page: string }
  | { type: "credentials.report"; appId: string; appSecret: string };

export type BridgeResponse =
  | { type: "action.open"; url: string }
  | { type: "action.detect_step" }
  | { type: "action.collect_credentials" }
  | {
      type: "action.pause";
      reason: string;
      step?: string;
      title?: string;
      instruction?: string;
      ctaLabel?: string;
    };

export interface BridgeEnvelope<T> {
  version: 1;
  sessionId: string;
  kind: "request" | "response" | "event";
  payload: T;
}

export function isBridgeEnvelope(value: unknown): value is BridgeEnvelope<unknown> {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<BridgeEnvelope<unknown>>;
  return (
    candidate.version === 1 &&
    typeof candidate.sessionId === "string" &&
    (candidate.kind === "request" || candidate.kind === "response" || candidate.kind === "event") &&
    "payload" in candidate
  );
}
