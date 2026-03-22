import { useCallback, useEffect, useMemo } from "react";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import type { PersistedChatRuntimeState, SessionInfo, SkillManifest } from "../types";

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

function persistLastSelectedSessionId(sessionId: string | null) {
  if (typeof window === "undefined") {
    return;
  }
  try {
    if (sessionId && sessionId.trim()) {
      window.localStorage.setItem("workclaw:last-selected-session-id", sessionId.trim());
      return;
    }
    window.localStorage.removeItem("workclaw:last-selected-session-id");
  } catch {
    // ignore localStorage failures
  }
}

function persistLastSelectedSessionSnapshot(session: SessionInfo | null) {
  if (typeof window === "undefined") {
    return;
  }
  try {
    if (session?.id?.trim()) {
      window.localStorage.setItem(
        "workclaw:last-selected-session-snapshot",
        JSON.stringify(session),
      );
      return;
    }
    window.localStorage.removeItem("workclaw:last-selected-session-snapshot");
  } catch {
    // ignore localStorage failures
  }
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

export function useSessionViewCoordinator(options: {
  tabs: WorkTab[];
  activeTabId: string;
  sessions: SessionInfo[];
  skills: SkillManifest[];
  selectedSkillId: string | null;
  setSelectedSkillId: Dispatch<SetStateAction<string | null>>;
  setActiveTabId: Dispatch<SetStateAction<string>>;
  setTabs: Dispatch<SetStateAction<WorkTab[]>>;
  setLiveSessionRuntimeStatusById: Dispatch<SetStateAction<Record<string, string>>>;
  activeMainView: "start-task" | "experts" | "experts-new" | "packaging" | "employees";
  setCreateSessionError: Dispatch<SetStateAction<string | null>>;
  navigate: (view: "start-task" | "experts" | "experts-new" | "packaging" | "employees") => void;
  createStartTaskTab: (id?: string) => WorkTab;
  createSessionTab: (sessionId: string, id?: string) => WorkTab;
  initialSelectedSessionSnapshotRef: MutableRefObject<SessionInfo | null>;
  initialPersistedSessionIdRef: MutableRefObject<string | null>;
  hasLoadedSessionsRef: MutableRefObject<boolean>;
  hasResolvedInitialPersistedSessionRef: MutableRefObject<boolean>;
}) {
  const {
    activeMainView,
    activeTabId,
    createSessionTab,
    createStartTaskTab,
    hasLoadedSessionsRef,
    hasResolvedInitialPersistedSessionRef,
    initialPersistedSessionIdRef,
    initialSelectedSessionSnapshotRef,
    navigate,
    selectedSkillId,
    sessions,
    setActiveTabId,
    setCreateSessionError,
    setLiveSessionRuntimeStatusById,
    setSelectedSkillId,
    setTabs,
    skills,
    tabs,
  } = options;

  const activeTab = useMemo(
    () => tabs.find((item) => item.id === activeTabId) ?? tabs[0] ?? null,
    [activeTabId, tabs],
  );
  const selectedSessionId = activeTab?.kind === "session" ? activeTab.sessionId : null;
  const hydratedSessionSnapshot = !hasLoadedSessionsRef.current
    ? initialSelectedSessionSnapshotRef.current
    : null;
  const hydratedSelectedSession =
    hydratedSessionSnapshot &&
    selectedSessionId &&
    hydratedSessionSnapshot.id === selectedSessionId
      ? hydratedSessionSnapshot
      : null;
  const visibleSessions = useMemo(() => {
    if (!hydratedSessionSnapshot) {
      return sessions;
    }
    return mergeSessionInfo(sessions, hydratedSessionSnapshot);
  }, [hydratedSessionSnapshot, sessions]);

  const replaceTab = useCallback(
    (tabId: string, nextTab: WorkTab) => {
      setTabs((prev) => prev.map((tab) => (tab.id === tabId ? nextTab : tab)));
    },
    [setTabs],
  );

  const replaceActiveTab = useCallback(
    (nextTab: WorkTab) => {
      if (!activeTab) {
        setTabs([nextTab]);
        setActiveTabId(nextTab.id);
        return;
      }
      replaceTab(activeTab.id, { ...nextTab, id: activeTab.id });
      setActiveTabId(activeTab.id);
    },
    [activeTab, replaceTab, setActiveTabId, setTabs],
  );

  const openSessionInActiveTab = useCallback(
    (sessionId: string) => {
      replaceActiveTab(createSessionTab(sessionId, activeTab?.id));
    },
    [activeTab?.id, createSessionTab, replaceActiveTab],
  );

  const openStartTaskInActiveTab = useCallback(() => {
    replaceActiveTab(createStartTaskTab(activeTab?.id));
  }, [activeTab?.id, createStartTaskTab, replaceActiveTab]);

  const openFreshStartTaskTab = useCallback(() => {
    const nextTab = createStartTaskTab();
    setTabs((prev) => [...prev, nextTab]);
    setActiveTabId(nextTab.id);
    return nextTab.id;
  }, [createStartTaskTab, setActiveTabId, setTabs]);

  const closeTaskTab = useCallback(
    (tabId: string) => {
      const closingTab = tabs.find((tab) => tab.id === tabId);
      setTabs((prev) => {
        if (prev.length <= 1) {
          const fallback = createStartTaskTab();
          setActiveTabId(fallback.id);
          return [fallback];
        }
        const index = prev.findIndex((tab) => tab.id === tabId);
        const nextTabs = prev.filter((tab) => tab.id !== tabId);
        if (tabId === activeTabId) {
          const fallbackTab =
            nextTabs[index] ?? nextTabs[index - 1] ?? nextTabs[0];
          if (fallbackTab) {
            setActiveTabId(fallbackTab.id);
          }
        }
        return nextTabs;
      });
      if (closingTab?.kind === "session") {
        setLiveSessionRuntimeStatusById((prev) => {
          if (!prev[closingTab.sessionId]) return prev;
          const next = { ...prev };
          delete next[closingTab.sessionId];
          return next;
        });
      }
    },
    [
      activeTabId,
      createStartTaskTab,
      setActiveTabId,
      setLiveSessionRuntimeStatusById,
      setTabs,
      tabs,
    ],
  );

  const handleSelectSession = useCallback(
    (sessionId: string, options?: { openChatView?: boolean }) => {
      const targetSession = visibleSessions.find((item) => item.id === sessionId);
      const targetSkillId = (targetSession?.skill_id || "").trim();
      if (
        targetSkillId &&
        skills.some((item) => item.id === targetSkillId)
      ) {
        setSelectedSkillId(targetSkillId);
      }
      openSessionInActiveTab(sessionId);
      setCreateSessionError(null);
      if (options?.openChatView !== false) {
        navigate("start-task");
      }
    },
    [
      navigate,
      openSessionInActiveTab,
      setCreateSessionError,
      setSelectedSkillId,
      skills,
      visibleSessions,
    ],
  );

  useEffect(() => {
    persistLastSelectedSessionId(selectedSessionId);
  }, [selectedSessionId]);

  useEffect(() => {
    if (!tabs.some((item) => item.id === activeTabId) && tabs[0]) {
      setActiveTabId(tabs[0].id);
    }
  }, [activeTabId, setActiveTabId, tabs]);

  useEffect(() => {
    if (!selectedSessionId || skills.length === 0) {
      return;
    }

    const activeSession = visibleSessions.find((item) => item.id === selectedSessionId);
    if (activeSession) {
      if (
        selectedSessionId === initialPersistedSessionIdRef.current &&
        (hasLoadedSessionsRef.current ||
          activeSession.id !== hydratedSelectedSession?.id)
      ) {
        hasResolvedInitialPersistedSessionRef.current = true;
      }

      const targetSkillId = (activeSession.skill_id || "").trim();
      if (
        targetSkillId &&
        targetSkillId !== selectedSkillId &&
        skills.some((item) => item.id === targetSkillId)
      ) {
        setSelectedSkillId(targetSkillId);
      }
      return;
    }

    if (
      hasLoadedSessionsRef.current &&
      selectedSessionId === initialPersistedSessionIdRef.current &&
      !hasResolvedInitialPersistedSessionRef.current
    ) {
      hasResolvedInitialPersistedSessionRef.current = true;
      openStartTaskInActiveTab();
    }
  }, [
    hasLoadedSessionsRef,
    hasResolvedInitialPersistedSessionRef,
    hydratedSelectedSession?.id,
    initialPersistedSessionIdRef,
    openStartTaskInActiveTab,
    selectedSessionId,
    selectedSkillId,
    setSelectedSkillId,
    skills,
    visibleSessions,
  ]);

  useEffect(() => {
    persistLastSelectedSessionSnapshot(
      selectedSessionId ? visibleSessions.find((s) => s.id === selectedSessionId) ?? hydratedSelectedSession : null,
    );
  }, [hydratedSelectedSession, selectedSessionId, visibleSessions]);

  return {
    activeTab,
    closeTaskTab,
    handleSelectSession,
    hydratedSelectedSession,
    openFreshStartTaskTab,
    openSessionInActiveTab,
    openStartTaskInActiveTab,
    replaceTab,
    selectedSessionId,
    visibleSessions,
  };
}
