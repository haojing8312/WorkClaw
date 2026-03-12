import { describe, expect, it } from "vitest";
import {
  isBridgeEnvelope,
  type BridgeEnvelope,
  type BridgeRequest,
} from "../protocol";

describe("browser bridge protocol", () => {
  it("accepts valid request envelopes", () => {
    const msg: BridgeEnvelope<BridgeRequest> = {
      version: 1,
      sessionId: "sess-1",
      kind: "request",
      payload: { type: "session.start", provider: "feishu" },
    };

    expect(isBridgeEnvelope(msg)).toBe(true);
  });

  it("rejects envelopes without version/session metadata", () => {
    expect(isBridgeEnvelope({ kind: "request" })).toBe(false);
  });
});
