import { detectFeishuPage } from "./feishu-detector";

export function detectCurrentFeishuPage(doc: Document = document) {
  return detectFeishuPage(doc);
}

export function extractFeishuCredentials(doc: Document = document): {
  appId: string;
  appSecret: string;
} | null {
  const appId =
    doc.querySelector("[data-field='app-id']")?.textContent?.trim() ??
    findValueNearLabel(doc, "App ID");
  const appSecret =
    doc.querySelector("[data-field='app-secret']")?.textContent?.trim() ??
    findValueNearLabel(doc, "App Secret");

  if (!appId || !appSecret) {
    return null;
  }

  return {
    appId,
    appSecret,
  };
}

type ChromeLike = {
  runtime?: {
    sendMessage?: (message: unknown) => Promise<unknown> | unknown;
  };
};

export function getFeishuBrowserSetupSessionId(href: string = window.location.href): string | null {
  const url = new URL(href);
  return url.searchParams.get("workclaw_session_id");
}

export async function maybeReportFeishuCredentialsToExtension(
  locationLike: Pick<Location, "href"> = window.location,
  doc: Document = document,
  chromeLike: ChromeLike = globalThis as ChromeLike,
): Promise<boolean> {
  const sessionId = getFeishuBrowserSetupSessionId(locationLike.href);
  if (!sessionId) {
    return false;
  }

  if (detectCurrentFeishuPage(doc).kind !== "credentials") {
    return false;
  }

  const credentials = extractFeishuCredentials(doc);
  if (!credentials) {
    return false;
  }

  await chromeLike.runtime?.sendMessage?.({
    type: "workclaw.report-feishu-credentials",
    sessionId,
    appId: credentials.appId,
    appSecret: credentials.appSecret,
  });

  return true;
}

export async function initializeFeishuContentScript(
  locationLike: Pick<Location, "href"> = window.location,
  doc: Document = document,
  chromeLike: ChromeLike = globalThis as ChromeLike,
): Promise<boolean> {
  return maybeReportFeishuCredentialsToExtension(locationLike, doc, chromeLike);
}

function findValueNearLabel(doc: Document, label: string): string {
  const elements = Array.from(doc.querySelectorAll("div, span, p, td, dt, dd, label"));
  const labelElement = elements.find((element) => normalizeText(element.textContent) === label);
  if (!labelElement) {
    return "";
  }

  let sibling = labelElement.nextElementSibling;
  while (sibling) {
    const text = firstMeaningfulText(sibling, label);
    if (text && text !== label) {
      return text;
    }
    sibling = sibling.nextElementSibling;
  }

  const parentValue = findValueInParentBlock(labelElement, label);
  if (parentValue) {
    return parentValue;
  }

  return "";
}

function findValueInParentBlock(labelElement: Element, label: string): string {
  const parent = labelElement.parentElement;
  if (!parent) {
    return "";
  }

  const children = Array.from(parent.children);
  const labelIndex = children.indexOf(labelElement);
  for (let index = labelIndex + 1; index < children.length; index += 1) {
    const text = firstMeaningfulText(children[index] as Element, label);
    if (text && text !== label) {
      return text;
    }
  }

  return "";
}

function normalizeText(value: string | null | undefined): string {
  return (value ?? "").replace(/\s+/g, " ").trim();
}

function firstMeaningfulText(element: Element, label: string): string {
  const ownText = normalizeText(element.textContent);
  if (ownText && ownText !== label && ownText !== "凭证与基础信息") {
    return ownText;
  }

  const descendants = Array.from(element.querySelectorAll("div, span, p, td, dt, dd, label"));
  for (const descendant of descendants) {
    const text = normalizeText(descendant.textContent);
    if (text && text !== label && text !== "凭证与基础信息") {
      return text;
    }
  }

  return "";
}

void initializeFeishuContentScript().catch(() => {
  // Ignore runtime bridge failures in the passive content script path.
});
