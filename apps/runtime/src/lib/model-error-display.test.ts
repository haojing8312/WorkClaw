import { describe, expect, test } from "vitest";
import { getModelErrorDisplay, inferModelErrorKindFromMessage } from "./model-error-display";

describe("getModelErrorDisplay", () => {
  test("maps billing errors to the shared balance warning copy", () => {
    expect(getModelErrorDisplay("billing")).toEqual(
      expect.objectContaining({
        kind: "billing",
        title: "模型余额不足",
        message: "当前模型平台返回余额或额度不足，请到对应服务商控制台充值或检查套餐额度。",
      }),
    );
  });

  test("prefers structured backend copy when provided", () => {
    expect(
      getModelErrorDisplay({
        ok: false,
        kind: "auth",
        title: "鉴权失败",
        message: "请检查 API Key、组织权限或接口访问范围是否正确。",
        raw_message: "Unauthorized: invalid_api_key",
      }),
    ).toEqual(
      expect.objectContaining({
        kind: "auth",
        title: "鉴权失败",
        message: "请检查 API Key、组织权限或接口访问范围是否正确。",
        rawMessage: "Unauthorized: invalid_api_key",
      }),
    );
  });

  test("infers auth errors from raw provider JSON payloads", () => {
    const raw =
      '{"type":"error","error":{"type":"authentication_error","message":"login fail: Please carry the API secret key in the Authorization field of the request header"},"request_id":"060d83de3828d796eb11939cf30ed6b8"}';

    expect(inferModelErrorKindFromMessage(raw)).toBe("auth");
  });

  test.each([
    ["billing", '{"error":{"message":"insufficient_quota","code":"insufficient_quota"}}', "billing"],
    ["rate limit", '{"error":{"message":"429 Too Many Requests","type":"rate_limit_error"}}', "rate_limit"],
    [
      "minimax overloaded",
      '{"type":"error","error":{"type":"overloaded_error","message":"High traffic detected. For a more stable experience, upgrade to our Plus plan and use the highspeed model. (2064) (529)"}}',
      "rate_limit",
    ],
    ["timeout", "upstream request timed out after 30s", "timeout"],
    ["network", "error sending request for url (https://provider.example/v1/chat/completions)", "network"],
  ])("infers %s model errors from raw transport messages", (_label, raw, expected) => {
    expect(inferModelErrorKindFromMessage(raw)).toBe(expected);
  });
});
