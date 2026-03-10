import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import ReactMarkdown from "react-markdown";
import { StreamItem, ToolCallInfo } from "../types";

/** 工具名 → 人性化描述 */
const TOOL_LABELS: Record<string, string> = {
  read_file: "读取文件",
  write_file: "写入文件",
  edit: "编辑文件",
  glob: "搜索文件",
  grep: "搜索内容",
  bash: "执行命令",
  web_search: "网页搜索",
  web_fetch: "获取网页",
  task: "子任务",
  todo_write: "更新任务",
  memory: "访问记忆",
  ask_user: "等待用户回复",
  compact: "压缩上下文",
};

/** 提取工具调用的关键参数摘要 */
function getParamSummary(tc: ToolCallInfo): string {
  if (tc.name === "task") return String(tc.input.agent_type || "general");
  if (tc.name === "read_file" || tc.name === "write_file" || tc.name === "edit") {
    const p = String(tc.input.file_path || tc.input.path || "");
    return p.split(/[/\\]/).pop() || p;
  }
  if (tc.name === "glob") return String(tc.input.pattern || "");
  if (tc.name === "grep") return String(tc.input.pattern || "");
  if (tc.name === "bash") {
    const cmd = String(tc.input.command || "");
    return cmd.length > 30 ? cmd.slice(0, 30) + "..." : cmd;
  }
  if (tc.name === "web_search") return String(tc.input.query || "");
  return "";
}

function getToolStatusLabel(tc: ToolCallInfo): string {
  const baseLabel = TOOL_LABELS[tc.name] || tc.name;
  if (tc.status === "error") {
    return `${baseLabel}失败`;
  }
  if (tc.status === "running") {
    return `正在${baseLabel}`;
  }
  return baseLabel;
}

interface ToolIslandProps {
  /** 当前批次的工具调用 items（仅 type==="tool_call"） */
  toolCalls: ToolCallInfo[];
  /** 是否正在执行中 */
  isRunning: boolean;
  /** 子 Agent 实时输出 */
  subAgentBuffer?: string;
}

export function ToolIsland({ toolCalls, isRunning, subAgentBuffer }: ToolIslandProps) {
  const [expanded, setExpanded] = useState(false);
  const [detailIndex, setDetailIndex] = useState<number | null>(null);

  const completed = toolCalls.filter((tc) => tc.status !== "running").length;
  const total = toolCalls.length;
  const errorCount = toolCalls.filter((tc) => tc.status === "error").length;
  const current = toolCalls.find((tc) => tc.status === "running");
  const currentLabel = current
    ? `${getToolStatusLabel(current)}${getParamSummary(current) ? ` · ${getParamSummary(current)}` : ""}`
    : null;

  const allDone = !isRunning && total > 0;
  const summaryLabel = isRunning
    ? currentLabel || "正在处理步骤"
    : errorCount > 0
    ? `已完成 ${completed} 个步骤，${errorCount} 个待处理`
    : `已完成 ${total} 个步骤`;

  return (
    <motion.div
      layout
      className="my-2 mx-auto max-w-[360px]"
      transition={{ type: "spring", stiffness: 400, damping: 30 }}
    >
      {/* 胶囊主体 */}
      <motion.div
        layout
        className={
          "rounded-2xl overflow-hidden cursor-pointer select-none " +
          (expanded
            ? "bg-white/95 backdrop-blur-md shadow-lg border border-gray-200"
            : "bg-white/90 backdrop-blur-md shadow-md border border-gray-200")
        }
        onClick={() => setExpanded(!expanded)}
      >
        {/* 顶部摘要行 */}
        <motion.div layout="position" className="flex items-center gap-2.5 px-4 py-2.5">
          {/* 状态指示器 */}
          {isRunning ? (
            <span className="relative flex h-2.5 w-2.5">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-blue-400 opacity-75" />
              <span className="relative inline-flex rounded-full h-2.5 w-2.5 bg-blue-500" />
            </span>
          ) : (
            <svg className="h-3.5 w-3.5 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
            </svg>
          )}

          {/* 描述文字 */}
          <span className="flex-1 text-xs font-medium text-gray-700 truncate">
            {summaryLabel}
          </span>

          {/* 进度计数 */}
          {isRunning && total > 1 && (
            <span className="text-[11px] text-gray-400 tabular-nums">
              {completed}/{total}
            </span>
          )}

          {/* 展开箭头 */}
          <motion.span
            animate={{ rotate: expanded ? 180 : 0 }}
            transition={{ duration: 0.2 }}
            className="text-gray-400 text-xs"
          >
            ▾
          </motion.span>
        </motion.div>

        {/* 进度条（仅运行中且未展开时显示） */}
        {isRunning && !expanded && total > 1 && (
          <div className="px-4 pb-2.5">
            <div className="h-1 bg-gray-100 rounded-full overflow-hidden">
              <motion.div
                className="h-full bg-blue-400 rounded-full"
                initial={{ width: 0 }}
                animate={{ width: `${(completed / total) * 100}%` }}
                transition={{ duration: 0.3 }}
              />
            </div>
          </div>
        )}

        {/* 展开的详情列表 */}
        <AnimatePresence>
          {expanded && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ type: "spring", stiffness: 500, damping: 35 }}
              className="overflow-hidden"
            >
              <div className="border-t border-gray-100 px-3 py-2 space-y-0.5">
                {toolCalls.map((tc, i) => (
                  <div key={tc.id}>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setDetailIndex(detailIndex === i ? null : i);
                      }}
                      className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-xs hover:bg-gray-50 transition-colors text-left"
                    >
                      {/* 状态图标 */}
                      {tc.status === "running" ? (
                        <span className="h-2 w-2 rounded-full bg-blue-400 animate-pulse flex-shrink-0" />
                      ) : tc.status === "completed" ? (
                        <svg className="h-3 w-3 text-green-500 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                          <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                        </svg>
                      ) : (
                        <svg className="h-3 w-3 text-red-400 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                          <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                      )}
                      {/* 工具名 */}
                      <span className="text-gray-700 w-20 truncate flex-shrink-0">
                        {getToolStatusLabel(tc)}
                      </span>
                      {/* 参数摘要 */}
                      <span className="text-gray-400 truncate flex-1">
                        {getParamSummary(tc)}
                      </span>
                    </button>
                    {/* 二级展开：完整参数和输出 */}
                    <AnimatePresence>
                      {detailIndex === i && (
                        <motion.div
                          initial={{ height: 0, opacity: 0 }}
                          animate={{ height: "auto", opacity: 1 }}
                          exit={{ height: 0, opacity: 0 }}
                          transition={{ duration: 0.15 }}
                          className="overflow-hidden"
                        >
                          <div className="ml-7 mr-2 mb-2 space-y-1.5">
                            <pre className="bg-gray-50 rounded-xl p-2.5 text-[11px] text-gray-600 overflow-x-auto max-h-32 overflow-y-auto">
                              {tc.name === "task"
                                ? String(tc.input.prompt || "")
                                : JSON.stringify(tc.input, null, 2)}
                            </pre>
                            {/* 子 Agent 实时输出 */}
                            {tc.name === "task" && tc.status === "running" && subAgentBuffer && (
                              <div className="bg-gray-50 rounded-xl p-2.5 text-[11px] text-gray-600 max-h-32 overflow-y-auto prose prose-xs prose-gray">
                                <ReactMarkdown>{subAgentBuffer}</ReactMarkdown>
                                <span className="animate-pulse text-blue-400">|</span>
                              </div>
                            )}
                            {tc.output && (
                              <pre className="bg-gray-50 rounded-xl p-2.5 text-[11px] text-gray-500 overflow-x-auto max-h-32 overflow-y-auto">
                                {tc.name === "task" ? (
                                  <div className="prose prose-xs prose-gray">
                                    <ReactMarkdown>{tc.output}</ReactMarkdown>
                                  </div>
                                ) : (
                                  tc.output
                                )}
                              </pre>
                            )}
                          </div>
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </div>
                ))}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </motion.div>
  );
}
