import type { PersistedChatRuntimeState } from "../../types";
import { storageKey } from "../../lib/branding";

const SESSION_DRAFT_STORAGE_PREFIX = storageKey("session-draft");

export function getSessionDraftStorageKey(sessionId: string): string {
  return `${SESSION_DRAFT_STORAGE_PREFIX}:${sessionId}`;
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
    toolManifest: state?.toolManifest ? state.toolManifest.map((item) => ({ ...item })) : [],
    streamReasoning: state?.streamReasoning ? { ...state.streamReasoning } : null,
    agentState: state?.agentState ? { ...state.agentState } : null,
    subAgentBuffer: state?.subAgentBuffer ?? "",
    subAgentRoleName: state?.subAgentRoleName ?? "",
    mainRoleName: state?.mainRoleName ?? "",
    mainSummaryDelivered: state?.mainSummaryDelivered ?? false,
    delegationCards: state?.delegationCards ? state.delegationCards.map((item) => ({ ...item })) : [],
  };
}

function areComparableValuesEqual(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) {
    return true;
  }
  if (left === null || right === null || left === undefined || right === undefined) {
    return left === right;
  }
  if (typeof left !== typeof right) {
    return false;
  }
  if (typeof left !== "object") {
    return false;
  }
  if (Array.isArray(left) || Array.isArray(right)) {
    if (!Array.isArray(left) || !Array.isArray(right) || left.length !== right.length) {
      return false;
    }
    return left.every((item, index) => areComparableValuesEqual(item, right[index]));
  }

  const leftRecord = left as Record<string, unknown>;
  const rightRecord = right as Record<string, unknown>;
  const leftKeys = Object.keys(leftRecord);
  const rightKeys = Object.keys(rightRecord);
  if (leftKeys.length !== rightKeys.length) {
    return false;
  }
  return leftKeys.every((key) => rightKeys.includes(key) && areComparableValuesEqual(leftRecord[key], rightRecord[key]));
}

export function arePersistedChatRuntimeStatesEqual(
  left?: PersistedChatRuntimeState | null,
  right?: PersistedChatRuntimeState | null,
): boolean {
  if (!left || !right) {
    return left === right;
  }
  return (
    left.streaming === right.streaming &&
    areComparableValuesEqual(left.streamItems, right.streamItems) &&
    areComparableValuesEqual(left.toolManifest, right.toolManifest) &&
    areComparableValuesEqual(left.streamReasoning, right.streamReasoning) &&
    areComparableValuesEqual(left.agentState, right.agentState) &&
    left.subAgentBuffer === right.subAgentBuffer &&
    left.subAgentRoleName === right.subAgentRoleName &&
    left.mainRoleName === right.mainRoleName &&
    left.mainSummaryDelivered === right.mainSummaryDelivered &&
    areComparableValuesEqual(left.delegationCards, right.delegationCards)
  );
}
