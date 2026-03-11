import type { ChannelAdapter } from "./types.js";

export class ChannelAdapterRegistry {
  private readonly adapters = new Map<string, ChannelAdapter>();

  register(name: string, adapter: ChannelAdapter): void {
    this.adapters.set(name, adapter);
  }

  get(name: string): ChannelAdapter | undefined {
    return this.adapters.get(name);
  }

  entries(): Array<[string, ChannelAdapter]> {
    return Array.from(this.adapters.entries());
  }
}

export function createChannelAdapterRegistry(): ChannelAdapterRegistry {
  return new ChannelAdapterRegistry();
}
