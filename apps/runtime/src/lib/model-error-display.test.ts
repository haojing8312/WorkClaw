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

  test.each([
    ["context overflow", "prompt is too long: 277403 tokens > 200000 maximum", "context_overflow"],
    ["context overflow without is", "prompt too long: 277403 tokens > 200000 maximum", "context_overflow"],
    ["max tokens context overflow", "input length and `max_tokens` exceed context limit: 188059 + 20000 > 200000", "context_overflow"],
    ["invalid token budget", "max_tokens must be at least 1, got -1024", "invalid_token_budget"],
    ["compact invalid token budget", "max_tokens got -1024", "invalid_token_budget"],
    ["media too large", "image exceeds 5 MB maximum", "media_too_large"],
    ["payload too large", "payload too large", "media_too_large"],
    ["TPM rate limit", "413 tokens per minute limit exceeded", "rate_limit"],
    ["HTTP 400 context overflow with 429 token count", "HTTP 400: prompt is too long: 429 tokens > 200 maximum", "context_overflow"],
  ])("infers %s model errors from raw transport messages", (_label, raw, expected) => {
    expect(inferModelErrorKindFromMessage(raw)).toBe(expected);
  });

  test("maps invalid token budget to careful non-vision-specific copy", () => {
    expect(getModelErrorDisplay("invalid_token_budget")).toEqual(
      expect.objectContaining({
        kind: "invalid_token_budget",
        title: "模型输出空间不足",
        message: "模型请求没有剩余空间生成回复。请减少当前会话上下文、压缩图片，或使用更大上下文的模型后重试。",
      }),
    );
  });

  test("maps context overflow and media size errors to user-facing copy", () => {
    expect(getModelErrorDisplay("context_overflow")).toEqual(
      expect.objectContaining({
        title: "上下文过长",
        message: "当前会话内容超过了模型可处理的上下文。请减少历史内容、开启新会话，或使用更大上下文的模型。",
      }),
    );
    expect(getModelErrorDisplay("media_too_large")).toEqual(
      expect.objectContaining({
        title: "附件或图片过大",
        message: "上传的图片或附件超过了当前模型请求限制。请压缩图片、减少附件数量，或移除不必要的附件后重试。",
      }),
    );
  });
});
