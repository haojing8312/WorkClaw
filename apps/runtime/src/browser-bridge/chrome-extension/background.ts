import { createBridgeClient } from "../native-host/client";
import { type BridgeEnvelope, type BridgeResponse } from "../shared/protocol";
import { detectCurrentFeishuPage, extractFeishuCredentials } from "./content";

export function getNativeHostName(): string {
  return "dev.workclaw.runtime";
}

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
};

type FeishuCredentialReportMessage = {
  type: "workclaw.report-feishu-credentials";
  sessionId: string;
  appId: string;
  appSecret: string;
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
  },
): Promise<BridgeEnvelope<BridgeResponse> | null> {
  if (!isFeishuCredentialReportMessage(message)) {
    return null;
  }

  const sendViaNativeHost = dependencies.sendViaNativeHost ?? forwardCredentialsViaNativeHost;
  const sendViaLocalBridge = dependencies.sendViaLocalBridge ?? forwardCredentialsToLocalBridge;
  const payload = {
    sessionId: message.sessionId,
    appId: message.appId,
    appSecret: message.appSecret,
  };

  try {
    return await sendViaNativeHost(payload);
  } catch {
    return sendViaLocalBridge(payload, dependencies.bridgeBaseUrl);
  }
}

export function registerFeishuBackgroundMessageHandler(
  chromeLike: ChromeLike = globalThis as ChromeLike,
  dependencies: {
    bridgeBaseUrl: string;
    sendViaNativeHost?: typeof forwardCredentialsViaNativeHost;
    sendViaLocalBridge?: typeof forwardCredentialsToLocalBridge;
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

registerFeishuBackgroundMessageHandler(globalThis as ChromeLike, {
  bridgeBaseUrl: "http://127.0.0.1:4312",
});
