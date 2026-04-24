import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { WecomChannelAdapter } from "../src/adapters/wecom/index.js";
import { normalizeWecomEvent } from "../src/adapters/wecom/normalize.js";
import {
  buildWecomCaptureFileName,
  resolveWecomCaptureDir,
} from "../src/adapters/wecom/recording.js";
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
    topic_id: null,
    root_id: null,
    raw_payload: {
      msgtype: "text",
    },
  };
}

function loadWecomFixture(name: string): WecomInboundEvent {
  const path = join(import.meta.dirname, "fixtures", "wecom", name);
  return JSON.parse(readFileSync(path, "utf8")) as WecomInboundEvent;
}

test("normalizeWecomEvent maps inbound payload into normalized event", () => {
  const event = normalizeWecomEvent(createWecomEvent());
  assert.equal(event.channel, "wecom");
  assert.equal(event.workspace_id, "corp-123");
  assert.equal(event.account_id, "1000002");
  assert.equal(event.thread_id, "room-001");
  assert.equal(event.chat_type, "group");
  assert.equal(event.routing_context.peer.kind, "group");
  assert.equal(event.mentions[0]?.id, "project_manager");
});

test("normalizeWecomEvent preserves topic metadata for normalized routing", () => {
  const event = normalizeWecomEvent({
    ...createWecomEvent(),
    topic_id: "topic-42",
    root_id: "root-42",
  });

  assert.equal(event.topic_id, "topic-42");
  assert.equal(event.root_id, "root-42");
  assert.deepEqual(event.routing_context.topic, {
    kind: "topic",
    id: "topic-42",
  });
});

test("normalizeWecomEvent falls back to raw payload topic fields", () => {
  const event = normalizeWecomEvent({
    ...createWecomEvent(),
    raw_payload: {
      topicId: "topic-from-raw",
      rootId: "root-from-raw",
    },
  });

  assert.equal(event.topic_id, "topic-from-raw");
  assert.equal(event.root_id, "root-from-raw");
  assert.deepEqual(event.routing_context.topic, {
    kind: "topic",
    id: "topic-from-raw",
  });
});

test("normalizeWecomEvent preserves top-level topic fixture fields", () => {
  const event = normalizeWecomEvent(loadWecomFixture("inbound-topic-top-level.json"));

  assert.equal(event.chat_type, "group");
  assert.equal(event.topic_id, "topic-42");
  assert.equal(event.root_id, "root-42");
  assert.deepEqual(event.routing_context.topic, {
    kind: "topic",
    id: "topic-42",
  });
});

test("normalizeWecomEvent treats raw threadId as topic fallback fixture", () => {
  const event = normalizeWecomEvent(loadWecomFixture("inbound-topic-raw-threadid.json"));

  assert.equal(event.topic_id, "topic-from-thread-id");
  assert.equal(event.root_id, "root-from-thread-id");
  assert.deepEqual(event.routing_context.topic, {
    kind: "topic",
    id: "topic-from-thread-id",
  });
});

test("resolveWecomCaptureDir only enables recording when env var is set", () => {
  assert.equal(resolveWecomCaptureDir({}), null);
  assert.equal(
    resolveWecomCaptureDir({ WORKCLAW_WECOM_CAPTURE_DIR: "  E:/tmp/wecom-capture  " } as NodeJS.ProcessEnv),
    "E:/tmp/wecom-capture",
  );
});

test("buildWecomCaptureFileName derives a stable file name", () => {
  const name = buildWecomCaptureFileName(createWecomEvent());
  assert.match(name, /^2026-03-10T00_00_05Z__room-001__msg-001\.json$/);
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

test("wecom adapter can capture inbound samples to a local temp directory", async () => {
  const runtime = new FakeWecomRuntime();
  runtime.seedConnector("connector-wecom-1", [loadWecomFixture("inbound-topic-top-level.json")]);
  const adapter = new WecomChannelAdapter(new FakeWecomTransport() as never, runtime as never);
  const tempRoot = await mkdtemp(join(tmpdir(), "workclaw-wecom-capture-"));
  const previousCaptureDir = process.env.WORKCLAW_WECOM_CAPTURE_DIR;

  try {
    process.env.WORKCLAW_WECOM_CAPTURE_DIR = tempRoot;

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
    assert.equal(events[0]?.topic_id, "topic-42");

    const capturedFiles = await readdir(tempRoot);
    assert.equal(capturedFiles.length, 1);

    const capturedJson = JSON.parse(
      await readFile(join(tempRoot, capturedFiles[0]!), "utf8"),
    ) as {
      source: string;
      inbound_event: WecomInboundEvent;
      normalized_event: { topic_id?: string | null };
    };
    assert.equal(capturedJson.source, "wecom-adapter");
    assert.equal(capturedJson.inbound_event.conversation_id, "room-001");
    assert.equal(capturedJson.normalized_event.topic_id, "topic-42");
  } finally {
    if (previousCaptureDir === undefined) {
      delete process.env.WORKCLAW_WECOM_CAPTURE_DIR;
    } else {
      process.env.WORKCLAW_WECOM_CAPTURE_DIR = previousCaptureDir;
    }
    await rm(tempRoot, { recursive: true, force: true });
  }
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
