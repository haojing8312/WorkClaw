import { describe, expect, it } from "vitest";
import { decodeNativeMessage, encodeNativeMessage } from "../index";

describe("native messaging framing", () => {
  it("round-trips a bridge envelope", () => {
    const encoded = encodeNativeMessage({
      version: 1,
      sessionId: "sess-1",
      kind: "request",
      payload: { type: "session.start", provider: "feishu" },
    });

    expect(decodeNativeMessage(encoded)).toMatchObject({
      sessionId: "sess-1",
      kind: "request",
      payload: { type: "session.start", provider: "feishu" },
    });
  });
});
