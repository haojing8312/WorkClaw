import test from "node:test";
import assert from "node:assert/strict";
import { resolveRoute } from "../src/openclaw-bridge/route-engine.js";

test("peer binding wins over account and channel", () => {
  const out = resolveRoute({
    channel: "feishu",
    accountId: "acct-a",
    peer: { kind: "group", id: "chat-1" },
    bindings: [
      { agentId: "channel-agent", match: { channel: "feishu", accountId: "*" } },
      { agentId: "account-agent", match: { channel: "feishu", accountId: "acct-a" } },
      {
        agentId: "peer-agent",
        match: {
          channel: "feishu",
          accountId: "acct-a",
          peer: { kind: "group", id: "chat-1" },
        },
      },
    ],
    defaultAgentId: "main",
  });

  assert.equal(out.agentId, "peer-agent");
  assert.equal(out.matchedBy, "binding.peer");
});

