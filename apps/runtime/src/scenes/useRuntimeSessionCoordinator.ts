import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect } from "react";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import { reportFrontendDiagnostic } from "../diagnostics";
import type { SessionInfo } from "../types";

type WorkTab =
  | {
      id: string;
      kind: "start-task";
    }
  | {
      id: string;
      kind: "session";
      sessionId: string;
    };

type SessionLaunchMode = "general" | "employee_direct" | "team_entry";

const DEFAULT_SESSION_TITLE = "New Chat";
const SESSION_LIST_RETRY_DELAY_MS = 250;
const SESSION_LIST_MAX_RETRIES = 2;

function extractErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message || fallback;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }
  return fallback;
}

function isSqliteLockedError(error: unknown): boolean {
  return extractErrorMessage(error, "").toLowerCase().includes("database is locked");
}

function canonicalizeSessionTitle(value: string): string {
  return (value || "")
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "");
}

const GENERIC_SESSION_TITLES = new Set([
  "",
  "newchat",
  "hi",
  "hello",
  "hey",
  "start",
  "continue",
  "continueprevious",
  "continuefrombefore",
  "helpme",
  "needhelp",
  "你好",
  "您好",
  "在吗",
  "继续",
  "开始",
  "帮我一下",
  "帮我处理",
  "请帮我一下",
  "继续上次",
  "继续刚才",
]);

function normalizeCandidateSessionTitle(value?: string): string | null {
  const collapsed = (value || "").trim().replace(/\s+/g, " ");
  const normalized = collapsed.replace(/^[\s,.:;!?，。：；！？、…·|/\\'"()[\]{}-]+|[\s,.:;!?，。：；！？、…·|/\\'"()[\]{}-]+$/g, "");
  if (!normalized) {
    return null;
  }
  if (GENERIC_SESSION_TITLES.has(canonicalizeSessionTitle(normalized))) {
    return null;
  }
  return normalized.slice(0, 28).trim() || null;
}

function resolveOptimisticDisplayTitle(input: {
  title?: string;
  initialUserMessage?: string;
  sessionMode: SessionLaunchMode;
}): string {
  if (input.sessionMode === "team_entry") {
    return normalizeCandidateSessionTitle(input.title) || input.title?.trim() || DEFAULT_SESSION_TITLE;
  }
  const explicitTitle = normalizeCandidateSessionTitle(input.title);
  if (explicitTitle) {
    return explicitTitle;
  }
  const derivedTitle = normalizeCandidateSessionTitle(input.initialUserMessage);
  if (derivedTitle) {
    return derivedTitle;
  }
  return input.title?.trim() || DEFAULT_SESSION_TITLE;
}

function buildOptimisticSession(input: {
  sessionId: string;
  skillId: string;
  modelId: string;
  title?: string;
  initialUserMessage?: string;
  employeeId?: string;
  sessionMode: SessionLaunchMode;
  teamId?: string;
  workDir?: string;
}): SessionInfo {
  const fallbackTitle = (input.title || "").trim() || DEFAULT_SESSION_TITLE;
  return {
    id: input.sessionId,
    skill_id: input.skillId,
    title: fallbackTitle,
    display_title: resolveOptimisticDisplayTitle({
      title: input.title,
      initialUserMessage: input.initialUserMessage,
      sessionMode: input.sessionMode,
    }),
    created_at: new Date().toISOString(),
    model_id: input.modelId,
    employee_id: input.employeeId || "",
    optimistic: true,
    session_mode: input.sessionMode,
    team_id: input.teamId || "",
    permission_mode: "standard",
    permission_mode_label: "标准模式",
    source_channel: "local",
    source_label: "",
    work_dir: (input.workDir || "").trim(),
  };
}

function mergeSessionInfo(list: SessionInfo[], session: SessionInfo): SessionInfo[] {
  const existing = list.find((item) => item.id === session.id);
  const merged: SessionInfo = existing
    ? {
        ...existing,
        ...session,
        skill_id: session.skill_id ?? existing.skill_id,
        work_dir: session.work_dir ?? existing.work_dir,
        employee_id: session.employee_id ?? existing.employee_id,
        employee_name: session.employee_name ?? existing.employee_name,
        permission_mode: session.permission_mode ?? existing.permission_mode,
        session_mode: session.session_mode ?? existing.session_mode,
        team_id: session.team_id ?? existing.team_id,
        source_channel: session.source_channel ?? existing.source_channel,
        source_label: session.source_label ?? existing.source_label,
      }
    : session;
  const withoutTarget = list.filter((item) => item.id !== session.id);
  return [merged, ...withoutTarget];
}

export function useRuntimeSessionCoordinator(options: {
  selectedSkillId: string | null;
  operationPermissionMode: string;
  activeTab:
    | {
        id: string;
        kind: "start-task";
      }
    | {
        id: string;
        kind: "session";
        sessionId: string;
      }
    | null;
  visibleSessions: SessionInfo[];
  loadSessionsRequestIdRef: MutableRefObject<number>;
  hasLoadedSessionsRef: MutableRefObject<boolean>;
  openFreshStartTaskTab: () => string;
  openStartTaskInActiveTab: () => void;
  createStartTaskTab: (id?: string) => WorkTab;
  createSessionTab: (sessionId: string, id?: string) => WorkTab;
  isSessionBlockingStartTaskReuse: (
    session: SessionInfo | null | undefined,
    sessionId?: string | null,
  ) => boolean;
  setSessions: Dispatch<SetStateAction<SessionInfo[]>>;
  setTabs: Dispatch<SetStateAction<WorkTab[]>>;
  setActiveTabId: Dispatch<SetStateAction<string>>;
}) {
  const {
    activeTab,
    createSessionTab,
    createStartTaskTab,
    hasLoadedSessionsRef,
    isSessionBlockingStartTaskReuse,
    loadSessionsRequestIdRef,
    openFreshStartTaskTab,
    openStartTaskInActiveTab,
    operationPermissionMode,
    selectedSkillId,
    setActiveTabId,
    setSessions,
    setTabs,
    visibleSessions,
  } = options;

  const createRuntimeSession = useCallback(
    async (input: {
      skillId: string;
      modelId: string;
      workDir?: string;
      employeeId?: string;
      title?: string;
      sessionMode: SessionLaunchMode;
      teamId?: string;
    }) => {
      return invoke<string>("create_session", {
        skillId: input.skillId,
        modelId: input.modelId,
        workDir: input.workDir || "",
        employeeId: input.employeeId || "",
        title: input.title,
        permissionMode: operationPermissionMode,
        sessionMode: input.sessionMode,
        teamId: input.sessionMode === "team_entry" ? input.teamId || "" : "",
      });
    },
    [operationPermissionMode],
  );

  const appendOptimisticSession = useCallback(
    (input: {
      sessionId: string;
      skillId: string;
      modelId: string;
      title?: string;
      initialUserMessage?: string;
      employeeId?: string;
      sessionMode: SessionLaunchMode;
      teamId?: string;
      workDir?: string;
    }) => {
      setSessions((prev) =>
        mergeSessionInfo(
          prev,
          buildOptimisticSession({
            sessionId: input.sessionId,
            skillId: input.skillId,
            modelId: input.modelId,
            title: input.title,
            initialUserMessage: input.initialUserMessage,
            employeeId: input.employeeId,
            sessionMode: input.sessionMode,
            teamId: input.teamId,
            workDir: input.workDir,
          }),
        ),
      );
    },
    [setSessions],
  );

  const activateSessionTab = useCallback(
    (sessionId: string, tabId: string) => {
      setTabs((prev) =>
        prev.map((tab) =>
          tab.id === tabId ? createSessionTab(sessionId, tabId) : tab,
        ),
      );
      setActiveTabId(tabId);
    },
    [createSessionTab, setActiveTabId, setTabs],
  );

  const prepareTabForNewTask = useCallback(() => {
    if (!activeTab) {
      const fallback = createStartTaskTab();
      setTabs([fallback]);
      setActiveTabId(fallback.id);
      return fallback.id;
    }
    if (activeTab.kind === "session") {
      const currentSession = visibleSessions.find((item) => item.id === activeTab.sessionId);
      if (isSessionBlockingStartTaskReuse(currentSession, activeTab.sessionId)) {
        return openFreshStartTaskTab();
      }
      openStartTaskInActiveTab();
    }
    return activeTab.id;
  }, [
    activeTab,
    createStartTaskTab,
    isSessionBlockingStartTaskReuse,
    openFreshStartTaskTab,
    openStartTaskInActiveTab,
    setActiveTabId,
    setTabs,
    visibleSessions,
  ]);

  const loadSessions = useCallback(
    async (_skillId: string, options?: { requestId?: number; attempt?: number }) => {
      const requestId = options?.requestId ?? ++loadSessionsRequestIdRef.current;
      const attempt = options?.attempt ?? 0;
      try {
        const list = await invoke<SessionInfo[]>("list_sessions");
        if (requestId !== loadSessionsRequestIdRef.current) {
          return;
        }
        hasLoadedSessionsRef.current = true;
        setSessions((prev) => {
          const previousById = new Map(prev.map((session) => [session.id, session]));
          let next = (Array.isArray(list) ? list : []).map((session) => {
            const previous = previousById.get(session.id);
            if (!previous) {
              return session;
            }
            return mergeSessionInfo([previous], session)[0];
          });
          for (const session of prev) {
            if (session.optimistic && !next.some((item) => item.id === session.id)) {
              next = mergeSessionInfo(next, session);
            }
          }
          return next;
        });
      } catch (error) {
        if (requestId !== loadSessionsRequestIdRef.current) {
          return;
        }
        if (isSqliteLockedError(error) && attempt < SESSION_LIST_MAX_RETRIES) {
          window.setTimeout(() => {
            void loadSessions(_skillId, { requestId, attempt: attempt + 1 });
          }, SESSION_LIST_RETRY_DELAY_MS);
          return;
        }
        console.error("加载会话列表失败:", error);
        void reportFrontendDiagnostic({
          kind: "session_list_load_failed",
          message: extractErrorMessage(error, "加载会话列表失败"),
          href: typeof window !== "undefined" ? window.location?.href : undefined,
        });
      }
    },
    [hasLoadedSessionsRef, loadSessionsRequestIdRef, setSessions],
  );

  useEffect(() => {
    if (selectedSkillId) {
      void loadSessions(selectedSkillId);
    } else {
      setSessions([]);
    }
  }, [loadSessions, selectedSkillId, setSessions]);

  return {
    activateSessionTab,
    appendOptimisticSession,
    createRuntimeSession,
    loadSessions,
    prepareTabForNewTask,
  };
}
