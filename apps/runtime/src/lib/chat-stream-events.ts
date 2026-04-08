import { listen } from "@tauri-apps/api/event";

export type StreamTokenEvent = {
  session_id: string;
  token: string;
  done: boolean;
  sub_agent?: boolean;
  role_id?: string;
  role_name?: string;
};

export type AssistantReasoningStartedEvent = {
  session_id: string;
};

export type AssistantReasoningDeltaEvent = {
  session_id: string;
  text: string;
};

export type AssistantReasoningCompletedEvent = {
  session_id: string;
  duration_ms?: number;
};

export type AssistantReasoningInterruptedEvent = {
  session_id: string;
};

export type ToolCallEvent = {
  session_id: string;
  tool_name: string;
  tool_input: Record<string, unknown>;
  tool_output: string | null;
  status: string;
};

export type SessionToolManifestEvent = {
  session_id: string;
  manifest: Array<{
    name: string;
    description: string;
    display_name: string;
    category: string;
    read_only: boolean;
    destructive: boolean;
    concurrency_safe: boolean;
    open_world: boolean;
    requires_approval: boolean;
    source: string;
  }>;
};

type EventMap = {
  "stream-token": StreamTokenEvent;
  "assistant-reasoning-started": AssistantReasoningStartedEvent;
  "assistant-reasoning-delta": AssistantReasoningDeltaEvent;
  "assistant-reasoning-completed": AssistantReasoningCompletedEvent;
  "assistant-reasoning-interrupted": AssistantReasoningInterruptedEvent;
  "tool-call-event": ToolCallEvent;
  "session-tool-manifest": SessionToolManifestEvent;
};

type EventName = keyof EventMap;
type Subscriber<T> = (payload: T) => void;

type RegistryEntry<T> = {
  started: boolean;
  startPromise: Promise<void> | null;
  stopRawListener: (() => void) | null;
  subscribers: Set<Subscriber<T>>;
};

function createEntry<T>(): RegistryEntry<T> {
  return {
    started: false,
    startPromise: null,
    stopRawListener: null,
    subscribers: new Set<Subscriber<T>>(),
  };
}

const registry: { [K in EventName]: RegistryEntry<EventMap[K]> } = {
  "stream-token": createEntry<StreamTokenEvent>(),
  "assistant-reasoning-started": createEntry<AssistantReasoningStartedEvent>(),
  "assistant-reasoning-delta": createEntry<AssistantReasoningDeltaEvent>(),
  "assistant-reasoning-completed": createEntry<AssistantReasoningCompletedEvent>(),
  "assistant-reasoning-interrupted": createEntry<AssistantReasoningInterruptedEvent>(),
  "tool-call-event": createEntry<ToolCallEvent>(),
  "session-tool-manifest": createEntry<SessionToolManifestEvent>(),
};

function ensureRawListener<K extends EventName>(eventName: K) {
  const entry = registry[eventName];
  if (entry.started) {
    return;
  }
  entry.started = true;
  entry.startPromise = listen<EventMap[K]>(eventName, ({ payload }) => {
    const snapshot = Array.from(entry.subscribers);
    for (const subscriber of snapshot) {
      subscriber(payload);
    }
  })
    .then((unlisten) => {
      entry.stopRawListener = unlisten;
    })
    .catch((error) => {
      entry.started = false;
      entry.startPromise = null;
      entry.stopRawListener = null;
      console.error(`注册共享事件监听失败: ${eventName}`, error);
    });
}

export function subscribeChatStreamEvent<K extends EventName>(
  eventName: K,
  subscriber: Subscriber<EventMap[K]>,
) {
  const entry = registry[eventName];
  entry.subscribers.add(subscriber);
  ensureRawListener(eventName);

  return () => {
    entry.subscribers.delete(subscriber);
  };
}

export function resetChatStreamEventSubscriptionsForTest() {
  for (const eventName of Object.keys(registry) as EventName[]) {
    const entry = registry[eventName];
    entry.stopRawListener?.();
    entry.started = false;
    entry.startPromise = null;
    entry.stopRawListener = null;
    entry.subscribers.clear();
  }
}
