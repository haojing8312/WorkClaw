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
      <div className="w-12 bg-white flex flex-col h-full border-r border-gray-200 items-center py-3 gap-3 flex-shrink-0">
        <button
          onClick={onCollapse}
          className="w-8 h-8 flex items-center justify-center text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors"
          title="展开侧边栏"
          aria-label="展开侧边栏"
        >
          ▶
        </button>
        <button
          onClick={onOpenStartTask}
          className={`w-8 h-8 flex items-center justify-center rounded transition-colors ${
            isStartTask ? "bg-blue-50 text-blue-600" : "text-gray-400 hover:text-gray-600 hover:bg-gray-100"
          }`}
          title="开始任务"
          aria-label="开始任务"
        >
          ○
        </button>
        <button
          onClick={onOpenExperts}
          className={`w-8 h-8 flex items-center justify-center rounded transition-colors ${
            isExperts ? "bg-blue-50 text-blue-600" : "text-gray-400 hover:text-gray-600 hover:bg-gray-100"
          }`}
          title="专家技能"
          aria-label="专家技能"
        >
          ◆
        </button>
        <button
          onClick={onOpenEmployees}
          className={`w-8 h-8 flex items-center justify-center rounded transition-colors ${
            isEmployees ? "bg-blue-50 text-blue-600" : "text-gray-400 hover:text-gray-600 hover:bg-gray-100"
          }`}
          title="智能体员工"
          aria-label="智能体员工"
        >
          ◎
        </button>
        <button
          onClick={onSettings}
          className="w-8 h-8 flex items-center justify-center text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors mt-auto"
          title="设置"
          aria-label="设置"
        >
          ⚙
        </button>
      </div>
    );
  }

  return (
    <div className="w-60 bg-white flex flex-col h-full border-r border-gray-200 flex-shrink-0">
      <div className="px-4 py-3 text-xs font-medium text-gray-500 border-b border-gray-200 flex items-center justify-between">
        <span>SkillMint</span>
        <button
          onClick={onCollapse}
          className="text-gray-500 hover:text-gray-600 text-sm transition-colors"
          title="折叠侧边栏"
        >
          ◀
        </button>
      </div>

      <div className="px-3 py-2 border-b border-gray-200">
        <div className="grid grid-cols-3 gap-2">
          <button
            onClick={onOpenStartTask}
            className={
              "text-xs py-1.5 rounded-md transition-colors " +
              (isStartTask ? "bg-blue-500 text-white" : "bg-gray-100 text-gray-600 hover:bg-gray-200")
            }
          >
            开始任务
          </button>
          <button
            onClick={onOpenExperts}
            className={
              "text-xs py-1.5 rounded-md transition-colors " +
              (isExperts ? "bg-blue-500 text-white" : "bg-gray-100 text-gray-600 hover:bg-gray-200")
            }
          >
            专家技能
          </button>
          <button
            onClick={onOpenEmployees}
            className={
              "text-xs py-1.5 rounded-md transition-colors " +
              (isEmployees ? "bg-blue-500 text-white" : "bg-gray-100 text-gray-600 hover:bg-gray-200")
            }
          >
            智能体员工
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-hidden">
        {selectedSkillId && (
          <div className="h-full flex flex-col">
            <div className="px-4 py-2 text-xs font-medium text-gray-500 border-t border-b border-gray-200">
              <span>会话历史</span>
            </div>
            <div className="px-3 py-2 border-b border-gray-200">
              <label className="block text-[11px] text-gray-500 mb-1">操作确认级别</label>
              <select
                value={newSessionPermissionMode}
                onChange={(e) =>
                  onChangeNewSessionPermissionMode(e.target.value as "default" | "accept_edits" | "unrestricted")
                }
                className="w-full bg-gray-50 border border-gray-200 rounded px-2 py-1 text-xs text-gray-800 focus:outline-none focus:border-blue-400 focus:ring-1 focus:ring-blue-400"
              >
                <option value="accept_edits">推荐模式（常见改动自动处理）</option>
                <option value="default">谨慎模式（关键操作先确认）</option>
                <option value="unrestricted">全自动模式（高风险）</option>
              </select>
            </div>
            <div className="px-3 py-2 border-b border-gray-200">
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => handleSearchChange(e.target.value)}
                placeholder="搜索会话..."
                className="w-full bg-gray-50 border border-gray-200 rounded px-2 py-1 text-xs text-gray-800 placeholder-gray-400 focus:outline-none focus:border-blue-400 focus:ring-1 focus:ring-blue-400"
              />
            </div>
            <div className="flex-1 overflow-y-auto py-1">
              {sessions.length === 0 && (
                <div className="px-4 py-3 text-xs text-gray-400">{searchQuery ? "未找到匹配会话" : "暂无会话"}</div>
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
                      "group flex items-center px-4 py-2 text-sm cursor-pointer transition-colors " +
                      (selectedSessionId === s.id ? "bg-blue-50 text-blue-600" : "text-gray-700 hover:bg-gray-50")
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
                      className="hidden group-hover:block text-gray-400 hover:text-gray-600 text-xs ml-1 flex-shrink-0"
                      title="导出会话"
                    >
                      ↓
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        onDeleteSession(s.id);
                      }}
                      className="hidden group-hover:block text-red-400 hover:text-red-300 text-xs ml-1 flex-shrink-0"
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

      <div className="p-3 space-y-2 border-t border-gray-200">
        <button
          onClick={onSettings}
          className="w-full bg-gray-100 hover:bg-gray-200 active:scale-[0.97] text-gray-700 text-sm py-1.5 rounded-lg transition-all"
        >
          设置
        </button>
      </div>
    </div>
  );
}
