import { describe, expect, it, vi } from "vitest";
import {
  detectCurrentFeishuPage,
  extractFeishuCredentials,
  getFeishuBrowserSetupSessionId,
  maybeReportFeishuCredentialsToExtension,
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
});
