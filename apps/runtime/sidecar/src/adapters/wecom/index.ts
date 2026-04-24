import type {
  AckRequest,
  AdapterConfig,
  AdapterHandle,
  AdapterHealth,
  ChannelAdapter,
  ConnectorDescriptor,
  SendMessageRequest,
  SendMessageResult,
} from "../types.js";
import { classifyConnectorIssue } from "../types.js";
import { normalizeWecomEvent } from "./normalize.js";
import { recordWecomInboundSample } from "./recording.js";
import type { WecomAdapterHealthSnapshot, WecomInboundEvent } from "./types.js";

interface WecomInstanceState {
  connectorId: string;
  corpId: string;
  agentId: string;
  agentSecret: string;
}

interface WecomTransportLike {
  sendTextMessage(input: {
    corp_id: string;
    agent_id: string;
    agent_secret: string;
    conversation_id: string;
    text: string;
    idempotency_key: string;
  }): Promise<{
    msgid?: string;
    [key: string]: unknown;
  }>;
}

interface WecomRuntimeLike {
  start(settings: {
    connector_id: string;
    corp_id: string;
    agent_id: string;
    agent_secret: string;
  }): void;
  stop(connectorId: string): void;
  status(connectorId: string): WecomAdapterHealthSnapshot;
  drain(connectorId: string, limit: number): WecomInboundEvent[];
}

class NoopWecomTransport implements WecomTransportLike {
  async sendTextMessage(input: {
    corp_id: string;
    agent_id: string;
    agent_secret: string;
    conversation_id: string;
    text: string;
    idempotency_key: string;
  }) {
    return {
      msgid: `wecom:${input.idempotency_key}`,
      accepted: true,
    };
  }
}

class NoopWecomRuntime implements WecomRuntimeLike {
  private readonly states = new Map<string, WecomAdapterHealthSnapshot>();
  private readonly queues = new Map<string, WecomInboundEvent[]>();

  start(settings: {
    connector_id: string;
    corp_id: string;
    agent_id: string;
    agent_secret: string;
  }): void {
    this.states.set(settings.connector_id, {
      running: true,
      started_at: new Date().toISOString(),
      last_event_at: null,
      last_error: null,
      reconnect_attempts: 0,
      queued_events: 0,
    });
    if (!this.queues.has(settings.connector_id)) {
      this.queues.set(settings.connector_id, []);
    }
  }

  stop(connectorId: string): void {
    const current = this.status(connectorId);
    this.states.set(connectorId, {
      ...current,
      running: false,
    });
  }

  status(connectorId: string): WecomAdapterHealthSnapshot {
    return (
      this.states.get(connectorId) ?? {
        running: false,
        started_at: null,
        last_event_at: null,
        last_error: null,
        reconnect_attempts: 0,
        queued_events: 0,
      }
    );
  }

  drain(connectorId: string, limit: number): WecomInboundEvent[] {
    const queue = this.queues.get(connectorId) || [];
    const drained = queue.splice(0, limit);
    const current = this.status(connectorId);
    this.states.set(connectorId, {
      ...current,
      queued_events: queue.length,
    });
    return drained;
  }
}

function toHealth(snapshot: WecomAdapterHealthSnapshot, instanceId: string): AdapterHealth {
  return {
    adapter_name: "wecom",
    instance_id: instanceId,
    state: snapshot.running ? "running" : snapshot.last_error ? "error" : "stopped",
    last_ok_at: snapshot.last_event_at || snapshot.started_at,
    last_error: snapshot.last_error,
    reconnect_attempts: snapshot.reconnect_attempts,
    queue_depth: snapshot.queued_events,
    issue: classifyConnectorIssue(snapshot.last_error, snapshot.last_event_at || snapshot.started_at),
  };
}

export class WecomChannelAdapter implements ChannelAdapter {
  private readonly instances = new Map<string, WecomInstanceState>();

  constructor(
    private readonly transport: WecomTransportLike = new NoopWecomTransport(),
    private readonly runtime: WecomRuntimeLike = new NoopWecomRuntime(),
  ) {}

  async describe(): Promise<ConnectorDescriptor> {
    return {
      channel: "wecom",
      display_name: "企业微信连接器",
      capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
    };
  }

  async start(config: AdapterConfig): Promise<AdapterHandle> {
    const corpId = String(config.settings.corp_id || "").trim();
    const agentId = String(config.settings.agent_id || "").trim();
    const agentSecret = String(config.settings.agent_secret || "").trim();
    if (!corpId || !agentId || !agentSecret) {
      throw new Error("Missing WeCom adapter settings: corp_id/agent_id/agent_secret");
    }

    this.runtime.start({
      connector_id: config.connector_id,
      corp_id: corpId,
      agent_id: agentId,
      agent_secret: agentSecret,
    });

    const instanceId = `wecom:${config.connector_id}`;
    this.instances.set(instanceId, {
      connectorId: config.connector_id,
      corpId,
      agentId,
      agentSecret,
    });
    return { instance_id: instanceId };
  }

  async stop(instanceId: string): Promise<void> {
    const instance = this.getInstance(instanceId);
    this.runtime.stop(instance.connectorId);
    this.instances.delete(instanceId);
  }

  async health(instanceId: string): Promise<AdapterHealth> {
    const instance = this.getInstance(instanceId);
    return toHealth(this.runtime.status(instance.connectorId), instanceId);
  }

  async drainEvents(instanceId: string, limit: number) {
    const instance = this.getInstance(instanceId);
    const inboundEvents = this.runtime.drain(instance.connectorId, limit);
    const normalizedEvents = await Promise.all(
      inboundEvents.map(async (event) => {
        const normalizedEvent = normalizeWecomEvent(event);
        try {
          await recordWecomInboundSample(event, normalizedEvent);
        } catch {
          // Local sample capture is best-effort and must not break message delivery.
        }
        return normalizedEvent;
      }),
    );
    return normalizedEvents;
  }

  async sendMessage(instanceId: string, req: SendMessageRequest): Promise<SendMessageResult> {
    const instance = this.getInstance(instanceId);
    const response = await this.transport.sendTextMessage({
      corp_id: instance.corpId,
      agent_id: instance.agentId,
      agent_secret: instance.agentSecret,
      conversation_id: req.reply_target || req.thread_id,
      text: req.text,
      idempotency_key: instanceId,
    });
    return {
      message_id: typeof response?.msgid === "string" ? response.msgid : "",
      delivered_at: new Date().toISOString(),
      raw_response: response,
    };
  }

  async ack(_instanceId: string, _req: AckRequest): Promise<void> {}

  private getInstance(instanceId: string): WecomInstanceState {
    const instance = this.instances.get(instanceId);
    if (!instance) {
      throw new Error(`unknown adapter instance: ${instanceId}`);
    }
    return instance;
  }
}
