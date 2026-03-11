import { detectFeishuPage } from "./feishu-detector";

export function detectCurrentFeishuPage(doc: Document = document) {
  return detectFeishuPage(doc);
}

export function extractFeishuCredentials(doc: Document = document): {
  appId: string;
  appSecret: string;
} | null {
  const appId = doc.querySelector("[data-field='app-id']")?.textContent?.trim() ?? "";
  const appSecret = doc.querySelector("[data-field='app-secret']")?.textContent?.trim() ?? "";

  if (!appId || !appSecret) {
    return null;
  }

  return {
    appId,
    appSecret,
  };
}
