import test from "node:test";
import assert from "node:assert/strict";

import {
  decodeNativeMessage,
  encodeNativeMessage,
  processNativeHostFrame,
} from "./workclaw-chrome-native-host.mjs";

test("encodeNativeMessage and decodeNativeMessage round-trip a browser bridge envelope", () => {
  const encoded = encodeNativeMessage({
    version: 1,
    sessionId: "sess-script-1",
    kind: "request",
    payload: { type: "session.start", provider: "feishu" },
  });

  assert.deepEqual(decodeNativeMessage(encoded), {
    version: 1,
    sessionId: "sess-script-1",
    kind: "request",
    payload: { type: "session.start", provider: "feishu" },
  });
});

test("processNativeHostFrame forwards the decoded message to the local bridge endpoint", async () => {
  const calls = [];
  const input = encodeNativeMessage({
    version: 1,
    sessionId: "sess-script-2",
    kind: "request",
    payload: { type: "credentials.report", appId: "cli_123", appSecret: "sec_456" },
  });

  const output = await processNativeHostFrame(input, {
    baseUrl: "http://127.0.0.1:4312",
    fetchImpl: async (url, init) => {
      calls.push({ url: String(url), init });
      return new Response(
        JSON.stringify({
          version: 1,
          sessionId: "sess-script-2",
          kind: "response",
          payload: { type: "action.pause", reason: "saved" },
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      );
    },
  });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "http://127.0.0.1:4312/api/browser-bridge/native-message");
  assert.deepEqual(decodeNativeMessage(output), {
    version: 1,
    sessionId: "sess-script-2",
    kind: "response",
    payload: { type: "action.pause", reason: "saved" },
  });
});
