import type { ComponentProps } from "react";

import { ChatGroupRunBoard } from "./ChatGroupRunBoard";

type ChatGroupRunBoardProps = ComponentProps<typeof ChatGroupRunBoard>;

type ChatGroupRunSectionProps = ChatGroupRunBoardProps & {
  shouldShowTeamEntryEmptyState: boolean;
  sessionDisplaySubtitle: string;
};

export function ChatGroupRunSection({
  shouldShowTeamEntryEmptyState,
  sessionDisplaySubtitle,
  ...boardProps
}: ChatGroupRunSectionProps) {
  return (
    <>
      <ChatGroupRunBoard {...boardProps} />
      {shouldShowTeamEntryEmptyState && (
        <div
          data-testid="team-entry-empty-state"
          className="max-w-[80%] rounded-2xl border border-sky-200 bg-sky-50 px-5 py-4 text-sm text-sky-950 shadow-sm"
        >
          <div className="text-sm font-semibold">团队已就绪</div>
          <div className="mt-1 text-xs text-sky-800">
            {sessionDisplaySubtitle || "当前团队"} 已进入协作模式，等待你下达第一条任务。
          </div>
          <div className="mt-3 rounded-xl border border-sky-100 bg-white/80 px-3 py-2 text-[11px] text-sky-900">
            适合提交需要拆分、审核、执行和汇总的复杂任务。直接在下方输入目标即可开始团队协作。
          </div>
        </div>
      )}
    </>
  );
}
