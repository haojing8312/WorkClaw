import { act, renderHook } from "@testing-library/react";

import type { PersistedChatRuntimeState } from "../../types";
import { useSessionRuntimeStateCoordinator } from "../useSessionRuntimeStateCoordinator";

function createRuntimeState(overrides: Partial<PersistedChatRuntimeState> = {}): PersistedChatRuntimeState {
  return {
    streaming: false,
    streamItems: [],
    toolManifest: [],
    streamReasoning: null,
    agentState: null,
    subAgentBuffer: "",
    subAgentRoleName: "",
    mainRoleName: "",
    mainSummaryDelivered: false,
    delegationCards: [],
    ...overrides,
  };
}

describe("useSessionRuntimeStateCoordinator", () => {
  test("does not replace stored runtime state when the snapshot is unchanged", () => {
    let sessionRuntimeStateById: Record<string, PersistedChatRuntimeState> = {};
    const setLiveSessionRuntimeStatusById = vi.fn();
    const setSessionRuntimeStateById = vi.fn((updater) => {
      sessionRuntimeStateById =
        typeof updater === "function" ? updater(sessionRuntimeStateById) : updater;
    });

    const { result } = renderHook(() =>
      useSessionRuntimeStateCoordinator({
        selectedSessionId: "sess-runtime",
        liveSessionRuntimeStatusById: {},
        setLiveSessionRuntimeStatusById,
        setSessionRuntimeStateById,
      }),
    );

    act(() => {
      result.current.handlePersistSessionRuntimeState("sess-runtime", createRuntimeState({
        streamItems: [{ type: "text", content: "已有输出" }],
      }));
    });

    const firstStoredState = sessionRuntimeStateById;

    act(() => {
      result.current.handlePersistSessionRuntimeState("sess-runtime", createRuntimeState({
        streamItems: [{ type: "text", content: "已有输出" }],
      }));
    });

    expect(sessionRuntimeStateById).toBe(firstStoredState);
  });

  test("stores a new snapshot when the runtime state changes", () => {
    let sessionRuntimeStateById: Record<string, PersistedChatRuntimeState> = {};
    const setLiveSessionRuntimeStatusById = vi.fn();
    const setSessionRuntimeStateById = vi.fn((updater) => {
      sessionRuntimeStateById =
        typeof updater === "function" ? updater(sessionRuntimeStateById) : updater;
    });

    const { result } = renderHook(() =>
      useSessionRuntimeStateCoordinator({
        selectedSessionId: "sess-runtime",
        liveSessionRuntimeStatusById: {},
        setLiveSessionRuntimeStatusById,
        setSessionRuntimeStateById,
      }),
    );

    act(() => {
      result.current.handlePersistSessionRuntimeState("sess-runtime", createRuntimeState());
    });

    const firstStoredState = sessionRuntimeStateById;

    act(() => {
      result.current.handlePersistSessionRuntimeState("sess-runtime", createRuntimeState({
        streaming: true,
        streamItems: [{ type: "text", content: "新的输出" }],
      }));
    });

    expect(sessionRuntimeStateById).not.toBe(firstStoredState);
    expect(sessionRuntimeStateById["sess-runtime"]).toMatchObject({
      streaming: true,
      streamItems: [{ type: "text", content: "新的输出" }],
    });
  });
});
