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

function collectErrorStrings(value: unknown, out: string[]) {
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (trimmed) {
      out.push(trimmed);
    }
    return;
  }

  if (Array.isArray(value)) {
    value.forEach((item) => collectErrorStrings(item, out));
    return;
  }

  if (!value || typeof value !== "object") {
    return;
  }

  const record = value as Record<string, unknown>;
  ["message", "code", "type", "error", "detail"].forEach((key) => {
    if (key in record) {
      collectErrorStrings(record[key], out);
    }
  });
  Object.values(record).forEach((item) => collectErrorStrings(item, out));
}

function normalizeErrorSearchText(rawMessage: string): string {
  try {
    const parsed = JSON.parse(rawMessage) as unknown;
    const parts: string[] = [];
    collectErrorStrings(parsed, parts);
    if (parts.length > 0) {
      return parts.join(" ").toLowerCase();
    }
  } catch {
    // Fall back to plain-text matching when the message is not JSON.
  }
  return rawMessage.toLowerCase();
}

export function inferModelErrorKindFromMessage(rawMessage: string): ModelErrorKind | null {
  const lower = normalizeErrorSearchText(rawMessage);

  if (
    lower.includes("insufficient_balance") ||
    lower.includes("insufficient balance") ||
    lower.includes("balance too low") ||
    lower.includes("account balance too low") ||
    lower.includes("insufficient_quota") ||
    lower.includes("insufficient quota") ||
    lower.includes("billing") ||
    lower.includes("payment required") ||
    lower.includes("credit balance") ||
    lower.includes("余额不足") ||
    lower.includes("欠费")
  ) {
    return "billing";
  }

  if (
    lower.includes("api key") ||
    lower.includes("unauthorized") ||
    lower.includes("invalid_api_key") ||
    lower.includes("authentication") ||
    lower.includes("permission denied") ||
    lower.includes("forbidden")
  ) {
    return "auth";
  }

  if (
    lower.includes("rate limit") ||
    lower.includes("too many requests") ||
    lower.includes("429") ||
    lower.includes("529") ||
    lower.includes("overloaded_error") ||
    lower.includes("high traffic detected") ||
    lower.includes("quota")
  ) {
    return "rate_limit";
  }

  if (lower.includes("timeout") || lower.includes("timed out") || lower.includes("deadline")) {
    return "timeout";
  }

  if (
    lower.includes("connection") ||
    lower.includes("network") ||
    lower.includes("dns") ||
    lower.includes("connect") ||
    lower.includes("socket") ||
    lower.includes("decoding response body") ||
    lower.includes("decode response body") ||
    lower.includes("error decoding response body") ||
    lower.includes("error sending request for url") ||
    lower.includes("sending request for url")
  ) {
    return "network";
  }

  return null;
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
