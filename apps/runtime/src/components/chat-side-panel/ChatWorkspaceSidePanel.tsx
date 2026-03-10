import { AnimatePresence, motion } from "framer-motion";
import { TaskPanel } from "./TaskPanel";
import { WebSearchPanel } from "./WebSearchPanel";
import { WorkspaceFilesPanel } from "./WorkspaceFilesPanel";
import type { TaskPanelViewModel, WebSearchEntryView } from "./view-model";

interface ChatWorkspaceSidePanelProps {
  open: boolean;
  tab: "tasks" | "files" | "websearch";
  onTabChange: (tab: "tasks" | "files" | "websearch") => void;
  onClose: () => void;
  workspace: string;
  touchedFiles: string[];
  active: boolean;
  taskModel: TaskPanelViewModel;
  webSearchEntries: WebSearchEntryView[];
}

export function ChatWorkspaceSidePanel({
  open,
  tab,
  onTabChange,
  onClose,
  workspace,
  touchedFiles,
  active,
  taskModel,
  webSearchEntries,
}: ChatWorkspaceSidePanelProps) {
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ width: 0, opacity: 0 }}
          animate={{ width: 320, opacity: 1 }}
          exit={{ width: 0, opacity: 0 }}
          transition={{ type: "spring", stiffness: 300, damping: 30 }}
          className="h-full bg-gray-50 border-l border-gray-200 overflow-hidden flex flex-col"
        >
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 bg-white/50">
            <div className="flex items-center gap-2">
              <button
                onClick={() => onTabChange("tasks")}
                className={`px-2 py-1 rounded text-xs transition-colors ${
                  tab === "tasks" ? "bg-blue-100 text-blue-600" : "text-gray-500 hover:bg-gray-100"
                }`}
              >
                当前任务
              </button>
              <button
                onClick={() => onTabChange("files")}
                className={`px-2 py-1 rounded text-xs transition-colors ${
                  tab === "files" ? "bg-blue-100 text-blue-600" : "text-gray-500 hover:bg-gray-100"
                }`}
              >
                文件
              </button>
              <button
                onClick={() => onTabChange("websearch")}
                className={`px-2 py-1 rounded text-xs transition-colors ${
                  tab === "websearch" ? "bg-blue-100 text-blue-600" : "text-gray-500 hover:bg-gray-100"
                }`}
              >
                Web 搜索
              </button>
            </div>
            <button
              onClick={onClose}
              className="text-gray-400 hover:text-gray-600"
              aria-label="关闭面板"
            >
              ✕
            </button>
          </div>

          <div className="flex-1 min-h-0 overflow-y-auto p-4 space-y-4">
            {tab === "tasks" && <TaskPanel model={taskModel} />}
            {tab === "files" && (
              <WorkspaceFilesPanel workspace={workspace} touchedFiles={touchedFiles} active={active && tab === "files"} />
            )}
            {tab === "websearch" && <WebSearchPanel entries={webSearchEntries} />}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
