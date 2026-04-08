import { motion } from "framer-motion";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import { ToolIsland } from "../ToolIsland";
import type { SessionRunProjection, SessionToolManifestEntry, StreamItem } from "../../types";

const RUN_FAILURE_CARD_CLASS =
  "mr-auto max-w-[80%] rounded-2xl border border-amber-200 bg-amber-50 px-5 py-4 text-sm text-amber-900 shadow-sm";
const RUN_FAILURE_SECTION_LABEL_CLASS = "text-xs font-medium tracking-wide text-amber-700";
const RUN_FAILURE_MESSAGE_CLASS = "mt-2 whitespace-pre-wrap text-sm text-amber-800";
const RUN_FAILURE_TECHNICAL_DETAILS_TOGGLE_CLASS =
  "inline-flex items-center rounded-lg border border-amber-200 bg-white/70 px-3 py-1.5 text-xs font-medium text-amber-800 transition-colors hover:bg-white";
const RUN_FAILURE_TECHNICAL_DETAILS_PANEL_CLASS =
  "mt-2 whitespace-pre-wrap break-all rounded-xl border border-white/70 bg-white/70 px-4 py-3 font-mono text-xs text-amber-900/90";

export function getRunFailureTechnicalToggleLabel(expanded: boolean): string {
  return expanded ? "隐藏技术详情" : "查看技术详情";
}

export type ChatRunFailureDisplay = {
  title: string;
  message: string;
  rawMessage: string | null;
};

export type RenderChatStreamItemsArgs = {
  items: StreamItem[];
  subAgentBuffer: string;
  markdownComponents: Record<string, unknown>;
  toolManifest: SessionToolManifestEntry[];
};

export type RenderChatRunFailureCardArgs = {
  run: SessionRunProjection;
  streaming: boolean;
  expandedRunDetailIds: string[];
  onToggleRunDetail: (runId: string) => void;
  onContinueExecution: () => Promise<void> | void;
  getRunFailureDisplay: (run: SessionRunProjection) => ChatRunFailureDisplay;
  getRunFailureTechnicalToggleLabel: (expanded: boolean) => string;
};

export function extractPlainTextFromStreamItems(items: StreamItem[]): string {
  return items
    .filter((item) => item.type === "text")
    .map((item) => item.content || "")
    .join("");
}

export function renderChatStreamItems({ items, subAgentBuffer, markdownComponents, toolManifest }: RenderChatStreamItemsArgs) {
  const groups: { type: "text" | "tools"; items: StreamItem[] }[] = [];
  for (const item of items) {
    if (item.type === "tool_call") {
      const last = groups[groups.length - 1];
      if (last && last.type === "tools") {
        last.items.push(item);
      } else {
        groups.push({ type: "tools", items: [item] });
      }
    } else {
      groups.push({ type: "text", items: [item] });
    }
  }

  return groups.map((group, index) => {
    if (group.type === "tools") {
      const toolCalls = group.items
        .filter((it) => it.toolCall)
        .map((it) => it.toolCall!);
      const hasRunning = toolCalls.some((toolCall) => toolCall.status === "running");
      return (
        <ToolIsland
          key={`island-${index}`}
          toolCalls={toolCalls}
          isRunning={hasRunning}
          subAgentBuffer={hasRunning ? subAgentBuffer : undefined}
          toolManifest={toolManifest}
        />
      );
    }

    const text = group.items.map((it) => it.content || "").join("");
    if (!text) return null;

    return (
      <div key={`txt-${index}`}>
        <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
          {text}
        </ReactMarkdown>
      </div>
    );
  });
}

export function renderChatRunFailureCard({
  run,
  streaming,
  expandedRunDetailIds,
  onToggleRunDetail,
  onContinueExecution,
  getRunFailureDisplay,
  getRunFailureTechnicalToggleLabel,
}: RenderChatRunFailureCardArgs) {
  const display = getRunFailureDisplay(run);
  const showContinueAction = run.error_kind === "max_turns";
  const showBufferedOutput =
    Boolean(run.buffered_text) && !String(run.assistant_message_id || "").trim();
  const rawDetailsExpanded = expandedRunDetailIds.includes(run.id);
  const technicalToggleLabel = getRunFailureTechnicalToggleLabel(rawDetailsExpanded);

  return (
    <motion.div
      key={`run-failure-${run.id}`}
      data-testid={`run-failure-card-${run.id}`}
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className={RUN_FAILURE_CARD_CLASS}
    >
      <div className={RUN_FAILURE_SECTION_LABEL_CLASS}>本轮执行结果</div>
      <div className="mt-1 text-lg font-semibold">{display.title}</div>
      {display.message ? <div className={RUN_FAILURE_MESSAGE_CLASS}>{display.message}</div> : null}
      {display.rawMessage ? (
        <div className="mt-3">
          <button
            type="button"
            onClick={() => onToggleRunDetail(run.id)}
            className={RUN_FAILURE_TECHNICAL_DETAILS_TOGGLE_CLASS}
          >
            {technicalToggleLabel}
          </button>
          {rawDetailsExpanded ? <div className={RUN_FAILURE_TECHNICAL_DETAILS_PANEL_CLASS}>{display.rawMessage}</div> : null}
        </div>
      ) : null}
      {showBufferedOutput && (
        <div className="mt-3 rounded-xl border border-white/70 bg-white/70 px-4 py-3 text-sm text-gray-700">
          <div className="mb-1 text-xs font-medium tracking-wide text-gray-500">已保留的部分输出</div>
          <div className="whitespace-pre-wrap">{run.buffered_text}</div>
        </div>
      )}
      {showContinueAction ? (
        <div className="mt-3">
          <button
            type="button"
            onClick={() => void onContinueExecution()}
            disabled={streaming}
            className="inline-flex items-center rounded-lg bg-amber-600 px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-amber-700 disabled:cursor-not-allowed disabled:bg-amber-300"
          >
            继续执行
          </button>
        </div>
      ) : null}
    </motion.div>
  );
}
