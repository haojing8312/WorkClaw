import { createBridgeClient } from "../native-host/client";
import { type BridgeEnvelope, type BridgeResponse } from "../shared/protocol";
import { detectCurrentFeishuPage, extractFeishuCredentials } from "./content";

export function getNativeHostName(): string {
  return "dev.workclaw.runtime";
}

const INSTALL_HELLO_SESSION_ID = "browser-bridge-install";

type FetchLike = typeof fetch;
type ChromeLike = {
  runtime?: {
    connectNative?: (application: string) => {
      postMessage: (message: unknown) => void;
      disconnect: () => void;
      onMessage: {
        addListener: (listener: (message: unknown) => void) => void;
      };
    };
    onMessage?: {
      addListener: (listener: (message: unknown) => unknown) => void;
    };
  };
  tabs?: {
    query?: (queryInfo: { active: boolean; currentWindow: boolean }) => Promise<Array<{ id?: number }>>;
    sendMessage?: (tabId: number, message: unknown) => Promise<unknown> | unknown;
  };
};

type FeishuCredentialReportMessage = {
  type: "workclaw.report-feishu-credentials";
  sessionId: string;
  appId: string;
  appSecret: string;
};

type BrowserBridgeReadyMessage = {
  type: "workclaw.browser-bridge-ready";
};

export async function forwardCredentialsToLocalBridge(
  input: {
    sessionId: string;
    appId: string;
    appSecret: string;
  },
  baseUrl: string,
  fetchImpl: FetchLike = fetch,
): Promise<BridgeEnvelope<BridgeResponse>> {
  const client = createBridgeClient(baseUrl, fetchImpl);
  return client.send({
    version: 1,
    sessionId: input.sessionId,
    kind: "request",
    payload: {
      type: "credentials.report",
      appId: input.appId,
      appSecret: input.appSecret,
    },
  });
}

export async function sendBrowserBridgeHelloToLocalBridge(
  baseUrl: string,
  fetchImpl: FetchLike = fetch,
): Promise<BridgeEnvelope<BridgeResponse>> {
  const client = createBridgeClient(baseUrl, fetchImpl);
  return client.send({
    version: 1,
    sessionId: INSTALL_HELLO_SESSION_ID,
    kind: "request",
    payload: {
      type: "bridge.hello",
    },
  });
}

export function forwardCredentialsViaNativeHost(
  input: {
    sessionId: string;
    appId: string;
    appSecret: string;
  },
  chromeLike: ChromeLike = globalThis as ChromeLike,
): Promise<BridgeEnvelope<BridgeResponse>> {
  const port = chromeLike.runtime?.connectNative?.(getNativeHostName());
  if (!port) {
    return Promise.reject(new Error("chrome native host unavailable"));
  }

  return new Promise((resolve) => {
    port.onMessage.addListener((message: unknown) => {
      resolve(message as BridgeEnvelope<BridgeResponse>);
      port.disconnect();
    });

    port.postMessage({
      version: 1,
      sessionId: input.sessionId,
      kind: "request",
      payload: {
        type: "credentials.report",
        appId: input.appId,
        appSecret: input.appSecret,
      },
    });
  });
}

export function sendBrowserBridgeHelloViaNativeHost(
  chromeLike: ChromeLike = globalThis as ChromeLike,
): Promise<BridgeEnvelope<BridgeResponse>> {
  const port = chromeLike.runtime?.connectNative?.(getNativeHostName());
  if (!port) {
    return Promise.reject(new Error("chrome native host unavailable"));
  }

  return new Promise((resolve) => {
    port.onMessage.addListener((message: unknown) => {
      resolve(message as BridgeEnvelope<BridgeResponse>);
      port.disconnect();
    });

    port.postMessage({
      version: 1,
      sessionId: INSTALL_HELLO_SESSION_ID,
      kind: "request",
      payload: {
        type: "bridge.hello",
      },
    });
  });
}

export async function announceBrowserBridgeReady(
  baseUrl: string,
  dependencies: {
    sendViaNativeHost?: typeof sendBrowserBridgeHelloViaNativeHost;
    sendViaLocalBridge?: typeof sendBrowserBridgeHelloToLocalBridge;
  } = {},
): Promise<BridgeEnvelope<BridgeResponse>> {
  const sendViaNativeHost = dependencies.sendViaNativeHost ?? sendBrowserBridgeHelloViaNativeHost;
  const sendViaLocalBridge = dependencies.sendViaLocalBridge ?? sendBrowserBridgeHelloToLocalBridge;

  try {
    return await sendViaNativeHost();
  } catch {
    return sendViaLocalBridge(baseUrl);
  }
}

export async function reportFeishuCredentialsFromDocument(
  input: {
    sessionId: string;
    bridgeBaseUrl: string;
  },
  dependencies: {
    doc?: Document;
    sendViaNativeHost?: typeof forwardCredentialsViaNativeHost;
    sendViaLocalBridge?: typeof forwardCredentialsToLocalBridge;
  } = {},
): Promise<BridgeEnvelope<BridgeResponse>> {
  const doc = dependencies.doc ?? document;
  const page = detectCurrentFeishuPage(doc);
  if (page.kind !== "credentials") {
    throw new Error("current page is not the Feishu credential page");
  }

  const credentials = extractFeishuCredentials(doc);
  if (!credentials) {
    throw new Error("Feishu credentials are not available on the current page");
  }

  const payload = {
    sessionId: input.sessionId,
    appId: credentials.appId,
    appSecret: credentials.appSecret,
  };
  const sendViaNativeHost = dependencies.sendViaNativeHost ?? forwardCredentialsViaNativeHost;
  const sendViaLocalBridge = dependencies.sendViaLocalBridge ?? forwardCredentialsToLocalBridge;

  try {
    return await sendViaNativeHost(payload);
  } catch {
    return sendViaLocalBridge(payload, input.bridgeBaseUrl);
  }
}

export async function handleFeishuExtensionMessage(
  message: unknown,
  dependencies: {
    bridgeBaseUrl: string;
    sendViaNativeHost?: typeof forwardCredentialsViaNativeHost;
    sendViaLocalBridge?: typeof forwardCredentialsToLocalBridge;
    sendHelloViaNativeHost?: typeof sendBrowserBridgeHelloViaNativeHost;
    sendHelloViaLocalBridge?: typeof sendBrowserBridgeHelloToLocalBridge;
    broadcastInstruction?: typeof maybeBroadcastBridgeInstruction;
  },
): Promise<BridgeEnvelope<BridgeResponse> | null> {
  if (!isFeishuCredentialReportMessage(message)) {
    if (!isBrowserBridgeReadyMessage(message)) {
      return null;
    }
    return announceBrowserBridgeReady(dependencies.bridgeBaseUrl, {
      sendViaNativeHost: dependencies.sendHelloViaNativeHost,
      sendViaLocalBridge: dependencies.sendHelloViaLocalBridge,
    });
  }

  const sendViaNativeHost = dependencies.sendViaNativeHost ?? forwardCredentialsViaNativeHost;
  const sendViaLocalBridge = dependencies.sendViaLocalBridge ?? forwardCredentialsToLocalBridge;
  const payload = {
    sessionId: message.sessionId,
    appId: message.appId,
    appSecret: message.appSecret,
  };
  const broadcastInstruction = dependencies.broadcastInstruction ?? maybeBroadcastBridgeInstruction;

  try {
    const response = await sendViaNativeHost(payload);
    await broadcastInstruction(response);
    return response;
  } catch {
    const response = await sendViaLocalBridge(payload, dependencies.bridgeBaseUrl);
    await broadcastInstruction(response);
    return response;
  }
}

export async function maybeBroadcastBridgeInstruction(
  response: BridgeEnvelope<BridgeResponse>,
  chromeLike: ChromeLike = globalThis as ChromeLike,
): Promise<void> {
  if (response.kind !== "response" || response.payload.type !== "action.pause") {
    return;
  }
  if (!response.payload.step || !response.payload.title || !response.payload.instruction) {
    return;
  }

  const tabs = (await chromeLike.tabs?.query?.({
    active: true,
    currentWindow: true,
  })) ?? [];
  const tabId = tabs.find((tab) => typeof tab.id === "number")?.id;
  if (typeof tabId !== "number") {
    return;
  }

  await chromeLike.tabs?.sendMessage?.(tabId, {
    type: "workclaw.show-browser-setup-instruction",
    sessionId: response.sessionId,
    step: response.payload.step,
    title: response.payload.title,
    instruction: response.payload.instruction,
    ctaLabel: response.payload.ctaLabel,
  });
}

export function registerFeishuBackgroundMessageHandler(
  chromeLike: ChromeLike = globalThis as ChromeLike,
  dependencies: {
    bridgeBaseUrl: string;
    sendViaNativeHost?: typeof forwardCredentialsViaNativeHost;
    sendViaLocalBridge?: typeof forwardCredentialsToLocalBridge;
    sendHelloViaNativeHost?: typeof sendBrowserBridgeHelloViaNativeHost;
    sendHelloViaLocalBridge?: typeof sendBrowserBridgeHelloToLocalBridge;
  },
): void {
  chromeLike.runtime?.onMessage?.addListener((message: unknown) =>
    handleFeishuExtensionMessage(message, dependencies),
  );
}

function isFeishuCredentialReportMessage(message: unknown): message is FeishuCredentialReportMessage {
  if (typeof message !== "object" || message === null) {
    return false;
  }

  const candidate = message as Partial<FeishuCredentialReportMessage>;
  return (
    candidate.type === "workclaw.report-feishu-credentials" &&
    typeof candidate.sessionId === "string" &&
    typeof candidate.appId === "string" &&
    typeof candidate.appSecret === "string"
  );
}

function isBrowserBridgeReadyMessage(message: unknown): message is BrowserBridgeReadyMessage {
  if (typeof message !== "object" || message === null) {
    return false;
  }

  return (message as Partial<BrowserBridgeReadyMessage>).type === "workclaw.browser-bridge-ready";
}

registerFeishuBackgroundMessageHandler(globalThis as ChromeLike, {
  bridgeBaseUrl: "http://127.0.0.1:4312",
});

void announceBrowserBridgeReady("http://127.0.0.1:4312").catch(() => {
  // Ignore startup handshake failures until the local bridge is installed.
});
