import { createBridgeClient } from "../native-host/client";
import { type BridgeEnvelope, type BridgeResponse } from "../shared/protocol";

export function getNativeHostName(): string {
  return "dev.workclaw.runtime";
}

type FetchLike = typeof fetch;

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
