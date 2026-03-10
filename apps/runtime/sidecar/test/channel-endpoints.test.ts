import test from "node:test";
import assert from "node:assert/strict";
import { createSidecarApp } from "../src/index.js";

class FakeChannelKernel {
  public startCalls: unknown[] = [];
  public stopCalls: string[] = [];
  public healthCalls: string[] = [];
  public drainCalls: Array<{ instanceId: string; limit: number }> = [];
  public sendCalls: Array<{ instanceId: string; request: unknown }> = [];

  async start(config: unknown) {
    this.startCalls.push(config);
    const connectorId =
      config && typeof config === "object" && "connector_id" in config
        ? String((config as Record<string, unknown>).connector_id)
        : "unknown";
    return { instance_id: `feishu:${connectorId}` };
  }

  async stop(instanceId: string) {
    this.stopCalls.push(instanceId);
  }

  async health(instanceId: string) {
    this.healthCalls.push(instanceId);
    return {
      adapter_name: "feishu",
      instance_id: instanceId,
      state: "running",
      last_ok_at: "2026-03-10T00:00:00Z",
      last_error: null,
      reconnect_attempts: 1,
      queue_depth: 2,
    };
  }

  async drainEvents(instanceId: string, limit: number) {
    this.drainCalls.push({ instanceId, limit });
    return [
      {
        channel: "feishu",
        workspace_id: "tenant-1",
        account_id: "tenant-1",
        thread_id: "chat-1",
        message_id: "msg-1",
        sender_id: "ou_user_1",
        sender_name: null,
        text: "hello",
        mentions: [],
        raw_event_type: "im.message.receive_v1",
        occurred_at: "2026-03-10T00:00:00Z",
        reply_target: "msg-1",
        routing_context: {
          peer: { kind: "group", id: "chat-1" },
          parent_peer: null,
          guild_id: null,
          team_id: null,
          member_role_ids: [],
          identity_links: [],
        },
        raw_payload: { ok: true },
      },
    ];
  }

  async sendMessage(instanceId: string, request: unknown) {
    this.sendCalls.push({ instanceId, request });
    return {
      message_id: "om_msg_123",
      delivered_at: "2026-03-10T00:00:01Z",
      raw_response: { ok: true },
    };
  }
}

test("channel endpoints start and query adapter instances", async () => {
  const kernel = new FakeChannelKernel();
  const app = createSidecarApp({ channelKernel: kernel as never });

  const startRes = await app.fetch(
    new Request("http://localhost/api/channels/start", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        adapter_name: "feishu",
        connector_id: "connector-1",
        settings: { employee_id: "connector-1", app_id: "cli_a", app_secret: "sec_b" },
      }),
    }),
  );
  assert.equal(startRes.status, 200);
  const startJson = await startRes.json();
  const started = JSON.parse(String(startJson.output || "null"));
  assert.equal(started.instance_id, "feishu:connector-1");

  const healthRes = await app.fetch(
    new Request("http://localhost/api/channels/health", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ instance_id: "feishu:connector-1" }),
    }),
  );
  assert.equal(healthRes.status, 200);
  const healthJson = await healthRes.json();
  const health = JSON.parse(String(healthJson.output || "null"));
  assert.equal(health.state, "running");
  assert.equal(kernel.healthCalls[0], "feishu:connector-1");
});

test("feishu websocket compatibility endpoints forward start status and stop through kernel", async () => {
  const kernel = new FakeChannelKernel();
  const app = createSidecarApp({ channelKernel: kernel as never });

  const startRes = await app.fetch(
    new Request("http://localhost/api/feishu/ws/start", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ employee_id: "connector-1", app_id: "cli_a", app_secret: "sec_b" }),
    }),
  );
  assert.equal(startRes.status, 200);

  const statusRes = await app.fetch(
    new Request("http://localhost/api/feishu/ws/status", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ employee_id: "connector-1" }),
    }),
  );
  assert.equal(statusRes.status, 200);
  const statusJson = await statusRes.json();
  const status = JSON.parse(String(statusJson.output || "null"));
  assert.equal(status.running, true);
  assert.equal(status.employee_id, "connector-1");

  const stopRes = await app.fetch(
    new Request("http://localhost/api/feishu/ws/stop", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ employee_id: "connector-1" }),
    }),
  );
  assert.equal(stopRes.status, 200);
  assert.equal(kernel.stopCalls[0], "feishu:connector-1");
});

test("route resolve endpoint remains channel-neutral after channel endpoint wiring", async () => {
  const app = createSidecarApp();
  const res = await app.fetch(
    new Request("http://localhost/api/openclaw/resolve-route", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        channel: "discord",
        account_id: "acct-a",
        peer: { kind: "group", id: "thread-1" },
        default_agent_id: "main",
        bindings: [{ agentId: "main", match: { channel: "discord", accountId: "*" } }],
      }),
    }),
  );
  assert.equal(res.status, 200);
  const json = await res.json();
  assert.equal(Boolean(json.output), true);
});
