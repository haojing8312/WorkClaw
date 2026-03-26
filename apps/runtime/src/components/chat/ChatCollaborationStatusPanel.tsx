import { useState } from "react";

import type { ChatDelegationCardState } from "../../types";

type ChatCollaborationStatusPanelProps = {
  mainRoleName: string;
  primaryDelegationCard: ChatDelegationCardState | null;
  delegationHistoryCards: ChatDelegationCardState[];
  collaborationStatusText: string;
  completedDelegationCount: number;
  failedDelegationCount: number;
};

export function ChatCollaborationStatusPanel({
  mainRoleName,
  primaryDelegationCard,
  delegationHistoryCards,
  collaborationStatusText,
  completedDelegationCount,
  failedDelegationCount,
}: ChatCollaborationStatusPanelProps) {
  const [showDelegationHistory, setShowDelegationHistory] = useState(false);

  if (!mainRoleName && !primaryDelegationCard) {
    return null;
  }

  return (
    <div className="space-y-2">
      {(mainRoleName || primaryDelegationCard) && (
        <div
          data-testid="team-collab-status-bar"
          className="sticky top-0 z-10 max-w-[80%] rounded-xl border border-sky-200 bg-sky-50 px-4 py-2 text-xs text-sky-800"
        >
          <div className="flex items-center gap-2">
            <span className="inline-flex h-5 w-5 items-center justify-center rounded-full bg-sky-500 text-[10px] font-semibold text-white">
              主
            </span>
            <span>{collaborationStatusText}</span>
          </div>
          {(completedDelegationCount > 0 || failedDelegationCount > 0) && (
            <div className="mt-1 text-[11px] text-sky-700/90">
              {completedDelegationCount > 0 && <span>已完成 {completedDelegationCount} 次协作</span>}
              {completedDelegationCount > 0 && failedDelegationCount > 0 && <span> · </span>}
              {failedDelegationCount > 0 && <span>待处理失败 {failedDelegationCount} 次</span>}
            </div>
          )}
        </div>
      )}
      {primaryDelegationCard && (
        <div className="space-y-2">
          <div
            data-testid={`delegation-card-${primaryDelegationCard.id}`}
            className="max-w-[80%] rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-2 text-xs text-emerald-800"
          >
            <div className="font-medium">{`${primaryDelegationCard.fromRole} 已将任务分配给 ${primaryDelegationCard.toRole}`}</div>
            <div className="mt-1">
              {primaryDelegationCard.status === "running" && "执行中"}
              {primaryDelegationCard.status === "completed" && "已完成"}
              {primaryDelegationCard.status === "failed" && "失败"}
            </div>
          </div>
          {delegationHistoryCards.length > 0 && (
            <>
              <button
                data-testid="delegation-history-toggle"
                onClick={() => setShowDelegationHistory((prev) => !prev)}
                className="text-[11px] text-emerald-700 hover:text-emerald-800 underline underline-offset-2"
              >
                历史协作（{delegationHistoryCards.length}）
              </button>
              {showDelegationHistory && (
                <div data-testid="delegation-history-panel" className="space-y-2">
                  {delegationHistoryCards.map((card) => (
                    <div
                      key={card.id}
                      data-testid={`delegation-card-${card.id}`}
                      className="max-w-[80%] rounded-lg border border-gray-200 bg-white px-3 py-2 text-[11px] text-gray-700"
                    >
                      <div>{`${card.fromRole} -> ${card.toRole}`}</div>
                      <div className="mt-0.5 text-gray-500">
                        {card.status === "running" && "执行中"}
                        {card.status === "completed" && "已完成"}
                        {card.status === "failed" && "失败"}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
