import { describe, expect, it } from "vitest";
import { forwardCredentialsToLocalBridge, getNativeHostName } from "../background";

describe("chrome extension background bridge", () => {
  it("exposes the expected native host name", () => {
    expect(getNativeHostName()).toBe("dev.workclaw.runtime");
  });

  it("forwards credentials to the local browser bridge endpoint", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = [];

    const response = await forwardCredentialsToLocalBridge(
      {
        sessionId: "sess-1",
        appId: "cli_123",
        appSecret: "sec_456",
      },
      "http://127.0.0.1:4312",
      async (url, init) => {
        calls.push({ url: String(url), init });
        return new Response(
          JSON.stringify({
            version: 1,
            sessionId: "sess-1",
            kind: "response",
            payload: { type: "action.pause", reason: "saved" },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        );
      },
    );

    expect(calls).toHaveLength(1);
    expect(calls[0]?.url).toBe("http://127.0.0.1:4312/api/browser-bridge/native-message");
    expect(response).toMatchObject({
      kind: "response",
      payload: { type: "action.pause", reason: "saved" },
    });
  });
});
