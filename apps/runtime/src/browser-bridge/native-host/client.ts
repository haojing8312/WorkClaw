import { isBridgeEnvelope, type BridgeEnvelope, type BridgeRequest, type BridgeResponse } from "../shared/protocol";

type FetchLike = typeof fetch;

export function createBridgeClient(baseUrl: string, fetchImpl: FetchLike = fetch) {
  return {
    async send(
      envelope: BridgeEnvelope<BridgeRequest>,
    ): Promise<BridgeEnvelope<BridgeResponse>> {
      const response = await fetchImpl(
        `${baseUrl.replace(/\/$/, "")}/api/browser-bridge/native-message`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify(envelope),
        },
      );

      const payload = await response.json();
      if (!isBridgeEnvelope(payload)) {
        throw new Error("invalid browser bridge response");
      }

      return payload as BridgeEnvelope<BridgeResponse>;
    },
  };
}
