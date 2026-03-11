import {
  ChannelAdapterRegistry,
  createChannelAdapterRegistry,
} from "./registry.js";
import type {
  AckRequest,
  AdapterConfig,
  AdapterHandle,
  AdapterHealth,
  ChannelAdapter,
  ConnectorDescriptor,
  ConnectorDiagnostics,
  ConnectorReplayStats,
  NormalizedImEvent,
  SendMessageRequest,
  SendMessageResult,
} from "./types.js";
import { normalizeConnectorStatus as toConnectorStatus } from "./types.js";

interface InstanceRecord {
  adapterName: string;
  adapter: ChannelAdapter;
}

interface ReplayEntry {
  event: NormalizedImEvent;
  first_seen_at: string;
  last_drained_at: string;
  ack_status: string | null;
}

const MAX_REPLAY_EVENTS = 200;

export class ChannelAdapterKernel {
  private readonly instances = new Map<string, InstanceRecord>();
  private readonly replayByInstance = new Map<string, Map<string, ReplayEntry>>();

  constructor(private readonly registry: ChannelAdapterRegistry) {}

  async catalog(): Promise<ConnectorDescriptor[]> {
    return Promise.all(
      this.registry.entries().map(async ([, adapter]) => adapter.describe()),
    );
  }

  async start(config: AdapterConfig): Promise<AdapterHandle> {
    const adapter = this.registry.get(config.adapter_name);
    if (!adapter) {
      throw new Error(`unknown adapter: ${config.adapter_name}`);
    }

    const handle = await adapter.start(config);
    this.instances.set(handle.instance_id, {
      adapterName: config.adapter_name,
      adapter,
    });
    return handle;
  }

  async stop(instanceId: string): Promise<void> {
    const record = this.getInstance(instanceId);
    await record.adapter.stop(instanceId);
    this.instances.delete(instanceId);
    this.replayByInstance.delete(instanceId);
  }

  async health(instanceId: string): Promise<AdapterHealth> {
    return this.getInstance(instanceId).adapter.health(instanceId);
  }

  async drainEvents(instanceId: string, limit: number): Promise<NormalizedImEvent[]> {
    const events = await this.getInstance(instanceId).adapter.drainEvents(instanceId, limit);
    this.recordReplayEvents(instanceId, events);
    return events;
  }

  async sendMessage(
    instanceId: string,
    req: SendMessageRequest,
  ): Promise<SendMessageResult> {
    return this.getInstance(instanceId).adapter.sendMessage(instanceId, req);
  }

  async ack(instanceId: string, req: AckRequest): Promise<void> {
    await this.getInstance(instanceId).adapter.ack(instanceId, req);
    const replay = this.replayByInstance.get(instanceId);
    const entry = replay?.get(req.message_id);
    if (entry) {
      entry.ack_status = req.status || "acked";
    }
  }

  async replayEvents(instanceId: string, limit = 50): Promise<NormalizedImEvent[]> {
    this.getInstance(instanceId);
    const replay = this.replayByInstance.get(instanceId);
    if (!replay) {
      return [];
    }

    return Array.from(replay.values())
      .sort((left, right) => right.last_drained_at.localeCompare(left.last_drained_at))
      .slice(0, Math.max(0, limit))
      .map((entry) => entry.event);
  }

  async diagnostics(instanceId: string): Promise<ConnectorDiagnostics> {
    const record = this.getInstance(instanceId);
    const [connector, health] = await Promise.all([
      record.adapter.describe(),
      record.adapter.health(instanceId),
    ]);
    return {
      connector,
      status: toConnectorStatus(health),
      health,
      replay: this.getReplayStats(instanceId),
    };
  }

  private getInstance(instanceId: string): InstanceRecord {
    const record = this.instances.get(instanceId);
    if (!record) {
      throw new Error(`unknown adapter instance: ${instanceId}`);
    }
    return record;
  }

  private recordReplayEvents(instanceId: string, events: NormalizedImEvent[]): void {
    if (events.length === 0) {
      return;
    }

    const replay = this.ensureReplayStore(instanceId);
    const now = new Date().toISOString();
    for (const event of events) {
      const existing = replay.get(event.message_id);
      replay.set(event.message_id, {
        event,
        first_seen_at: existing?.first_seen_at || now,
        last_drained_at: now,
        ack_status: existing?.ack_status || null,
      });
    }

    while (replay.size > MAX_REPLAY_EVENTS) {
      const oldestKey = replay.keys().next().value;
      if (!oldestKey) {
        break;
      }
      replay.delete(oldestKey);
    }
  }

  private ensureReplayStore(instanceId: string): Map<string, ReplayEntry> {
    let replay = this.replayByInstance.get(instanceId);
    if (!replay) {
      replay = new Map<string, ReplayEntry>();
      this.replayByInstance.set(instanceId, replay);
    }
    return replay;
  }

  private getReplayStats(instanceId: string): ConnectorReplayStats {
    const replay = this.replayByInstance.get(instanceId);
    if (!replay) {
      return { retained_events: 0, acked_events: 0 };
    }

    let ackedEvents = 0;
    for (const entry of replay.values()) {
      if (entry.ack_status) {
        ackedEvents += 1;
      }
    }
    return {
      retained_events: replay.size,
      acked_events: ackedEvents,
    };
  }
}

export { ChannelAdapterRegistry, createChannelAdapterRegistry };
