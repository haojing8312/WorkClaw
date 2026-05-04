import { Fragment, memo, type MutableRefObject, useMemo } from "react";
import { motion } from "framer-motion";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import { ThinkingBlock } from "../ThinkingBlock";
import { TaskJourneySummary } from "../chat-journey/TaskJourneySummary";
import type { Message, SessionRunProjection, SessionToolManifestEntry, StreamItem } from "../../types";
import { ChatAskUserActionCard } from "./ChatAskUserActionCard";
import { createChatMarkdownComponents } from "./chatMarkdownComponents";
import { ChatStreamingAssistantBubble } from "./ChatStreamingAssistantBubble";
import { renderUserContentParts } from "./renderUserContentParts";
import {
  getRunFailureTechnicalToggleLabel,
  renderChatRunFailureCard,
  renderChatStreamItems,
} from "./chatMessageRailHelpers";

type TaskJourneyModel = ReturnType<
  typeof import("../chat-side-panel/view-model").buildTaskJourneyViewModel
>;

type ChatMessageRailProps = {
  renderedMessages: Message[];
  visibleStartIndex: number;
  topSpacerHeight: number;
  bottomSpacerHeight: number;
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

function ChatMessageRailImpl({
  renderedMessages,
  visibleStartIndex,
  topSpacerHeight,
  bottomSpacerHeight,
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
  return (
    <>
      {topSpacerHeight > 0 && <div aria-hidden="true" style={{ height: topSpacerHeight }} />}
      {renderedMessages.map((message, index) => {
        const absoluteIndex = visibleStartIndex + index;
        const isLatest = absoluteIndex === visibleStartIndex + renderedMessages.length - 1;
        const isSessionFocusTarget = highlightedMessageIndex === absoluteIndex;
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
                messageElementRefs.current[absoluteIndex] = node;
              }}
              data-testid={`chat-message-${absoluteIndex}`}
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
                        ? () => onToggleThinkingBlock(`message-${message.id || absoluteIndex}`)
                        : undefined
                    }
                    toggleTestId={`thinking-block-toggle-${message.id || absoluteIndex}`}
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
                      onClick={() => void onCopyAssistantMessage(`message-${message.id || absoluteIndex}`, message.content)}
                      className="inline-flex h-9 w-9 items-center justify-center rounded-full text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 focus-visible:ring-offset-2"
                    >
                      <CopyActionIcon copied={copiedAssistantMessageKey === `message-${message.id || absoluteIndex}`} />
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
      {bottomSpacerHeight > 0 && <div aria-hidden="true" style={{ height: bottomSpacerHeight }} />}

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
        <ChatStreamingAssistantBubble
          showStreamingThinkingState={showStreamingThinkingState}
          streamReasoning={streamReasoning}
          expandedThinkingKeys={expandedThinkingKeys}
          onToggleThinkingBlock={onToggleThinkingBlock}
          streamItems={streamItems}
          toolManifest={toolManifest}
          subAgentBuffer={subAgentBuffer}
          subAgentRoleName={subAgentRoleName}
          copiedAssistantMessageKey={copiedAssistantMessageKey}
          onCopyAssistantMessage={onCopyAssistantMessage}
          CopyActionIcon={CopyActionIcon}
          onOpenExternalLink={onOpenExternalLink}
        />
      )}

      {askUserQuestion && (
        <ChatAskUserActionCard
          askUserQuestion={askUserQuestion}
          askUserOptions={askUserOptions}
          askUserAnswer={askUserAnswer}
          onAskUserAnswerChange={onAskUserAnswerChange}
          onAnswerUser={onAnswerUser}
        />
      )}
    </>
  );
}

export const ChatMessageRail = memo(ChatMessageRailImpl, (prev, next) => {
  return (
    prev.renderedMessages === next.renderedMessages &&
    prev.visibleStartIndex === next.visibleStartIndex &&
    prev.topSpacerHeight === next.topSpacerHeight &&
    prev.bottomSpacerHeight === next.bottomSpacerHeight &&
    prev.highlightedMessageIndex === next.highlightedMessageIndex &&
    prev.messageElementRefs === next.messageElementRefs &&
    prev.expandedThinkingKeys === next.expandedThinkingKeys &&
    prev.failedRunsByAssistantMessageId === next.failedRunsByAssistantMessageId &&
    prev.failedRunsByUserMessageId === next.failedRunsByUserMessageId &&
    prev.renderInstallCandidates === next.renderInstallCandidates &&
    prev.extractInstallCandidates === next.extractInstallCandidates &&
    prev.copiedAssistantMessageKey === next.copiedAssistantMessageKey &&
    prev.expandedRunDetailIds === next.expandedRunDetailIds &&
    prev.streaming === next.streaming &&
    prev.orphanFailedRuns === next.orphanFailedRuns &&
    prev.showStreamingAssistantBubble === next.showStreamingAssistantBubble &&
    prev.showStreamingThinkingState === next.showStreamingThinkingState &&
    prev.streamReasoning === next.streamReasoning &&
    prev.streamItems === next.streamItems &&
    prev.toolManifest === next.toolManifest &&
    prev.subAgentBuffer === next.subAgentBuffer &&
    prev.subAgentRoleName === next.subAgentRoleName &&
    prev.askUserQuestion === next.askUserQuestion &&
    prev.askUserOptions === next.askUserOptions &&
    prev.askUserAnswer === next.askUserAnswer
  );
});
