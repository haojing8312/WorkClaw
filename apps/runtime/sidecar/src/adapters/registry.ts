import type { ChannelAdapter } from "./types.js";

export class ChannelAdapterRegistry {
  private readonly adapters = new Map<string, ChannelAdapter>();

  register(name: string, adapter: ChannelAdapter): void {
    this.adapters.set(name, adapter);
  }

  get(name: string): ChannelAdapter | undefined {
    return this.adapters.get(name);
  }
}

export function createChannelAdapterRegistry(): ChannelAdapterRegistry {
  return new ChannelAdapterRegistry();
}
