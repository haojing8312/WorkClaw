import type { ModelConnectionTestResult, ModelErrorKind } from "../types";

type ModelErrorDisplayCopy = {
  title: string;
  message: string;
};

const MODEL_ERROR_DISPLAY_COPY: Record<ModelErrorKind, ModelErrorDisplayCopy> = {
  billing: {
    title: "模型余额不足",
    message: "当前模型平台返回余额或额度不足，请到对应服务商控制台充值或检查套餐额度。",
  },
  auth: {
    title: "鉴权失败",
    message: "请检查 API Key、组织权限或接口访问范围是否正确。",
  },
  rate_limit: {
    title: "请求过于频繁",
    message: "模型平台当前触发限流，请稍后重试或降低并发频率。",
  },
  timeout: {
    title: "请求超时",
    message: "模型平台响应超时，请稍后重试，或检查网络和所选模型是否可用。",
  },
  network: {
    title: "网络连接失败",
    message: "无法连接到模型接口，请检查 Base URL、网络环境或代理配置。",
  },
  unknown: {
    title: "连接失败",
    message: "模型平台返回了未识别错误，可查看详细信息进一步排查。",
  },
};

export function isModelErrorKind(value: unknown): value is ModelErrorKind {
  return (
    value === "billing" ||
    value === "auth" ||
    value === "rate_limit" ||
    value === "timeout" ||
    value === "network" ||
    value === "unknown"
  );
}

export function getModelErrorDisplay(
  input: ModelErrorKind | ModelConnectionTestResult,
) {
  const kind = isModelErrorKind(typeof input === "string" ? input : input.kind) ? (typeof input === "string" ? input : input.kind) : "unknown";
  const fallback = MODEL_ERROR_DISPLAY_COPY[kind];

  if (typeof input === "string") {
    return {
      kind,
      title: fallback.title,
      message: fallback.message,
      rawMessage: null,
    };
  }

  return {
    kind,
    title: input.title?.trim() || fallback.title,
    message: input.message?.trim() || fallback.message,
    rawMessage: input.raw_message?.trim() || null,
  };
}
