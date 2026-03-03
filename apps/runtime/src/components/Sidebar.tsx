import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { SessionInfo } from "../types";

interface Props {
  activeMainView: "start-task" | "experts" | "experts-new" | "packaging" | "employees";
  onOpenStartTask: () => void;
  onOpenExperts: () => void;
  onOpenEmployees: () => void;
  selectedSkillId: string | null;
  sessions: SessionInfo[];
  selectedSessionId: string | null;
  onSelectSession: (id: string) => void;
  newSessionPermissionMode: "default" | "accept_edits" | "unrestricted";
  onChangeNewSessionPermissionMode: (mode: "default" | "accept_edits" | "unrestricted") => void;
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
  newSessionPermissionMode,
  onChangeNewSessionPermissionMode,
  onDeleteSession,
  onSettings,
  onSearchSessions,
  onExportSession,
  onCollapse,
  collapsed,
}: Props) {
  const [searchQuery, setSearchQuery] = useState("");

  const isStartTask = activeMainView === "start-task";
  const isExperts = activeMainView === "experts" || activeMainView === "experts-new";
  const isEmployees = activeMainView === "employees";

  function handleSearchChange(value: string) {
    setSearchQuery(value);
    onSearchSessions(value);
  }

  if (collapsed) {
    return (
      <div className="sm-surface sm-divider w-12 flex flex-col h-full border-r items-center py-3 gap-3 flex-shrink-0">
        <button
          onClick={onCollapse}
          className="sm-btn sm-btn-ghost w-8 h-8 rounded-md"
          title="展开侧边栏"
          aria-label="展开侧边栏"
        >
          ▶
        </button>
        <button
          onClick={onOpenStartTask}
          className={`sm-btn w-8 h-8 rounded-md ${
            isStartTask ? "sm-btn-primary" : "sm-btn-ghost"
          }`}
          title="开始任务"
          aria-label="开始任务"
        >
          ○
        </button>
        <button
          onClick={onOpenExperts}
          className={`sm-btn w-8 h-8 rounded-md ${
            isExperts ? "sm-btn-primary" : "sm-btn-ghost"
          }`}
          title="专家技能"
          aria-label="专家技能"
        >
          ◆
        </button>
        <button
          onClick={onOpenEmployees}
          className={`sm-btn w-8 h-8 rounded-md ${
            isEmployees ? "sm-btn-primary" : "sm-btn-ghost"
          }`}
          title="智能体员工"
          aria-label="智能体员工"
        >
          ◎
        </button>
        <button
          onClick={onSettings}
          className="sm-btn sm-btn-ghost w-8 h-8 rounded-md mt-auto"
          title="设置"
          aria-label="设置"
        >
          ⚙
        </button>
      </div>
    );
  }

  return (
    <div className="sm-surface sm-divider w-60 flex flex-col h-full border-r flex-shrink-0">
      <div className="sm-surface sm-divider px-4 py-3 text-xs font-medium sm-text-muted border-b flex items-center justify-between">
        <span>SkillMint</span>
        <button
          onClick={onCollapse}
          className="sm-btn sm-btn-ghost h-7 w-7 text-sm rounded-md"
          title="折叠侧边栏"
        >
          ◀
        </button>
      </div>

      <div className="sm-divider px-3 py-2 border-b">
        <div className="grid grid-cols-3 gap-2">
          <button
            onClick={onOpenStartTask}
            className={
              "sm-btn text-xs py-1.5 rounded-md " +
              (isStartTask ? "sm-btn-primary" : "sm-btn-secondary")
            }
          >
            开始任务
          </button>
          <button
            onClick={onOpenExperts}
            className={
              "sm-btn text-xs py-1.5 rounded-md " +
              (isExperts ? "sm-btn-primary" : "sm-btn-secondary")
            }
          >
            专家技能
          </button>
          <button
            onClick={onOpenEmployees}
            className={
              "sm-btn text-xs py-1.5 rounded-md " +
              (isEmployees ? "sm-btn-primary" : "sm-btn-secondary")
            }
          >
            智能体员工
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-hidden">
        {selectedSkillId && (
          <div className="h-full flex flex-col">
            <div className="sm-divider px-4 py-2 text-xs font-medium sm-text-muted border-t border-b">
              <span>会话历史</span>
            </div>
            <div className="sm-divider px-3 py-2 border-b">
              <label className="sm-field-label text-[11px]">操作确认级别</label>
              <select
                value={newSessionPermissionMode}
                onChange={(e) =>
                  onChangeNewSessionPermissionMode(e.target.value as "default" | "accept_edits" | "unrestricted")
                }
                className="sm-select w-full py-1 text-xs"
              >
                <option value="accept_edits">推荐模式（常见改动自动处理）</option>
                <option value="default">谨慎模式（关键操作先确认）</option>
                <option value="unrestricted">全自动模式（高风险）</option>
              </select>
            </div>
            <div className="sm-divider px-3 py-2 border-b">
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => handleSearchChange(e.target.value)}
                placeholder="搜索会话..."
                className="sm-input w-full py-1 text-xs"
              />
            </div>
            <div className="flex-1 overflow-y-auto py-1">
              {sessions.length === 0 && (
                <div className="px-4 py-3 text-xs sm-text-muted">{searchQuery ? "未找到匹配会话" : "暂无会话"}</div>
              )}
              <AnimatePresence>
                {sessions.map((s) => (
                  <motion.div
                    key={s.id}
                    initial={{ opacity: 0, x: -10 }}
                    animate={{ opacity: 1, x: 0 }}
                    exit={{ opacity: 0, x: -20, height: 0 }}
                    whileHover={{ scale: 1.01 }}
                    transition={{ duration: 0.2 }}
                    className={
                      "group flex items-center px-4 py-2 text-sm cursor-pointer rounded-md mx-1 transition-colors " +
                      (selectedSessionId === s.id ? "bg-[var(--sm-primary-soft)] text-[var(--sm-primary-strong)]" : "sm-text-primary hover:bg-[var(--sm-surface-soft)]")
                    }
                    onClick={() => onSelectSession(s.id)}
                  >
                    <div className="flex-1 min-w-0">
                      <div className="truncate text-xs">{s.title || "未命名任务"}</div>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        onExportSession(s.id);
                      }}
                      className="hidden group-hover:block sm-btn sm-btn-ghost h-6 w-6 text-xs ml-1 flex-shrink-0"
                      title="导出会话"
                    >
                      ↓
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        onDeleteSession(s.id);
                      }}
                      className="hidden group-hover:block sm-btn sm-btn-danger h-6 w-6 text-xs ml-1 flex-shrink-0"
                    >
                      ×
                    </button>
                  </motion.div>
                ))}
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
          设置
        </button>
      </div>
    </div>
  );
}
