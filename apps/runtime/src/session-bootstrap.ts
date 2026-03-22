import type { SessionInfo } from "./types";

const DEFAULT_SESSION_TITLE = "New Chat";
const LAST_SELECTED_SESSION_ID_KEY = "workclaw:last-selected-session-id";
const LAST_SELECTED_SESSION_SNAPSHOT_KEY = "workclaw:last-selected-session-snapshot";

export type WorkTab =
  | {
      id: string;
      kind: "start-task";
    }
  | {
      id: string;
      kind: "session";
      sessionId: string;
    };

export function createWorkTabId(prefix: "start-task" | "session" = "start-task"): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function createStartTaskTab(id = createWorkTabId("start-task")): WorkTab {
  return {
    id,
    kind: "start-task",
  };
}

export function createSessionTab(sessionId: string, id = createWorkTabId("session")): WorkTab {
  return {
    id,
    kind: "session",
    sessionId,
  };
}

export function readPersistedLastSelectedSessionId(): string | null {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    const value = window.localStorage.getItem(LAST_SELECTED_SESSION_ID_KEY);
    return value?.trim() || null;
  } catch {
    return null;
  }
}

export function readPersistedLastSelectedSessionSnapshot(): SessionInfo | null {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    const raw = window.localStorage.getItem(LAST_SELECTED_SESSION_SNAPSHOT_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as SessionInfo | null;
    if (!parsed || typeof parsed !== "object") {
      return null;
    }
    const sessionId = typeof parsed.id === "string" ? parsed.id.trim() : "";
    if (!sessionId) {
      return null;
    }
    return {
      ...parsed,
      id: sessionId,
      title: typeof parsed.title === "string" && parsed.title.trim() ? parsed.title.trim() : DEFAULT_SESSION_TITLE,
      display_title:
        typeof parsed.display_title === "string" && parsed.display_title.trim()
          ? parsed.display_title.trim()
          : undefined,
      created_at: typeof parsed.created_at === "string" ? parsed.created_at : "",
      model_id: typeof parsed.model_id === "string" ? parsed.model_id : "",
      skill_id: typeof parsed.skill_id === "string" ? parsed.skill_id.trim() : "",
      session_mode:
        typeof parsed.session_mode === "string" && parsed.session_mode.trim()
          ? (parsed.session_mode as SessionInfo["session_mode"])
          : "general",
      team_id: typeof parsed.team_id === "string" ? parsed.team_id : "",
    };
  } catch {
    return null;
  }
}
