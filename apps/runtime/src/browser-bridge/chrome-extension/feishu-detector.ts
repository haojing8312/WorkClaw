export interface FeishuDetectedPage {
  kind: "login" | "credentials" | "unknown";
  confidence: number;
}

export function detectFeishuPage(doc: Document): FeishuDetectedPage {
  const text = doc.body?.textContent ?? "";
  const hasStructuredCredentialFields =
    Boolean(doc.querySelector("[data-field='app-id']")) &&
    Boolean(doc.querySelector("[data-field='app-secret']"));
  if (text.includes("登录")) {
    return { kind: "login", confidence: 0.9 };
  }
  if (
    (text.includes("凭证与基础信息") && text.includes("App ID")) ||
    (text.includes("凭证与基础信息") && hasStructuredCredentialFields)
  ) {
    return { kind: "credentials", confidence: 0.9 };
  }
  return { kind: "unknown", confidence: 0.1 };
}
