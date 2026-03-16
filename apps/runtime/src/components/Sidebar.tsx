import { useEffect, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  BrainCog,
  CheckCircle2,
  CircleAlert,
  CirclePlay,
  Download,
  History,
  LoaderCircle,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  Settings2,
  ShieldAlert,
  Trash2,
  Users,
} from "lucide-react";
import { SessionInfo } from "../types";
import workclawLogo from "../assets/branding/workclaw-logo.png";

interface Props {
  activeMainView: "start-task" | "experts" | "experts-new" | "packaging" | "employees";
  onOpenStartTask: () => void;
  onOpenExperts: () => void;
  onOpenEmployees: () => void;
  selectedSkillId: string | null;
  sessions: SessionInfo[];
  selectedSessionId: string | null;
  onSelectSession: (id: string) => void;
  onDeleteSession: (id: string) => void;
  onSettings: () => void;
  onSearchSessions: (query: string) => void;
  onExportSession: (sessionId: string) => void;
  onCollapse: () => void;
  collapsed: boolean;
}

export function Sidebar({
  activeMainView,
  onOpenStartTask,
  onOpenExperts,
  onOpenEmployees,
  selectedSkillId,
  sessions,
  selectedSessionId,
  onSelectSession,
  onDeleteSession,
  onSettings,
  onSearchSessions,
  onExportSession,
  onCollapse,
  collapsed,
}: Props) {
  const [searchQuery, setSearchQuery] = useState("");
  const sessionListRef = useRef<HTMLDivElement | null>(null);

  const isStartTask = activeMainView === "start-task";
  const isExperts = activeMainView === "experts" || activeMainView === "experts-new";
  const isEmployees = activeMainView === "employees";
  const iconClassName = "h-4 w-4 flex-shrink-0";

  function handleSearchChange(value: string) {
    setSearchQuery(value);
    onSearchSessions(value);
  }

  function getSessionDisplayTitle(session: SessionInfo): string {
    return (session.display_title || session.title || "").trim() || "未命名任务";
  }

  function getSessionRuntimeStatusMeta(session: SessionInfo): {
    label: string;
    className: string;
    icon: JSX.Element;
  } | null {
    const runtimeStatus = (session.runtime_status || "").trim().toLowerCase();
    if (runtimeStatus === "running") {
      return {
        label: "执行中",
        className: "text-blue-600",
        icon: <LoaderCircle className="h-3.5 w-3.5 animate-spin" />,
      };
    }
    if (runtimeStatus === "waiting_approval") {
      return {
        label: "等待确认",
        className: "text-amber-600",
        icon: <ShieldAlert className="h-3.5 w-3.5" />,
      };
    }
    if (runtimeStatus === "completed") {
      return {
        label: "已完成",
        className: "text-emerald-600",
        icon: <CheckCircle2 className="h-3.5 w-3.5" />,
      };
    }
    if (runtimeStatus === "failed") {
      return {
        label: "执行失败",
        className: "text-rose-600",
        icon: <CircleAlert className="h-3.5 w-3.5" />,
      };
    }
    return null;
  }

  useEffect(() => {
    if (collapsed || !selectedSessionId) {
      return;
    }
    const selectedItem = sessionListRef.current?.querySelector<HTMLElement>(
      `[data-session-id="${selectedSessionId}"]`
    );
    if (!selectedItem) {
      return;
    }
    selectedItem.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [collapsed, selectedSessionId, sessions]);

  if (collapsed) {
    return (
      <div className="sm-surface sm-divider w-12 flex flex-col h-full border-r items-center py-3 gap-3 flex-shrink-0">
        <button
          onClick={onCollapse}
          className="sm-btn sm-btn-ghost w-8 h-8 rounded-md"
          title="展开侧边栏"
          aria-label="展开侧边栏"
        >
          <PanelLeftOpen className={iconClassName} />
        </button>
        <button
          onClick={onOpenStartTask}
          className={`sm-btn w-8 h-8 rounded-md ${
            isStartTask ? "sm-btn-primary" : "sm-btn-ghost"
          }`}
          title="开始任务"
          aria-label="开始任务"
        >
          <CirclePlay className={iconClassName} />
        </button>
        <button
          onClick={onOpenExperts}
          className={`sm-btn w-8 h-8 rounded-md ${
            isExperts ? "sm-btn-primary" : "sm-btn-ghost"
          }`}
          title="专家技能"
          aria-label="专家技能"
        >
          <BrainCog className={iconClassName} />
        </button>
        <button
          onClick={onOpenEmployees}
          className={`sm-btn w-8 h-8 rounded-md ${
            isEmployees ? "sm-btn-primary" : "sm-btn-ghost"
          }`}
          title="智能体员工"
          aria-label="智能体员工"
        >
          <Users className={iconClassName} />
        </button>
        <button
          onClick={onSettings}
          className="sm-btn sm-btn-ghost w-8 h-8 rounded-md mt-auto"
          title="设置"
          aria-label="设置"
        >
          <Settings2 className={iconClassName} />
        </button>
      </div>
    );
  }

  return (
    <div className="sm-surface sm-divider w-64 flex flex-col h-full border-r flex-shrink-0">
      <div className="sm-surface sm-divider px-4 py-3 text-xs font-medium sm-text-muted border-b flex items-center justify-between">
        <img
          src={workclawLogo}
          alt="WorkClaw Logo"
          className="h-8 w-8 flex-shrink-0 object-contain"
        />
        <button
          onClick={onCollapse}
          className="sm-btn sm-btn-ghost h-7 w-7 text-sm rounded-md"
          title="折叠侧边栏"
          aria-label="折叠侧边栏"
        >
          <PanelLeftClose className="h-4 w-4" />
        </button>
      </div>

      <div className="sm-divider px-3 py-3 border-b">
        <div className="flex flex-col gap-1.5">
          <button
            onClick={onOpenStartTask}
            aria-pressed={isStartTask}
            className={
              "sm-btn w-full justify-start text-[13px] font-medium py-2 px-2 rounded-md " +
              (isStartTask ? "sm-btn-primary" : "sm-btn-secondary")
            }
          >
            <CirclePlay className={iconClassName} />
            开始任务
          </button>
          <button
            onClick={onOpenExperts}
            aria-pressed={isExperts}
            className={
              "sm-btn w-full justify-start text-[13px] font-medium py-2 px-2 rounded-md " +
              (isExperts ? "sm-btn-primary" : "sm-btn-secondary")
            }
          >
            <BrainCog className={iconClassName} />
            专家技能
          </button>
          <button
            onClick={onOpenEmployees}
            aria-pressed={isEmployees}
            className={
              "sm-btn w-full justify-start text-[13px] font-medium py-2 px-2 rounded-md " +
              (isEmployees ? "sm-btn-primary" : "sm-btn-secondary")
            }
          >
            <Users className={iconClassName} />
            智能体员工
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-hidden">
        {selectedSkillId && (
          <div className="h-full flex flex-col">
            <div className="sm-divider px-4 py-2 text-xs font-medium sm-text-muted border-t border-b flex items-center gap-1.5">
              <History className="h-3.5 w-3.5" />
              <span>会话历史</span>
            </div>
            <div className="sm-divider px-3 py-2 border-b">
              <div className="relative">
                <Search className="h-3.5 w-3.5 absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none sm-text-muted" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => handleSearchChange(e.target.value)}
                  placeholder="搜索会话..."
                  className="sm-input w-full py-1 text-xs pl-8"
                />
              </div>
            </div>
            <div ref={sessionListRef} className="flex-1 overflow-y-auto py-1">
              {sessions.length === 0 && (
                <div className="px-4 py-3 text-xs sm-text-muted">{searchQuery ? "未找到匹配会话" : "暂无会话"}</div>
              )}
              <AnimatePresence>
                {sessions.map((s) => {
                  const sourceLabel = (s.source_label || "").trim();
                  const sourceChannel = (s.source_channel || "").trim().toLowerCase();
                  const isImSession = sourceChannel.length > 0 && sourceChannel !== "local";
                  const badgeText = sourceLabel || sourceChannel || "IM";
                  const runtimeStatusMeta = getSessionRuntimeStatusMeta(s);
                  return (
                    <motion.div
                      key={s.id}
                      data-session-id={s.id}
                      initial={{ opacity: 0, x: -10 }}
                      animate={{ opacity: 1, x: 0 }}
                      exit={{ opacity: 0, x: -20, height: 0 }}
                      transition={{ duration: 0.2 }}
                      className={
                        "group flex items-center px-4 py-2 text-sm cursor-pointer rounded-md mx-1 transition-colors " +
                        (selectedSessionId === s.id ? "bg-[var(--sm-primary-soft)] text-[var(--sm-primary-strong)]" : "sm-text-primary hover:bg-[var(--sm-surface-soft)]")
                      }
                      onClick={() => onSelectSession(s.id)}
                    >
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 min-w-0">
                          {runtimeStatusMeta && (
                            <span
                              data-testid={`session-runtime-status-${s.id}`}
                              title={runtimeStatusMeta.label}
                              className={`inline-flex flex-shrink-0 ${runtimeStatusMeta.className}`}
                            >
                              {runtimeStatusMeta.icon}
                            </span>
                          )}
                          <div className="truncate text-[13px]">{getSessionDisplayTitle(s)}</div>
                          {isImSession && (
                            <span
                              title={`来自${badgeText}会话同步`}
                              className="inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium bg-blue-50 text-blue-700 border border-blue-100 flex-shrink-0"
                            >
                              {badgeText}
                            </span>
                          )}
                        </div>
                      </div>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onExportSession(s.id);
                        }}
                        className="hidden group-hover:inline-flex sm-btn sm-btn-ghost h-6 w-6 text-xs ml-1 flex-shrink-0"
                        title="导出会话"
                        aria-label="导出会话"
                      >
                        <Download className="h-3.5 w-3.5" />
                      </button>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onDeleteSession(s.id);
                        }}
                        className="hidden group-hover:inline-flex sm-btn sm-btn-danger h-6 w-6 text-xs ml-1 flex-shrink-0"
                        title="删除会话"
                        aria-label="删除会话"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </motion.div>
                  );
                })}
              </AnimatePresence>
            </div>
          </div>
        )}
      </div>

      <div className="sm-divider p-3 space-y-2 border-t">
        <button
          onClick={onSettings}
          className="sm-btn sm-btn-secondary w-full text-sm py-1.5 rounded-lg"
        >
          <Settings2 className={iconClassName} />
          设置
        </button>
      </div>
    </div>
  );
}
