import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { TaskPanel } from "./TaskPanel";
import { WebSearchPanel } from "./WebSearchPanel";
import { WorkspaceFilesPanel } from "./WorkspaceFilesPanel";
import type { TaskPanelViewModel, WebSearchEntryView } from "./view-model";

const DEFAULT_DRAWER_WIDTH = 760;
const MIN_DRAWER_WIDTH = 420;
const MAX_DRAWER_WIDTH = 1100;

function clampDrawerWidth(width: number): number {
  return Math.min(MAX_DRAWER_WIDTH, Math.max(MIN_DRAWER_WIDTH, width));
}

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
  const [drawerWidth, setDrawerWidth] = useState(DEFAULT_DRAWER_WIDTH);
  const resizingRef = useRef(false);
  const activePointerIdRef = useRef<number | null>(null);

  const updateDrawerWidth = useCallback((clientX: number) => {
    setDrawerWidth(clampDrawerWidth(window.innerWidth - clientX));
  }, []);

  useEffect(() => {
    if (open) {
      setDrawerWidth(DEFAULT_DRAWER_WIDTH);
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;

    const handleMouseMove = (event: MouseEvent) => {
      if (!resizingRef.current) return;
      updateDrawerWidth(event.clientX);
    };

    const handlePointerMove = (event: PointerEvent) => {
      if (!resizingRef.current) return;
      if (activePointerIdRef.current !== null && event.pointerId !== activePointerIdRef.current) return;
      updateDrawerWidth(event.clientX);
    };

    const stopResizing = () => {
      resizingRef.current = false;
      activePointerIdRef.current = null;
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", stopResizing);
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", stopResizing);
    window.addEventListener("pointercancel", stopResizing);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", stopResizing);
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", stopResizing);
      window.removeEventListener("pointercancel", stopResizing);
    };
  }, [open, updateDrawerWidth]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          data-testid="chat-workspace-drawer"
          initial={{ x: 24, opacity: 0 }}
          animate={{ x: 0, opacity: 1 }}
          exit={{ x: 24, opacity: 0 }}
          transition={{ type: "spring", stiffness: 300, damping: 30 }}
          style={{ width: `${drawerWidth}px` }}
          className="relative flex h-full flex-col overflow-hidden border-l border-gray-200 bg-gray-50"
        >
          <button
            type="button"
            aria-label="调整面板宽度"
            data-testid="chat-workspace-drawer-resize-handle"
            onPointerDown={(event) => {
              event.preventDefault();
              resizingRef.current = true;
              activePointerIdRef.current = event.pointerId;
              event.currentTarget.setPointerCapture?.(event.pointerId);
            }}
            onMouseDown={(event) => {
              event.preventDefault();
              resizingRef.current = true;
            }}
            className="absolute -left-1 top-0 z-20 h-full w-3 cursor-col-resize bg-transparent"
          />
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

          <div className={`min-h-0 flex-1 p-4 ${tab === "files" ? "overflow-hidden" : "space-y-4 overflow-y-auto"}`}>
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
