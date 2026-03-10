import test from "node:test";
import assert from "node:assert/strict";
import { FeishuChannelAdapter } from "../src/adapters/feishu/index.js";
import { normalizeFeishuEvent } from "../src/adapters/feishu/normalize.js";
import type { FeishuWsEventRecord, FeishuEmployeeWsStatus } from "../src/feishu_ws.js";

class FakeFeishuClient {
  public lastInput: unknown = null;

  async sendMessage(input: unknown) {
    this.lastInput = input;
    return { code: 0, data: { message_id: "om_msg_123" } };
  }
}

class FakeFeishuWsManager {
  private readonly eventsByEmployee = new Map<string, FeishuWsEventRecord[]>();
  private readonly statusByEmployeeMap = new Map<string, FeishuEmployeeWsStatus>();

  seedEmployee(employeeId: string, events: FeishuWsEventRecord[], status?: Partial<FeishuEmployeeWsStatus>) {
    this.eventsByEmployee.set(employeeId, [...events]);
    this.statusByEmployeeMap.set(employeeId, {
      employee_id: employeeId,
      running: true,
      started_at: "2026-03-10T00:00:00Z",
      queued_events: events.length,
      last_event_at: "2026-03-10T00:00:05Z",
      last_error: null,
      reconnect_attempts: 0,
      ...status,
    });
  }

  reconcile(items: Array<{ employee_id: string; app_id?: string; app_secret?: string }>) {
    for (const item of items) {
      const current = this.statusByEmployee(item.employee_id);
      this.statusByEmployeeMap.set(item.employee_id, {
        ...current,
        running: Boolean(item.app_id && item.app_secret),
      });
    }
    return { running: true, started_at: "2026-03-10T00:00:00Z", queued_events: 0, running_count: items.length, items: [] };
  }

  statusByEmployee(employeeId: string): FeishuEmployeeWsStatus {
    return (
      this.statusByEmployeeMap.get(employeeId) ?? {
        employee_id: employeeId,
        running: false,
        started_at: null,
        queued_events: 0,
        last_event_at: null,
        last_error: null,
        reconnect_attempts: 0,
      }
    );
  }

  drainAll(limit = 50, employeeId?: string): FeishuWsEventRecord[] {
    const id = employeeId || "default";
    const events = this.eventsByEmployee.get(id) || [];
    const drained = events.splice(0, limit);
    const current = this.statusByEmployee(id);
    this.statusByEmployeeMap.set(id, {
      ...current,
      queued_events: events.length,
    });
    return drained;
  }

  stop(employeeId?: string) {
    if (!employeeId) {
      return { running: false, started_at: null, queued_events: 0 };
    }
    const current = this.statusByEmployee(employeeId);
    this.statusByEmployeeMap.set(employeeId, {
      ...current,
      running: false,
      started_at: null,
    });
    return { running: false, started_at: null, queued_events: 0 };
  }
}

function createWsEvent(): FeishuWsEventRecord {
  return {
    employee_id: "connector-1",
    source_employee_ids: ["connector-1"],
    id: "chat-1:msg-1",
    event_type: "im.message.receive_v1",
    chat_id: "chat-1",
    message_id: "msg-1",
    text: "请联系销售负责人",
    mention_open_id: "ou_role_sales",
    mention_open_ids: ["ou_role_sales"],
    sender_open_id: "ou_user_1",
    received_at: "2026-03-10T00:00:05Z",
    raw: {
      tenant_key: "tenant-1",
    },
  };
}

test("normalizeFeishuEvent maps websocket record into normalized event", () => {
  const event = normalizeFeishuEvent(createWsEvent());
  assert.equal(event.channel, "feishu");
  assert.equal(event.thread_id, "chat-1");
  assert.equal(event.message_id, "msg-1");
  assert.equal(event.sender_id, "ou_user_1");
  assert.equal(event.routing_context.peer.kind, "group");
  assert.equal(event.mentions[0]?.id, "ou_role_sales");
});

test("feishu adapter exposes websocket health through channel adapter interface", async () => {
  const manager = new FakeFeishuWsManager();
  manager.seedEmployee("connector-1", [createWsEvent()], { reconnect_attempts: 2 });
  const adapter = new FeishuChannelAdapter(new FakeFeishuClient() as never, manager as never);

  const started = await adapter.start({
    adapter_name: "feishu",
    connector_id: "connector-1",
    settings: {
      employee_id: "connector-1",
      app_id: "cli_a",
      app_secret: "sec_b",
    },
  });

  const health = await adapter.health(started.instance_id);
  assert.equal(health.adapter_name, "feishu");
  assert.equal(health.state, "running");
  assert.equal(health.queue_depth, 1);
  assert.equal(health.reconnect_attempts, 2);
});

test("feishu adapter drains normalized events from websocket manager", async () => {
  const manager = new FakeFeishuWsManager();
  manager.seedEmployee("connector-1", [createWsEvent()]);
  const adapter = new FeishuChannelAdapter(new FakeFeishuClient() as never, manager as never);

  const started = await adapter.start({
    adapter_name: "feishu",
    connector_id: "connector-1",
    settings: {
      employee_id: "connector-1",
      app_id: "cli_a",
      app_secret: "sec_b",
    },
  });

  const events = await adapter.drainEvents(started.instance_id, 1);
  assert.equal(events.length, 1);
  assert.equal(events[0]?.raw_event_type, "im.message.receive_v1");
  assert.equal(events[0]?.account_id, "tenant-1");
});

test("feishu adapter delegates outbound sendMessage to FeishuClient", async () => {
  const client = new FakeFeishuClient();
  const manager = new FakeFeishuWsManager();
  manager.seedEmployee("connector-1", []);
  const adapter = new FeishuChannelAdapter(client as never, manager as never);

  const started = await adapter.start({
    adapter_name: "feishu",
    connector_id: "connector-1",
    settings: {
      employee_id: "connector-1",
      app_id: "cli_a",
      app_secret: "sec_b",
    },
  });

  const result = await adapter.sendMessage(started.instance_id, {
    channel: "feishu",
    thread_id: "chat-1",
    reply_target: null,
    text: "已收到",
  });

  assert.equal(result.message_id, "om_msg_123");
  assert.deepEqual(client.lastInput, {
    app_id: "cli_a",
    app_secret: "sec_b",
    receive_id: "chat-1",
    receive_id_type: "chat_id",
    msg_type: "text",
    content: JSON.stringify({ text: "已收到" }),
    uuid: started.instance_id,
  });
});
