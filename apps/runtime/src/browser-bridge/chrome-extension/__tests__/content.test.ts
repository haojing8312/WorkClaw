import { describe, expect, it } from "vitest";
import { detectCurrentFeishuPage, extractFeishuCredentials } from "../content";

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
});
