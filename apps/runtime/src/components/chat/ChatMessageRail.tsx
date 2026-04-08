import { Fragment, type MutableRefObject, useMemo } from "react";
import { motion } from "framer-motion";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import { ThinkingBlock } from "../ThinkingBlock";
import { TaskJourneySummary } from "../chat-journey/TaskJourneySummary";
import type { ChatMessagePart, Message, SessionRunProjection, SessionToolManifestEntry, StreamItem } from "../../types";
import { createChatMarkdownComponents } from "./chatMarkdownComponents";
import {
  extractPlainTextFromStreamItems,
  getRunFailureTechnicalToggleLabel,
  renderChatRunFailureCard,
  renderChatStreamItems,
} from "./chatMessageRailHelpers";

type TaskJourneyModel = ReturnType<
  typeof import("../chat-side-panel/view-model").buildTaskJourneyViewModel
>;

type ChatMessageRailProps = {
  renderedMessages: Message[];
  highlightedMessageIndex: number | null;
  messageElementRefs: MutableRefObject<Record<number, HTMLDivElement | null>>;
  expandedThinkingKeys: string[];
  onToggleThinkingBlock: (key: string) => void;
  buildTaskJourneyModel: (messages: Message[]) => TaskJourneyModel;
  shouldRenderCompletedJourneySummary: (model: TaskJourneyModel) => boolean;
  failedRunsByAssistantMessageId: Map<string, SessionRunProjection[]>;
  failedRunsByUserMessageId: Map<string, SessionRunProjection[]>;
  renderInstallCandidates: (candidates: unknown[]) => React.ReactNode;
  extractInstallCandidates: (items: StreamItem[], text: string) => unknown[];
  renderUserContentParts: (parts: ChatMessagePart[]) => React.ReactNode;
  copiedAssistantMessageKey: string | null;
  onCopyAssistantMessage: (messageKey: string, content: string) => Promise<void> | void;
  CopyActionIcon: (props: { copied: boolean }) => React.ReactNode;
  onViewFilesFromDelivery: () => void;
  expandedRunDetailIds: string[];
  streaming: boolean;
  onToggleRunDetail: (runId: string) => void;
  onContinueExecution: () => Promise<void> | void;
  getRunFailureDisplay: (run: SessionRunProjection) => {
    title: string;
    message: string;
    rawMessage: string | null;
  };
  orphanFailedRuns: SessionRunProjection[];
  showStreamingAssistantBubble: boolean;
  showStreamingThinkingState: boolean;
  streamReasoning:
    | {
        status: "thinking" | "completed" | "interrupted";
        content: string;
        durationMs?: number;
      }
    | null;
  streamItems: StreamItem[];
  toolManifest: SessionToolManifestEntry[];
  subAgentBuffer: string;
  subAgentRoleName: string;
  askUserQuestion: string | null;
  askUserOptions: string[];
  askUserAnswer: string;
  onAskUserAnswerChange: (value: string) => void;
  onAnswerUser: (answer: string) => void;
  onOpenExternalLink?: (url: string) => Promise<void> | void;
};

export function ChatMessageRail({
  renderedMessages,
  highlightedMessageIndex,
  messageElementRefs,
  expandedThinkingKeys,
  onToggleThinkingBlock,
  buildTaskJourneyModel,
  shouldRenderCompletedJourneySummary,
  failedRunsByAssistantMessageId,
  failedRunsByUserMessageId,
  renderInstallCandidates,
  extractInstallCandidates,
  renderUserContentParts,
  copiedAssistantMessageKey,
  onCopyAssistantMessage,
  CopyActionIcon,
  onViewFilesFromDelivery,
  expandedRunDetailIds,
  streaming,
  onToggleRunDetail,
  onContinueExecution,
  getRunFailureDisplay,
  orphanFailedRuns,
  showStreamingAssistantBubble,
  showStreamingThinkingState,
  streamReasoning,
  streamItems,
  toolManifest,
  subAgentBuffer,
  subAgentRoleName,
  askUserQuestion,
  askUserOptions,
  askUserAnswer,
  onAskUserAnswerChange,
  onAnswerUser,
  onOpenExternalLink,
}: ChatMessageRailProps) {
  const markdownComponents = useMemo(() => createChatMarkdownComponents(onOpenExternalLink), [onOpenExternalLink]);
  const streamText = useMemo(() => extractPlainTextFromStreamItems(streamItems), [streamItems]);
  return (
    <>
      {renderedMessages.map((message, index) => {
        const isLatest = index === renderedMessages.length - 1;
        const isSessionFocusTarget = highlightedMessageIndex === index;
        const messageJourneyModel = message.role === "assistant" ? buildTaskJourneyModel([message]) : null;
        const shouldRenderJourneySummary =
          messageJourneyModel !== null && shouldRenderCompletedJourneySummary(messageJourneyModel);
        const messageSummaryKey = (message.runId || message.id || `message-${index}`).trim();
        const inlineFailedRuns =
          message.role === "assistant" && (message.id || "").trim()
            ? failedRunsByAssistantMessageId.get((message.id || "").trim()) ?? []
            : message.role === "user" && (message.id || "").trim()
            ? failedRunsByUserMessageId.get((message.id || "").trim()) ?? []
            : [];

        return (
          <Fragment key={message.id || `${index}-${message.created_at}`}>
            <motion.div
              ref={(node) => {
                messageElementRefs.current[index] = node;
              }}
              data-testid={`chat-message-${index}`}
              data-recovered-run-message={message.id?.startsWith("recovered-run-") ? "true" : "false"}
              data-session-focus-highlighted={isSessionFocusTarget ? "true" : "false"}
              initial={isLatest ? { opacity: 0, x: message.role === "user" ? 20 : -20 } : false}
              animate={{ opacity: 1, x: 0 }}
              transition={{ type: "spring", stiffness: 300, damping: 24 }}
              className={"flex " + (message.role === "user" ? "justify-end" : "justify-start")}
            >
              <div
                data-testid={`chat-message-bubble-${message.id || index}`}
                className={
                  "text-sm transition-all " +
                  (isSessionFocusTarget ? "ring-2 ring-amber-300 " : "") +
                  (message.role === "user"
                    ? "max-w-[28rem] rounded-[1.4rem] bg-slate-100 px-4 py-2.5 text-slate-800 shadow-[0_1px_2px_rgba(15,23,42,0.04)] md:max-w-[32rem]"
                    : "w-full max-w-[92%] px-0 py-1 text-slate-800 sm:max-w-[88%] md:max-w-[48rem] xl:max-w-[52rem]")
                }
              >
                {message.role === "assistant" && message.reasoning && (
                  <ThinkingBlock
                    status={message.reasoning.status}
                    content={message.reasoning.content}
                    durationMs={message.reasoning.duration_ms}
                    expanded={expandedThinkingKeys.includes(`message-${message.id || index}`)}
                    onToggle={
                      message.reasoning.content.trim()
                        ? () => onToggleThinkingBlock(`message-${message.id || index}`)
                        : undefined
                    }
                    toggleTestId={`thinking-block-toggle-${message.id || index}`}
                  />
                )}
                {message.role === "assistant" && message.streamItems ? (
                  <>
                    {renderChatStreamItems({
                      items: message.streamItems,
                      subAgentBuffer,
                      markdownComponents,
                      toolManifest,
                    })}
                    {renderInstallCandidates(extractInstallCandidates(message.streamItems, message.content))}
                  </>
                ) : message.role === "assistant" && message.toolCalls ? (
                  <>
                    {renderChatStreamItems({
                      items: message.toolCalls.map((toolCall) => ({ type: "tool_call", toolCall })),
                      subAgentBuffer,
                      markdownComponents,
                      toolManifest,
                    })}
                    <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                      {message.content}
                    </ReactMarkdown>
                  </>
                ) : message.role === "assistant" ? (
                  <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                    {message.content}
                  </ReactMarkdown>
                ) : message.role === "user" && message.contentParts?.length ? (
                  renderUserContentParts(message.contentParts)
                ) : (
                  message.content
                )}
                {message.role === "assistant" && message.content.trim() && (
                  <div className="mt-3 flex items-center justify-end gap-2">
                    <button
                      type="button"
                      data-testid={`assistant-copy-action-${message.id || index}`}
                      aria-label="复制回答"
                      title={copiedAssistantMessageKey === `message-${message.id || index}` ? "已复制" : "复制回答"}
                      onClick={() => void onCopyAssistantMessage(`message-${message.id || index}`, message.content)}
                      className="inline-flex h-9 w-9 items-center justify-center rounded-full text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 focus-visible:ring-offset-2"
                    >
                      <CopyActionIcon copied={copiedAssistantMessageKey === `message-${message.id || index}`} />
                    </button>
                  </div>
                )}
              </div>
            </motion.div>
            {shouldRenderJourneySummary && messageJourneyModel && (
              <div data-testid={`task-journey-summary-${messageSummaryKey}`}>
                <TaskJourneySummary model={messageJourneyModel} onViewFiles={onViewFilesFromDelivery} />
              </div>
            )}
            {inlineFailedRuns.map((run) =>
              renderChatRunFailureCard({
                run,
                streaming,
                expandedRunDetailIds,
                onToggleRunDetail,
                onContinueExecution,
                getRunFailureDisplay,
                getRunFailureTechnicalToggleLabel,
              }),
            )}
          </Fragment>
        );
      })}

      {orphanFailedRuns.map((run) =>
        renderChatRunFailureCard({
          run,
          streaming,
          expandedRunDetailIds,
          onToggleRunDetail,
          onContinueExecution,
          getRunFailureDisplay,
          getRunFailureTechnicalToggleLabel,
        }),
      )}

      {showStreamingAssistantBubble && (
        <motion.div initial={{ opacity: 0, x: -20 }} animate={{ opacity: 1, x: 0 }} className="flex justify-start">
          <div
            data-testid="chat-streaming-bubble"
            className="w-full max-w-[92%] px-0 py-1 text-sm text-slate-800 sm:max-w-[88%] md:max-w-[48rem] xl:max-w-[52rem]"
          >
            {showStreamingThinkingState && (
              <ThinkingBlock
                status={streamReasoning?.status || "thinking"}
                content={streamReasoning?.content || ""}
                durationMs={streamReasoning?.durationMs}
                expanded={expandedThinkingKeys.includes("stream")}
                onToggle={(streamReasoning?.content || "").trim() ? () => onToggleThinkingBlock("stream") : undefined}
              />
            )}
            {streamItems.length > 0 &&
              renderChatStreamItems({
                items: streamItems,
                subAgentBuffer,
                markdownComponents,
                toolManifest,
              })}
            {subAgentBuffer && (
              <div
                data-testid="sub-agent-stream-buffer"
                className="mt-2 rounded-xl border border-slate-200/80 bg-slate-50/80 px-3 py-2"
              >
                <div className="mb-1 text-[11px] font-semibold text-slate-600">
                  {subAgentRoleName ? `子员工 · ${subAgentRoleName}` : "子员工"}
                </div>
                <div className="prose prose-xs max-w-none text-slate-700">
                  <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                    {subAgentBuffer}
                  </ReactMarkdown>
                  <span className="animate-pulse text-slate-400">|</span>
                </div>
              </div>
            )}
            {streamItems.length > 0 && <span className="inline-block h-4 w-0.5 animate-[blink_1s_infinite] align-middle bg-blue-400 ml-0.5" />}
            {streamText.trim() && (
              <div className="mt-3 flex items-center justify-end gap-2">
                <button
                  type="button"
                  data-testid="assistant-copy-action-stream"
                  aria-label="复制回答"
                  title={copiedAssistantMessageKey === "stream" ? "已复制" : "复制回答"}
                  onClick={() => void onCopyAssistantMessage("stream", streamText)}
                  className="inline-flex h-9 w-9 items-center justify-center rounded-full text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 focus-visible:ring-offset-2"
                >
                  <CopyActionIcon copied={copiedAssistantMessageKey === "stream"} />
                </button>
              </div>
            )}
          </div>
        </motion.div>
      )}

      {askUserQuestion && (
        <div className="sticky top-0 z-20 flex justify-start">
          <div
            data-testid="ask-user-action-card"
            className="max-w-[80%] rounded-2xl border border-amber-300 bg-amber-50 px-4 py-3 text-sm shadow-sm"
          >
            <div className="mb-1 font-semibold text-amber-800">需要你的确认</div>
            <div className="mb-2 font-medium text-amber-700">{askUserQuestion}</div>
            {askUserOptions.length > 0 && (
              <div className="mb-2 flex flex-wrap gap-2">
                {askUserOptions.map((option, index) => (
                  <button
                    key={index}
                    onClick={() => onAnswerUser(option)}
                    className="rounded border border-amber-300 bg-amber-100 px-3 py-1 text-xs text-amber-800 transition-colors hover:bg-amber-200"
                  >
                    {option}
                  </button>
                ))}
              </div>
            )}
            <div className="flex gap-2">
              <input
                value={askUserAnswer}
                onChange={(event) => onAskUserAnswerChange(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    onAnswerUser(askUserAnswer);
                  }
                }}
                placeholder="输入回答..."
                className="flex-1 rounded border border-gray-200 bg-white px-2 py-1 text-xs focus:border-amber-500 focus:outline-none"
              />
              <button
                onClick={() => onAnswerUser(askUserAnswer)}
                disabled={!askUserAnswer.trim()}
                className="rounded bg-amber-500 px-3 py-1 text-xs transition-colors hover:bg-amber-600 disabled:bg-gray-200 disabled:text-gray-400"
              >
                回答
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
