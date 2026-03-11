import { createBridgeClient } from "../native-host/client";
import { type BridgeEnvelope, type BridgeResponse } from "../shared/protocol";

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
  };
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
