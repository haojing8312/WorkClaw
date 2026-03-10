import test from "node:test";
import assert from "node:assert/strict";
import {
  ChannelAdapterKernel,
  createChannelAdapterRegistry,
} from "../src/adapters/kernel.js";
import type {
  AdapterConfig,
  AdapterHealth,
  AckRequest,
  ChannelAdapter,
  NormalizedImEvent,
  SendMessageRequest,
  SendMessageResult,
} from "../src/adapters/types.js";

function createEvent(id: string): NormalizedImEvent {
  return {
    channel: "feishu",
    workspace_id: "ws-1",
    account_id: "acct-1",
    thread_id: "thread-1",
    message_id: id,
    sender_id: "user-1",
    sender_name: "Alice",
    text: "hello",
    mentions: [],
    raw_event_type: "message.created",
    occurred_at: "2026-03-10T00:00:00Z",
    reply_target: null,
    routing_context: {
      peer: { kind: "group", id: "thread-1" },
      parent_peer: null,
      guild_id: null,
      team_id: null,
      member_role_ids: [],
      identity_links: [],
    },
    raw_payload: { ok: true },
  };
}

class FakeAdapter implements ChannelAdapter {
  private readonly events: NormalizedImEvent[] = [createEvent("msg-1"), createEvent("msg-2")];
  private readonly healthByInstance = new Map<string, AdapterHealth>();

  async start(config: AdapterConfig) {
    const instanceId = `fake:${config.connector_id}`;
    this.healthByInstance.set(instanceId, {
      adapter_name: "fake",
      instance_id: instanceId,
      state: "running",
      last_ok_at: "2026-03-10T00:00:00Z",
      last_error: null,
      reconnect_attempts: 0,
      queue_depth: this.events.length,
    });
    return { instance_id: instanceId };
  }

  async stop(instanceId: string): Promise<void> {
    this.healthByInstance.set(instanceId, {
      adapter_name: "fake",
      instance_id: instanceId,
      state: "stopped",
      last_ok_at: null,
      last_error: null,
      reconnect_attempts: 0,
      queue_depth: 0,
    });
  }

  async health(instanceId: string): Promise<AdapterHealth> {
    const health = this.healthByInstance.get(instanceId);
    if (!health) {
      throw new Error(`unknown instance: ${instanceId}`);
    }
    return health;
  }

  async drainEvents(instanceId: string, limit: number): Promise<NormalizedImEvent[]> {
    const health = await this.health(instanceId);
    const drained = this.events.splice(0, limit);
    this.healthByInstance.set(instanceId, {
      ...health,
      queue_depth: this.events.length,
    });
    return drained;
  }

  async sendMessage(_instanceId: string, req: SendMessageRequest): Promise<SendMessageResult> {
    return {
      message_id: `sent:${req.thread_id}`,
      delivered_at: "2026-03-10T00:00:01Z",
      raw_response: { ok: true },
    };
  }

  async ack(_instanceId: string, _req: AckRequest): Promise<void> {}
}

test("channel adapter kernel starts adapter instances via registry", async () => {
  const registry = createChannelAdapterRegistry();
  registry.register("fake", new FakeAdapter());
  const kernel = new ChannelAdapterKernel(registry);

  const started = await kernel.start({
    adapter_name: "fake",
    connector_id: "connector-1",
    settings: { token: "secret" },
  });

  assert.equal(started.instance_id, "fake:connector-1");
});

test("channel adapter kernel exposes health by instance", async () => {
  const registry = createChannelAdapterRegistry();
  registry.register("fake", new FakeAdapter());
  const kernel = new ChannelAdapterKernel(registry);

  const started = await kernel.start({
    adapter_name: "fake",
    connector_id: "connector-1",
    settings: {},
  });

  const health = await kernel.health(started.instance_id);
  assert.equal(health.state, "running");
  assert.equal(health.queue_depth, 2);
});

test("channel adapter kernel drains normalized events from registered adapter", async () => {
  const registry = createChannelAdapterRegistry();
  registry.register("fake", new FakeAdapter());
  const kernel = new ChannelAdapterKernel(registry);

  const started = await kernel.start({
    adapter_name: "fake",
    connector_id: "connector-1",
    settings: {},
  });

  const events = await kernel.drainEvents(started.instance_id, 1);
  assert.equal(events.length, 1);
  assert.equal(events[0]?.message_id, "msg-1");

  const health = await kernel.health(started.instance_id);
  assert.equal(health.queue_depth, 1);
});

test("channel adapter kernel rejects unknown adapters with controlled error", async () => {
  const kernel = new ChannelAdapterKernel(createChannelAdapterRegistry());

  await assert.rejects(
    () =>
      kernel.start({
        adapter_name: "missing",
        connector_id: "connector-1",
        settings: {},
      }),
    /unknown adapter: missing/,
  );
});
