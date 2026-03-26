import type { PersistedChatRuntimeState } from "../../types";

const SESSION_DRAFT_STORAGE_PREFIX = "workclaw:session-draft:";

export function getSessionDraftStorageKey(sessionId: string): string {
  return `${SESSION_DRAFT_STORAGE_PREFIX}${sessionId}`;
}

export function readSessionDraft(sessionId: string): string {
  if (typeof window === "undefined" || !sessionId) {
    return "";
  }
  try {
    return window.localStorage.getItem(getSessionDraftStorageKey(sessionId)) ?? "";
  } catch {
    return "";
  }
}

export function persistSessionDraft(sessionId: string, value: string) {
  if (typeof window === "undefined" || !sessionId) {
    return;
  }
  try {
    if (value.length > 0) {
      window.localStorage.setItem(getSessionDraftStorageKey(sessionId), value);
      return;
    }
    window.localStorage.removeItem(getSessionDraftStorageKey(sessionId));
  } catch {
    // ignore localStorage failures
  }
}

export function clearSessionDraft(sessionId: string) {
  persistSessionDraft(sessionId, "");
}

export function clonePersistedChatRuntimeState(
  state?: PersistedChatRuntimeState | null,
): PersistedChatRuntimeState {
  return {
    streaming: state?.streaming ?? false,
    streamItems: state?.streamItems ? [...state.streamItems] : [],
    streamReasoning: state?.streamReasoning ? { ...state.streamReasoning } : null,
    agentState: state?.agentState ? { ...state.agentState } : null,
    subAgentBuffer: state?.subAgentBuffer ?? "",
    subAgentRoleName: state?.subAgentRoleName ?? "",
    mainRoleName: state?.mainRoleName ?? "",
    mainSummaryDelivered: state?.mainSummaryDelivered ?? false,
    delegationCards: state?.delegationCards ? state.delegationCards.map((item) => ({ ...item })) : [],
  };
}
