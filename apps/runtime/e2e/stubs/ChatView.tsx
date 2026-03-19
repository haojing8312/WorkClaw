import { useEffect } from "react";

type ChatViewProps = {
  sessionId: string;
  initialMessage?: string;
  persistedRuntimeState?: {
    streamItems?: Array<{ type: string; content?: string }>;
    agentState?: { state?: string };
  };
  onPersistRuntimeState?: (state: {
    streaming: boolean;
    streamItems: Array<{ type: string; content?: string }>;
    streamReasoning: { status: "thinking"; content: string };
    agentState: { state: string; iteration: number };
    subAgentBuffer: string;
    subAgentRoleName: string;
    mainRoleName: string;
    mainSummaryDelivered: boolean;
    delegationCards: unknown[];
  }) => void;
};

type PersistRuntimeCallback = NonNullable<ChatViewProps["onPersistRuntimeState"]>;

function buildPersistedRuntimeState(text: string) {
  return {
    streaming: true,
    streamItems: [{ type: "text", content: text }],
    streamReasoning: { status: "thinking" as const, content: "恢复中" },
    agentState: { state: "thinking", iteration: 1 },
    subAgentBuffer: "",
    subAgentRoleName: "",
    mainRoleName: "",
    mainSummaryDelivered: false,
    delegationCards: [],
  };
}

export function ChatView(props: ChatViewProps): JSX.Element {
  useEffect(() => {
    const globalWindow = window as typeof window & {
      __E2E_RUNTIME_CALLBACKS__?: Record<string, PersistRuntimeCallback>;
    };
    if (!globalWindow.__E2E_RUNTIME_CALLBACKS__) {
      globalWindow.__E2E_RUNTIME_CALLBACKS__ = {};
    }
    if (props.onPersistRuntimeState) {
      globalWindow.__E2E_RUNTIME_CALLBACKS__[props.sessionId] = props.onPersistRuntimeState;
    }
  }, [props.onPersistRuntimeState, props.sessionId]);

  return (
    <div data-testid="e2e-chat-view" className="h-full p-4">
      <div>chat-view-stub</div>
      <div data-testid="e2e-chat-session-id">{props.sessionId}</div>
      <div data-testid="e2e-chat-runtime-stream-text">
        {(props.persistedRuntimeState?.streamItems || [])
          .filter((item) => item.type === "text")
          .map((item) => item.content || "")
          .join("")}
      </div>
      <div data-testid="e2e-chat-runtime-agent-state">{props.persistedRuntimeState?.agentState?.state || ""}</div>
      {props.initialMessage ? (
        <div data-testid="e2e-chat-initial-message">{props.initialMessage}</div>
      ) : null}
      <button
        type="button"
        onClick={() => props.onPersistRuntimeState?.(buildPersistedRuntimeState("已缓存的运行中输出"))}
      >
        persist-runtime-state
      </button>
      <button
        type="button"
        onClick={() => {
          const globalWindow = window as typeof window & {
            __E2E_RUNTIME_CALLBACKS__?: Record<string, PersistRuntimeCallback>;
          };
          window.setTimeout(() => {
            globalWindow.__E2E_RUNTIME_CALLBACKS__?.[props.sessionId]?.(
              buildPersistedRuntimeState("已缓存的运行中输出 + 后台新增片段"),
            );
          }, 400);
        }}
      >
        append-runtime-state-later
      </button>
    </div>
  );
}
