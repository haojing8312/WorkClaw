import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import ReactMarkdown from "react-markdown";
import type { SessionToolManifestEntry, ToolCallInfo } from "../types";
import { getToolResultDetails, getToolResultSummary } from "../lib/tool-result";

/** 工具名 → 人性化描述 */
const TOOL_LABELS: Record<string, string> = {
  read_file: "读取文件",
  write_file: "写入文件",
  edit: "编辑文件",
  glob: "搜索文件",
  grep: "搜索内容",
  bash: "执行命令",
  background_process: "后台进程",
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
  if (tc.name === "background_process") {
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

function getToolOutputDisplay(tc: ToolCallInfo): string {
  if (tc.name === "task") {
    return String(tc.output || "");
  }
  return getToolResultSummary(tc.output);
}

function findToolManifestEntry(
  name: string,
  toolManifest: SessionToolManifestEntry[] | undefined,
): SessionToolManifestEntry | null {
  if (!toolManifest?.length) return null;
  return toolManifest.find((item) => item.name === name) ?? null;
}

function getToolBadges(
  tc: ToolCallInfo,
  toolManifest: SessionToolManifestEntry[] | undefined,
): string[] {
  const manifestEntry = findToolManifestEntry(tc.name, toolManifest);
  if (manifestEntry) {
    if (manifestEntry.requires_approval) return ["需确认"];
    if (manifestEntry.read_only) return ["只读"];
    if (!manifestEntry.read_only) return ["会修改"];
  }

  if (tc.name === "read_file" || tc.name === "glob" || tc.name === "grep" || tc.name === "web_search" || tc.name === "web_fetch") {
    return ["只读"];
  }
  if (tc.name === "write_file" || tc.name === "edit" || tc.name === "todo_write") {
    return ["会修改"];
  }
  if (tc.name === "bash") {
    return ["需确认"];
  }
  return [];
}

function getToolDetailHint(
  tc: ToolCallInfo,
  toolManifest: SessionToolManifestEntry[] | undefined,
): string | null {
  const manifestEntry = findToolManifestEntry(tc.name, toolManifest);
  const toolLabel = manifestEntry?.display_name || TOOL_LABELS[tc.name] || tc.name;
  const badges = getToolBadges(tc, toolManifest);
  if (tc.status === "error") {
    return `${toolLabel}执行失败，可以展开查看失败原因和原始参数。`;
  }
  if (badges.includes("需确认")) {
    return `${toolLabel}属于需要确认的执行操作，确认后才会继续。`;
  }
  if (badges.includes("只读")) {
    return `${toolLabel}属于只读操作，一般不会直接修改本地内容。`;
  }
  if (badges.includes("会修改")) {
    return `${toolLabel}可能修改文件、命令环境或会话状态。`;
  }
  return null;
}

interface ToolIslandProps {
  /** 当前批次的工具调用 items（仅 type==="tool_call"） */
  toolCalls: ToolCallInfo[];
  /** 是否正在执行中 */
  isRunning: boolean;
  /** 子 Agent 实时输出 */
  subAgentBuffer?: string;
  /** 当前会话工具元数据 */
  toolManifest?: SessionToolManifestEntry[];
}

export function ToolIsland({ toolCalls, isRunning, subAgentBuffer, toolManifest }: ToolIslandProps) {
  const [expanded, setExpanded] = useState(false);
  const [detailIndex, setDetailIndex] = useState<number | null>(null);

  const completed = toolCalls.filter((tc) => tc.status !== "running").length;
  const total = toolCalls.length;
  const errorCount = toolCalls.filter((tc) => tc.status === "error").length;
  const current = toolCalls.find((tc) => tc.status === "running");
  const currentLabel = current
    ? `${getToolStatusLabel(current)}${getParamSummary(current) ? ` · ${getParamSummary(current)}` : ""}`
    : null;

  const summaryLabel = isRunning
    ? currentLabel
      ? `正在处理 ${currentLabel}`
      : total > 1
      ? `正在处理 ${completed}/${total} 步`
      : "正在处理 1 个步骤"
    : errorCount > 0
    ? `已完成 ${total} 个步骤 · ${errorCount} 个异常`
    : `已完成 ${total} 个步骤`;

  return (
    <motion.div
      layout
      className="my-2 w-full"
      transition={{ type: "spring", stiffness: 400, damping: 30 }}
    >
      {/* 胶囊主体 */}
      <motion.div
        layout
        data-testid="tool-island-summary"
        className={
          "w-full cursor-pointer select-none overflow-hidden rounded-xl border border-slate-200/80 bg-white/75 " +
          (expanded
            ? "shadow-sm"
            : "shadow-[0_1px_2px_rgba(15,23,42,0.04)]")
        }
        onClick={() => setExpanded(!expanded)}
      >
        {/* 顶部摘要行 */}
        <motion.div layout="position" className="flex items-center gap-2.5 px-3 py-2">
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

          <div className="min-w-0 flex-1 truncate text-[12px] font-medium text-slate-500">{summaryLabel}</div>

          {/* 展开箭头 */}
          <motion.span
            animate={{ rotate: expanded ? 180 : 0 }}
            transition={{ duration: 0.2 }}
            className="text-xs text-slate-400"
          >
            ▾
          </motion.span>
        </motion.div>

        {/* 进度条（仅运行中且未展开时显示） */}
        {isRunning && !expanded && total > 1 && (
          <div className="px-3 pb-2">
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
              <div className="space-y-0.5 border-t border-slate-200/80 px-3 py-2">
                {toolCalls.map((tc, i) => {
                  const badges = getToolBadges(tc, toolManifest);
                  const detailHint = getToolDetailHint(tc, toolManifest);
                  const detailPayload = getToolResultDetails(tc.output);
                  return (
                  <div key={tc.id}>
                    <button
                      data-testid={`tool-island-step-${tc.id}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        setDetailIndex(detailIndex === i ? null : i);
                      }}
                      className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-xs transition-colors hover:bg-slate-50"
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
                      <span className="w-20 shrink-0 truncate text-slate-600">
                        {getToolStatusLabel(tc)}
                      </span>
                      {/* 参数摘要 */}
                      <span className="flex-1 truncate text-slate-400">
                        {getParamSummary(tc)}
                      </span>
                      {badges.length > 0 && (
                        <span className="flex shrink-0 gap-1">
                          {badges.map((badge) => (
                            <span
                              key={badge}
                              className="rounded-full border border-slate-200 bg-slate-50 px-2 py-0.5 text-[10px] font-medium text-slate-500"
                            >
                              {badge}
                            </span>
                          ))}
                        </span>
                      )}
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
                            {detailHint && (
                              <div className="rounded-xl border border-slate-200/80 bg-slate-50/80 px-3 py-2 text-[11px] text-slate-600">
                                {detailHint}
                              </div>
                            )}
                            <pre className="max-h-32 overflow-x-auto overflow-y-auto rounded-xl bg-white/75 p-2.5 text-[11px] text-slate-600">
                              {tc.name === "task"
                                ? String(tc.input.prompt || "")
                                : JSON.stringify(tc.input, null, 2)}
                            </pre>
                            {/* 子 Agent 实时输出 */}
                            {tc.name === "task" && tc.status === "running" && subAgentBuffer && (
                              <div className="prose prose-xs max-h-32 overflow-y-auto rounded-xl bg-white/75 p-2.5 text-[11px] text-slate-600 prose-slate">
                                <ReactMarkdown>{subAgentBuffer}</ReactMarkdown>
                                <span className="animate-pulse text-blue-400">|</span>
                              </div>
                            )}
                            {tc.output && (
                              <pre className="max-h-32 overflow-x-auto overflow-y-auto rounded-xl bg-white/75 p-2.5 text-[11px] text-slate-500">
                                {tc.name === "task" ? (
                                  <div className="prose prose-xs prose-slate">
                                    <ReactMarkdown>{tc.output}</ReactMarkdown>
                                  </div>
                                ) : (
                                  getToolOutputDisplay(tc)
                                )}
                              </pre>
                            )}
                            {tc.name !== "task" && detailPayload && (
                              <pre className="max-h-32 overflow-x-auto overflow-y-auto rounded-xl bg-white/75 p-2.5 text-[11px] text-slate-500">
                                {JSON.stringify(detailPayload, null, 2)}
                              </pre>
                            )}
                          </div>
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </div>
                  );
                })}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </motion.div>
  );
}
