import test from "node:test";
import assert from "node:assert/strict";
import { WecomChannelAdapter } from "../src/adapters/wecom/index.js";
import { normalizeWecomEvent } from "../src/adapters/wecom/normalize.js";
import type { WecomInboundEvent, WecomAdapterHealthSnapshot } from "../src/adapters/wecom/types.js";

class FakeWecomTransport {
  public lastSendInput: unknown = null;

  async sendTextMessage(input: unknown) {
    this.lastSendInput = input;
    return {
      errcode: 0,
      errmsg: "ok",
      msgid: "wecom-msg-1",
    };
  }
}

class FakeWecomRuntime {
  private readonly eventsByConnector = new Map<string, WecomInboundEvent[]>();
  private readonly healthByConnector = new Map<string, WecomAdapterHealthSnapshot>();

  seedConnector(connectorId: string, events: WecomInboundEvent[], health?: Partial<WecomAdapterHealthSnapshot>) {
    this.eventsByConnector.set(connectorId, [...events]);
    this.healthByConnector.set(connectorId, {
      running: true,
      started_at: "2026-03-10T00:00:00Z",
      last_event_at: "2026-03-10T00:00:05Z",
      last_error: null,
      reconnect_attempts: 0,
      queued_events: events.length,
      ...health,
    });
  }

  start(_settings: { connector_id: string; corp_id: string; agent_id: string; agent_secret: string }) {
    return;
  }

  stop(connectorId: string) {
    const current = this.status(connectorId);
    this.healthByConnector.set(connectorId, {
      ...current,
      running: false,
    });
  }

  status(connectorId: string): WecomAdapterHealthSnapshot {
    return (
      this.healthByConnector.get(connectorId) ?? {
        running: false,
        started_at: null,
        last_event_at: null,
        last_error: null,
        reconnect_attempts: 0,
        queued_events: 0,
      }
    );
  }

  drain(connectorId: string, limit = 50): WecomInboundEvent[] {
    const events = this.eventsByConnector.get(connectorId) || [];
    const drained = events.splice(0, limit);
    const current = this.status(connectorId);
    this.healthByConnector.set(connectorId, {
      ...current,
      queued_events: events.length,
    });
    return drained;
  }
}

function createWecomEvent(): WecomInboundEvent {
  return {
    connector_id: "connector-wecom-1",
    corp_id: "corp-123",
    agent_id: "1000002",
    event_type: "message.receive",
    conversation_type: "group",
    conversation_id: "room-001",
    message_id: "msg-001",
    sender_id: "zhangsan",
    sender_name: "张三",
    text: "请联系项目经理",
    mentions: [{ type: "user", id: "project_manager", name: "项目经理" }],
    occurred_at: "2026-03-10T00:00:05Z",
    reply_target: "room-001",
    raw_payload: {
      msgtype: "text",
    },
  };
}

test("normalizeWecomEvent maps inbound payload into normalized event", () => {
  const event = normalizeWecomEvent(createWecomEvent());
  assert.equal(event.channel, "wecom");
  assert.equal(event.workspace_id, "corp-123");
  assert.equal(event.account_id, "1000002");
  assert.equal(event.thread_id, "room-001");
  assert.equal(event.routing_context.peer.kind, "group");
  assert.equal(event.mentions[0]?.id, "project_manager");
});

test("wecom adapter exposes health through channel adapter interface", async () => {
  const runtime = new FakeWecomRuntime();
  runtime.seedConnector("connector-wecom-1", [createWecomEvent()], { reconnect_attempts: 3 });
  const adapter = new WecomChannelAdapter(new FakeWecomTransport() as never, runtime as never);

  const started = await adapter.start({
    adapter_name: "wecom",
    connector_id: "connector-wecom-1",
    settings: {
      corp_id: "corp-123",
      agent_id: "1000002",
      agent_secret: "secret-abc",
    },
  });

  const health = await adapter.health(started.instance_id);
  assert.equal(health.adapter_name, "wecom");
  assert.equal(health.state, "running");
  assert.equal(health.queue_depth, 1);
  assert.equal(health.reconnect_attempts, 3);
});

test("wecom adapter drains normalized events from runtime buffer", async () => {
  const runtime = new FakeWecomRuntime();
  runtime.seedConnector("connector-wecom-1", [createWecomEvent()]);
  const adapter = new WecomChannelAdapter(new FakeWecomTransport() as never, runtime as never);

  const started = await adapter.start({
    adapter_name: "wecom",
    connector_id: "connector-wecom-1",
    settings: {
      corp_id: "corp-123",
      agent_id: "1000002",
      agent_secret: "secret-abc",
    },
  });

  const events = await adapter.drainEvents(started.instance_id, 1);
  assert.equal(events.length, 1);
  assert.equal(events[0]?.channel, "wecom");
  assert.equal(events[0]?.sender_id, "zhangsan");
});

test("wecom adapter delegates outbound sendMessage to transport shim", async () => {
  const transport = new FakeWecomTransport();
  const runtime = new FakeWecomRuntime();
  runtime.seedConnector("connector-wecom-1", []);
  const adapter = new WecomChannelAdapter(transport as never, runtime as never);

  const started = await adapter.start({
    adapter_name: "wecom",
    connector_id: "connector-wecom-1",
    settings: {
      corp_id: "corp-123",
      agent_id: "1000002",
      agent_secret: "secret-abc",
    },
  });

  const result = await adapter.sendMessage(started.instance_id, {
    channel: "wecom",
    thread_id: "room-001",
    reply_target: "room-001",
    text: "已收到，马上处理",
  });

  assert.equal(result.message_id, "wecom-msg-1");
  assert.deepEqual(transport.lastSendInput, {
    corp_id: "corp-123",
    agent_id: "1000002",
    agent_secret: "secret-abc",
    conversation_id: "room-001",
    text: "已收到，马上处理",
    idempotency_key: started.instance_id,
  });
});

test("wecom adapter exposes connector metadata and capabilities", async () => {
  const adapter = new WecomChannelAdapter(new FakeWecomTransport() as never, new FakeWecomRuntime() as never);

  assert.equal(typeof adapter.describe, "function");
  const descriptor = await adapter.describe();
  assert.equal(descriptor.channel, "wecom");
  assert.equal(descriptor.display_name, "企业微信连接器");
  assert.deepEqual(descriptor.capabilities, [
    "receive_text",
    "send_text",
    "group_route",
    "direct_route",
  ]);
});
