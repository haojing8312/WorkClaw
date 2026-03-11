import { describe, expect, it, vi } from "vitest";
import {
  forwardCredentialsToLocalBridge,
  forwardCredentialsViaNativeHost,
  getNativeHostName,
} from "../background";

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

  it("forwards credentials via chrome.runtime.connectNative when available", async () => {
    const postMessage = vi.fn();
    const disconnect = vi.fn();
    const listeners: Array<(message: unknown) => void> = [];

    const chromeLike = {
      runtime: {
        connectNative: vi.fn(() => ({
          postMessage,
          disconnect,
          onMessage: {
            addListener(listener: (message: unknown) => void) {
              listeners.push(listener);
            },
          },
        })),
      },
    };

    const pending = forwardCredentialsViaNativeHost(
      {
        sessionId: "sess-2",
        appId: "cli_native_123",
        appSecret: "sec_native_456",
      },
      chromeLike,
    );

    expect(chromeLike.runtime.connectNative).toHaveBeenCalledWith("dev.workclaw.runtime");
    expect(postMessage).toHaveBeenCalledWith({
      version: 1,
      sessionId: "sess-2",
      kind: "request",
      payload: {
        type: "credentials.report",
        appId: "cli_native_123",
        appSecret: "sec_native_456",
      },
    });

    listeners[0]?.({
      version: 1,
      sessionId: "sess-2",
      kind: "response",
      payload: { type: "action.pause", reason: "saved" },
    });

    await expect(pending).resolves.toMatchObject({
      kind: "response",
      payload: { type: "action.pause", reason: "saved" },
    });
    expect(disconnect).toHaveBeenCalled();
  });
});
