import { describe, expect, test } from "vitest";
import { getModelErrorDisplay } from "./model-error-display";

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
});
