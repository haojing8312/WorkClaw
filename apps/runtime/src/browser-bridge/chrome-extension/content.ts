import { detectFeishuPage } from "./feishu-detector";

type BrowserInstructionMessage = {
  type: "workclaw.show-browser-setup-instruction";
  sessionId: string;
  step: string;
  title: string;
  instruction: string;
  ctaLabel?: string;
};

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
    onMessage?: {
      addListener?: (listener: (message: unknown) => unknown) => void;
    };
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

export function installFeishuCredentialReporter(
  locationLike: Pick<Location, "href"> = window.location,
  doc: Document = document,
  chromeLike: ChromeLike = globalThis as ChromeLike,
): () => void {
  const sessionId = getFeishuBrowserSetupSessionId(locationLike.href);
  if (!sessionId) {
    return () => {};
  }

  let disposed = false;
  let reported = false;
  const tryReport = async () => {
    if (disposed || reported) {
      return;
    }

    reported = await maybeReportFeishuCredentialsToExtension(locationLike, doc, chromeLike);
    if (reported) {
      observer.disconnect();
    }
  };

  const observer = new MutationObserver(() => {
    void tryReport();
  });
  const root = doc.documentElement ?? doc.body;
  if (root) {
    observer.observe(root, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: true,
    });
  }

  void tryReport();

  return () => {
    disposed = true;
    observer.disconnect();
  };
}

export async function initializeFeishuContentScript(
  locationLike: Pick<Location, "href"> = window.location,
  doc: Document = document,
  chromeLike: ChromeLike = globalThis as ChromeLike,
): Promise<boolean> {
  installFeishuCredentialReporter(locationLike, doc, chromeLike);
  installFeishuInstructionListener(doc, chromeLike);
  return false;
}

export function renderFeishuBrowserSetupInstruction(
  doc: Document,
  instruction: {
    sessionId: string;
    step: string;
    title: string;
    instruction: string;
    ctaLabel?: string;
  },
): HTMLElement {
  const existing = doc.querySelector<HTMLElement>("[data-workclaw-feishu-setup-banner]");
  const banner = existing ?? doc.createElement("aside");
  banner.setAttribute("data-workclaw-feishu-setup-banner", "true");
  banner.setAttribute("data-session-id", instruction.sessionId);
  banner.setAttribute("data-step", instruction.step);
  banner.style.position = "fixed";
  banner.style.top = "16px";
  banner.style.right = "16px";
  banner.style.zIndex = "2147483647";
  banner.style.maxWidth = "360px";
  banner.style.padding = "14px 16px";
  banner.style.borderRadius = "12px";
  banner.style.border = "1px solid #c7d2fe";
  banner.style.background = "#eef2ff";
  banner.style.boxShadow = "0 12px 32px rgba(15, 23, 42, 0.18)";
  banner.style.color = "#312e81";
  banner.innerHTML = `
    <div style="font-size:12px;font-weight:700;letter-spacing:0.02em;text-transform:uppercase;color:#4338ca;">飞书配置向导</div>
    <div style="margin-top:6px;font-size:15px;font-weight:600;color:#1e1b4b;">${escapeHtml(instruction.title)}</div>
    <div style="margin-top:8px;font-size:13px;line-height:1.5;color:#312e81;">${escapeHtml(instruction.instruction)}</div>
    ${
      instruction.ctaLabel
        ? `<div style="margin-top:10px;font-size:12px;font-weight:600;color:#4338ca;">${escapeHtml(instruction.ctaLabel)}</div>`
        : ""
    }
  `;

  if (!existing) {
    (doc.body ?? doc.documentElement)?.appendChild(banner);
  }

  return banner;
}

export function installFeishuInstructionListener(
  doc: Document = document,
  chromeLike: ChromeLike = globalThis as ChromeLike,
): () => void {
  const listener = (message: unknown) => {
    if (!isBrowserInstructionMessage(message)) {
      return false;
    }
    renderFeishuBrowserSetupInstruction(doc, message);
    return true;
  };

  chromeLike.runtime?.onMessage?.addListener?.(listener);
  return () => {};
}

function findValueNearLabel(doc: Document, label: string): string {
  const elements = Array.from(doc.querySelectorAll("div, span, p, td, dt, dd, label"));
  const labelElement = elements
    .filter((element) => normalizeText(element.textContent) === label)
    .sort((left, right) => {
      const depthDifference = getElementDepth(right) - getElementDepth(left);
      if (depthDifference !== 0) {
        return depthDifference;
      }
      return left.children.length - right.children.length;
    })[0];
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

function getElementDepth(element: Element): number {
  let depth = 0;
  let current = element.parentElement;
  while (current) {
    depth += 1;
    current = current.parentElement;
  }
  return depth;
}

function firstMeaningfulText(element: Element, label: string): string {
  const fieldValue = readFieldValue(element);
  if (fieldValue && fieldValue !== label) {
    return fieldValue;
  }

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

function readFieldValue(element: Element): string {
  if ("value" in element && typeof element.value === "string") {
    return normalizeText(element.value);
  }

  const field = element.querySelector("input, textarea");
  if (field && "value" in field && typeof field.value === "string") {
    return normalizeText(field.value);
  }

  return "";
}

function isBrowserInstructionMessage(message: unknown): message is BrowserInstructionMessage {
  if (!message || typeof message !== "object") {
    return false;
  }
  const candidate = message as Partial<BrowserInstructionMessage>;
  return (
    candidate.type === "workclaw.show-browser-setup-instruction" &&
    typeof candidate.sessionId === "string" &&
    typeof candidate.step === "string" &&
    typeof candidate.title === "string" &&
    typeof candidate.instruction === "string"
  );
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (char) => {
    switch (char) {
      case "&":
        return "&amp;";
      case "<":
        return "&lt;";
      case ">":
        return "&gt;";
      case '"':
        return "&quot;";
      case "'":
        return "&#39;";
      default:
        return char;
    }
  });
}

void initializeFeishuContentScript().catch(() => {
  // Ignore runtime bridge failures in the passive content script path.
});
