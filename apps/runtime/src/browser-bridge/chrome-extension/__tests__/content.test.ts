import { describe, expect, it, vi } from "vitest";
import {
  detectCurrentFeishuPage,
  extractFeishuCredentials,
  getFeishuBrowserSetupSessionId,
  installFeishuCredentialReporter,
  installFeishuInstructionListener,
  maybeReportFeishuCredentialsToExtension,
  renderFeishuBrowserSetupInstruction,
} from "../content";

describe("chrome extension content helpers", () => {
  it("detects the current Feishu page", () => {
    document.body.innerHTML = `<div>凭证与基础信息</div><div>App ID</div>`;
    expect(detectCurrentFeishuPage(document).kind).toBe("credentials");
  });

  it("extracts App ID and App Secret from the credential page", () => {
    document.body.innerHTML = `
      <div>凭证与基础信息</div>
      <div data-field="app-id">cli_123</div>
      <div data-field="app-secret">sec_456</div>
    `;

    expect(extractFeishuCredentials(document)).toEqual({
      appId: "cli_123",
      appSecret: "sec_456",
    });
  });

  it("extracts credentials from label-and-value blocks", () => {
    document.body.innerHTML = `
      <section>
        <div class="field">
          <div class="label">App ID</div>
          <div class="value">cli_label_123</div>
        </div>
        <div class="field">
          <div class="label">App Secret</div>
          <div class="value">sec_label_456</div>
        </div>
      </section>
    `;

    expect(extractFeishuCredentials(document)).toEqual({
      appId: "cli_label_123",
      appSecret: "sec_label_456",
    });
  });

  it("extracts credentials from adjacent text when the values are not marked", () => {
    document.body.innerHTML = `
      <section>
        <div>凭证与基础信息</div>
        <div>App ID</div>
        <div>cli_adjacent_123</div>
        <div>App Secret</div>
        <div>sec_adjacent_456</div>
      </section>
    `;

    expect(extractFeishuCredentials(document)).toEqual({
      appId: "cli_adjacent_123",
      appSecret: "sec_adjacent_456",
    });
  });

  it("extracts credentials from readonly input fields on the credential page", () => {
    document.body.innerHTML = `
      <section>
        <div>凭证与基础信息</div>
        <div class="form-row">
          <label>App ID</label>
          <div class="value"><input readonly value="cli_input_123" /></div>
        </div>
        <div class="form-row">
          <label>App Secret</label>
          <div class="value"><input readonly value="sec_input_456" /></div>
        </div>
      </section>
    `;

    expect(extractFeishuCredentials(document)).toEqual({
      appId: "cli_input_123",
      appSecret: "sec_input_456",
    });
  });

  it("reads the setup session id from the current url", () => {
    expect(getFeishuBrowserSetupSessionId("https://open.feishu.cn/?workclaw_session_id=sess-url-1")).toBe(
      "sess-url-1",
    );
    expect(getFeishuBrowserSetupSessionId("https://open.feishu.cn/")).toBeNull();
  });

  it("reports extracted credentials to the extension runtime when session id is present", async () => {
    document.body.innerHTML = `
      <section>
        <div>凭证与基础信息</div>
        <div data-field="app-id">cli_message_123</div>
        <div data-field="app-secret">sec_message_456</div>
      </section>
    `;

    const sendMessage = vi.fn(async () => undefined);

    await expect(
      maybeReportFeishuCredentialsToExtension(
        {
          href: "https://open.feishu.cn/?workclaw_session_id=sess-message-1",
        } as Location,
        document,
        {
          runtime: {
            sendMessage,
          },
        },
      ),
    ).resolves.toBe(true);

    expect(sendMessage).toHaveBeenCalledWith({
      type: "workclaw.report-feishu-credentials",
      sessionId: "sess-message-1",
      appId: "cli_message_123",
      appSecret: "sec_message_456",
    });
  });

  it("watches for later credential page transitions and reports only once", async () => {
    document.body.innerHTML = `<div>飞书开放平台</div>`;

    const sendMessage = vi.fn(async () => undefined);
    const stop = installFeishuCredentialReporter(
      {
        href: "https://open.feishu.cn/?workclaw_session_id=sess-watch-1",
      } as Location,
      document,
      {
        runtime: {
          sendMessage,
        },
      },
    );

    document.body.innerHTML = `
      <section>
        <div>凭证与基础信息</div>
        <div class="form-row">
          <label>App ID</label>
          <div><input readonly value="cli_watch_123" /></div>
        </div>
        <div class="form-row">
          <label>App Secret</label>
          <div><input readonly value="sec_watch_456" /></div>
        </div>
      </section>
    `;

    await Promise.resolve();
    await Promise.resolve();

    expect(sendMessage).toHaveBeenCalledTimes(1);
    expect(sendMessage).toHaveBeenCalledWith({
      type: "workclaw.report-feishu-credentials",
      sessionId: "sess-watch-1",
      appId: "cli_watch_123",
      appSecret: "sec_watch_456",
    });

    document.body.appendChild(document.createElement("div"));
    await Promise.resolve();

    expect(sendMessage).toHaveBeenCalledTimes(1);

    stop();
  });

  it("renders a browser setup instruction banner and updates it in place", () => {
    renderFeishuBrowserSetupInstruction(document, {
      sessionId: "sess-banner-1",
      step: "ENABLE_LONG_CONNECTION",
      title: "本地绑定已完成",
      instruction: "请前往事件与回调，开启长连接接受事件。",
      ctaLabel: "继续到事件与回调",
    });

    expect(document.querySelector("[data-workclaw-feishu-setup-banner]")?.textContent).toContain(
      "本地绑定已完成",
    );
    expect(document.body.textContent).toContain("请前往事件与回调，开启长连接接受事件。");

    renderFeishuBrowserSetupInstruction(document, {
      sessionId: "sess-banner-1",
      step: "ENABLE_LONG_CONNECTION",
      title: "下一步",
      instruction: "请继续添加接受消息事件并发布版本。",
      ctaLabel: "继续配置",
    });

    expect(document.querySelectorAll("[data-workclaw-feishu-setup-banner]")).toHaveLength(1);
    expect(document.body.textContent).toContain("请继续添加接受消息事件并发布版本。");
  });

  it("registers a runtime listener that renders browser setup instructions", async () => {
    const listeners: Array<(message: unknown) => unknown> = [];
    const stop = installFeishuInstructionListener(document, {
      runtime: {
        onMessage: {
          addListener(listener: (message: unknown) => unknown) {
            listeners.push(listener);
          },
        },
      },
    } as never);

    expect(listeners).toHaveLength(1);

    listeners[0]?.({
      type: "workclaw.show-browser-setup-instruction",
      sessionId: "sess-banner-2",
      step: "ENABLE_LONG_CONNECTION",
      title: "本地绑定已完成",
      instruction: "请前往事件与回调，开启长连接接受事件。",
      ctaLabel: "继续到事件与回调",
    });

    expect(document.querySelector("[data-workclaw-feishu-setup-banner]")?.textContent).toContain(
      "本地绑定已完成",
    );

    stop();
  });
});
