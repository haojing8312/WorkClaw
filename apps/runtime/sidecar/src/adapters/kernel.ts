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
  NormalizedImEvent,
  SendMessageRequest,
  SendMessageResult,
} from "./types.js";

interface InstanceRecord {
  adapterName: string;
  adapter: ChannelAdapter;
}

export class ChannelAdapterKernel {
  private readonly instances = new Map<string, InstanceRecord>();

  constructor(private readonly registry: ChannelAdapterRegistry) {}

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
  }

  async health(instanceId: string): Promise<AdapterHealth> {
    return this.getInstance(instanceId).adapter.health(instanceId);
  }

  async drainEvents(instanceId: string, limit: number): Promise<NormalizedImEvent[]> {
    return this.getInstance(instanceId).adapter.drainEvents(instanceId, limit);
  }

  async sendMessage(
    instanceId: string,
    req: SendMessageRequest,
  ): Promise<SendMessageResult> {
    return this.getInstance(instanceId).adapter.sendMessage(instanceId, req);
  }

  async ack(instanceId: string, req: AckRequest): Promise<void> {
    await this.getInstance(instanceId).adapter.ack(instanceId, req);
  }

  private getInstance(instanceId: string): InstanceRecord {
    const record = this.instances.get(instanceId);
    if (!record) {
      throw new Error(`unknown adapter instance: ${instanceId}`);
    }
    return record;
  }
}

export { ChannelAdapterRegistry, createChannelAdapterRegistry };
