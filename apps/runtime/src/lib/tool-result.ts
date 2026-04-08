export interface ParsedToolResult {
  ok?: boolean;
  tool?: string;
  summary?: string;
  error_code?: string;
  error_message?: string;
  details?: Record<string, unknown>;
  data?: Record<string, unknown>;
  error?: {
    code?: string;
    message?: string;
  };
  artifacts?: unknown[];
}

export function parseStructuredToolResult(output?: string | null): ParsedToolResult | null {
  if (!output) return null;
  const trimmed = output.trim();
  if (!trimmed.startsWith("{")) return null;

  try {
    const parsed = JSON.parse(trimmed) as ParsedToolResult;
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      ("summary" in parsed ||
        "details" in parsed ||
        "error_code" in parsed ||
        "data" in parsed ||
        "error" in parsed ||
        "artifacts" in parsed)
    ) {
      return parsed;
    }
  } catch {
    return null;
  }

  return null;
}

export function getToolResultSummary(output?: string | null): string {
  const parsed = parseStructuredToolResult(output);
  return typeof parsed?.summary === "string" ? parsed.summary : String(output || "");
}

export function getToolResultErrorText(output?: string | null): string {
  const parsed = parseStructuredToolResult(output);
  if (typeof parsed?.error?.message === "string" && parsed.error.message.trim()) {
    return parsed.error.message;
  }
  if (typeof parsed?.error_message === "string" && parsed.error_message.trim()) {
    return parsed.error_message;
  }
  if (typeof parsed?.summary === "string" && parsed.summary.trim()) {
    return parsed.summary;
  }
  return String(output || "");
}

export function getToolResultDetailString(
  output: string | undefined | null,
  key: string,
): string {
  const parsed = parseStructuredToolResult(output);
  const value = parsed?.details?.[key] ?? parsed?.data?.[key];
  return typeof value === "string" ? value : "";
}

export function getToolResultDetails(
  output?: string | null,
): Record<string, unknown> | null {
  const parsed = parseStructuredToolResult(output);
  if (parsed?.details && typeof parsed.details === "object") {
    return parsed.details;
  }
  if (parsed?.data && typeof parsed.data === "object") {
    return parsed.data;
  }
  return null;
}
