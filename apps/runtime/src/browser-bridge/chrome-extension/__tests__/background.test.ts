import { describe, expect, it, vi } from "vitest";
import {
  forwardCredentialsToLocalBridge,
  forwardCredentialsViaNativeHost,
  getNativeHostName,
  handleFeishuExtensionMessage,
  maybeBroadcastBridgeInstruction,
  registerFeishuBackgroundMessageHandler,
  reportFeishuCredentialsFromDocument,
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

  it("detects and reports credentials from the current page via native host", async () => {
    document.body.innerHTML = `
      <section>
        <div>凭证与基础信息</div>
        <div>App ID</div>
        <div>cli_report_123</div>
        <div>App Secret</div>
        <div>sec_report_456</div>
      </section>
    `;

    const sendViaNativeHost = vi.fn(async () => ({
      version: 1 as const,
      sessionId: "sess-3",
      kind: "response" as const,
      payload: { type: "action.pause" as const, reason: "saved" },
    }));
    const sendViaLocalBridge = vi.fn();

    const response = await reportFeishuCredentialsFromDocument(
      {
        sessionId: "sess-3",
        bridgeBaseUrl: "http://127.0.0.1:4312",
      },
      {
        doc: document,
        sendViaNativeHost,
        sendViaLocalBridge,
      },
    );

    expect(sendViaNativeHost).toHaveBeenCalledWith({
      sessionId: "sess-3",
      appId: "cli_report_123",
      appSecret: "sec_report_456",
    });
    expect(sendViaLocalBridge).not.toHaveBeenCalled();
    expect(response).toMatchObject({
      kind: "response",
      payload: { type: "action.pause", reason: "saved" },
    });
  });

  it("falls back to the local bridge when native host is unavailable", async () => {
    document.body.innerHTML = `
      <section>
        <div>凭证与基础信息</div>
        <div data-field="app-id">cli_fallback_123</div>
        <div data-field="app-secret">sec_fallback_456</div>
      </section>
    `;

    const sendViaNativeHost = vi.fn(async () => {
      throw new Error("native host unavailable");
    });
    const sendViaLocalBridge = vi.fn(async () => ({
      version: 1 as const,
      sessionId: "sess-4",
      kind: "response" as const,
      payload: { type: "action.pause" as const, reason: "saved-via-http" },
    }));

    const response = await reportFeishuCredentialsFromDocument(
      {
        sessionId: "sess-4",
        bridgeBaseUrl: "http://127.0.0.1:4312",
      },
      {
        doc: document,
        sendViaNativeHost,
        sendViaLocalBridge,
      },
    );

    expect(sendViaNativeHost).toHaveBeenCalledTimes(1);
    expect(sendViaLocalBridge).toHaveBeenCalledWith(
      {
        sessionId: "sess-4",
        appId: "cli_fallback_123",
        appSecret: "sec_fallback_456",
      },
      "http://127.0.0.1:4312",
    );
    expect(response).toMatchObject({
      kind: "response",
      payload: { type: "action.pause", reason: "saved-via-http" },
    });
  });

  it("handles credential report messages from the content script", async () => {
    const sendViaNativeHost = vi.fn(async () => ({
      version: 1 as const,
      sessionId: "sess-5",
      kind: "response" as const,
      payload: {
        type: "action.pause" as const,
        reason: "saved-from-message",
        step: "ENABLE_LONG_CONNECTION",
        title: "本地绑定已完成",
        instruction: "请前往事件与回调，开启长连接接受事件。",
      },
    }));
    const broadcastInstruction = vi.fn();

    const response = await handleFeishuExtensionMessage(
      {
        type: "workclaw.report-feishu-credentials",
        sessionId: "sess-5",
        appId: "cli_msg_123",
        appSecret: "sec_msg_456",
      },
      {
        bridgeBaseUrl: "http://127.0.0.1:4312",
        sendViaNativeHost,
        broadcastInstruction,
      },
    );

    expect(sendViaNativeHost).toHaveBeenCalledWith({
      sessionId: "sess-5",
      appId: "cli_msg_123",
      appSecret: "sec_msg_456",
    });
    expect(response).toMatchObject({
      kind: "response",
      payload: {
        type: "action.pause",
        reason: "saved-from-message",
        step: "ENABLE_LONG_CONNECTION",
      },
    });
    expect(broadcastInstruction).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: "sess-5",
        kind: "response",
        payload: expect.objectContaining({
          type: "action.pause",
          step: "ENABLE_LONG_CONNECTION",
          title: "本地绑定已完成",
        }),
      }),
    );
  });

  it("registers a runtime message listener that handles credential reports", async () => {
    const listeners: Array<(message: unknown) => unknown> = [];
    const chromeLike = {
      runtime: {
        onMessage: {
          addListener(listener: (message: unknown) => unknown) {
            listeners.push(listener);
          },
        },
      },
    };
    const sendViaNativeHost = vi.fn(async () => ({
      version: 1 as const,
      sessionId: "sess-6",
      kind: "response" as const,
      payload: { type: "action.pause" as const, reason: "saved-from-listener" },
    }));

    registerFeishuBackgroundMessageHandler(chromeLike, {
      bridgeBaseUrl: "http://127.0.0.1:4312",
      sendViaNativeHost,
    });

    expect(listeners).toHaveLength(1);

    await expect(
      listeners[0]?.({
        type: "workclaw.report-feishu-credentials",
        sessionId: "sess-6",
        appId: "cli_listener_123",
        appSecret: "sec_listener_456",
      }),
    ).resolves.toMatchObject({
      kind: "response",
      payload: { type: "action.pause", reason: "saved-from-listener" },
    });
  });

  it("broadcasts pause instructions to the active Feishu tab", async () => {
    const query = vi.fn(async () => [{ id: 11 }]);
    const sendMessage = vi.fn(async () => undefined);

    await maybeBroadcastBridgeInstruction(
      {
        version: 1,
        sessionId: "sess-7",
        kind: "response",
        payload: {
          type: "action.pause",
          reason: "browser bridge credentials bound locally",
          step: "ENABLE_LONG_CONNECTION",
          title: "本地绑定已完成",
          instruction: "请前往事件与回调，开启长连接接受事件。",
          ctaLabel: "继续到事件与回调",
        },
      },
      {
        tabs: {
          query,
          sendMessage,
        },
      } as never,
    );

    expect(query).toHaveBeenCalledWith({
      active: true,
      currentWindow: true,
    });
    expect(sendMessage).toHaveBeenCalledWith(11, {
      type: "workclaw.show-browser-setup-instruction",
      sessionId: "sess-7",
      step: "ENABLE_LONG_CONNECTION",
      title: "本地绑定已完成",
      instruction: "请前往事件与回调，开启长连接接受事件。",
      ctaLabel: "继续到事件与回调",
    });
  });
});
