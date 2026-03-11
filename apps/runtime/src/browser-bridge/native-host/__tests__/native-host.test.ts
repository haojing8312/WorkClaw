import { describe, expect, it } from "vitest";
import { createBridgeClient } from "../client";
import { decodeNativeMessage, encodeNativeMessage } from "../index";

describe("native messaging framing", () => {
  it("round-trips a bridge envelope", () => {
    const encoded = encodeNativeMessage({
      version: 1,
      sessionId: "sess-1",
      kind: "request",
      payload: { type: "session.start", provider: "feishu" },
    });

    expect(decodeNativeMessage(encoded)).toMatchObject({
      sessionId: "sess-1",
      kind: "request",
      payload: { type: "session.start", provider: "feishu" },
    });
  });

  it("posts the decoded envelope to the local bridge endpoint", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = [];
    const client = createBridgeClient("http://127.0.0.1:4312", async (url, init) => {
      calls.push({ url: String(url), init });
      return new Response(
        JSON.stringify({
          version: 1,
          sessionId: "sess-1",
          kind: "response",
          payload: { type: "action.detect_step" },
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      );
    });

    const response = await client.send({
      version: 1,
      sessionId: "sess-1",
      kind: "request",
      payload: { type: "session.start", provider: "feishu" },
    });

    expect(calls).toHaveLength(1);
    expect(calls[0]?.url).toBe("http://127.0.0.1:4312/api/browser-bridge/native-message");
    expect(response).toMatchObject({
      kind: "response",
      payload: { type: "action.detect_step" },
    });
  });
});
