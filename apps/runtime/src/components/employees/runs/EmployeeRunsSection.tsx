import { EmployeeGroupRunSummary } from "../../../types";
import {
  EmployeeHubRunFilter,
  formatEmployeeHubRunTimestamp,
  resolveEmployeeHubRunStatusLabel,
} from "../employeeHubOverview";

interface EmployeeRunsSectionProps {
  runFilterLabel: string;
  runFilter: EmployeeHubRunFilter;
  filteredRuns: EmployeeGroupRunSummary[];
  onClearRunFilter: () => void;
  onOpenGroupRunSession?: (sessionId: string, skillId: string) => Promise<void> | void;
  onOpenTeamsTab: () => void;
}

export function EmployeeRunsSection({
  runFilterLabel,
  runFilter,
  filteredRuns,
  onClearRunFilter,
  onOpenGroupRunSession,
  onOpenTeamsTab,
}: EmployeeRunsSectionProps) {
  return (
    <div className="rounded-xl border border-gray-200 bg-white p-4 space-y-3">
      <div>
        <div className="text-sm font-medium text-gray-900">最近运行</div>
        <div className="text-xs text-gray-500 mt-1">统一查看最近发起的团队任务与执行状态。</div>
      </div>
      {runFilter !== "all" && (
        <div className="flex items-center justify-between rounded border border-blue-100 bg-blue-50 px-3 py-2 text-xs text-blue-700">
          <span>当前筛选：{runFilterLabel}</span>
          <button type="button" onClick={onClearRunFilter} className="text-blue-700 hover:text-blue-800">
            清除筛选
          </button>
        </div>
      )}
      {filteredRuns.length === 0 ? (
        <div className="rounded-lg border border-dashed border-gray-300 px-3 py-4 text-xs text-gray-500">
          {runFilter === "all" ? "还没有运行记录，可先到团队页发起一次任务。" : "当前筛选下暂无运行记录。"}
        </div>
      ) : (
        <div className="space-y-2">
          {filteredRuns.map((run) => (
            <div key={run.id} className="flex items-center justify-between rounded-lg border border-gray-200 px-3 py-2">
              <div className="min-w-0">
                <div className="text-sm text-gray-900 truncate">{run.goal || "未命名任务"}</div>
                <div className="text-xs text-gray-500">
                  {run.group_name || "未命名团队"} · {resolveEmployeeHubRunStatusLabel(run.status)} · {formatEmployeeHubRunTimestamp(run.started_at)}
                </div>
              </div>
              <button
                type="button"
                onClick={() => {
                  if (run.session_id && run.session_skill_id) {
                    void onOpenGroupRunSession?.(run.session_id, run.session_skill_id);
                    return;
                  }
                  onOpenTeamsTab();
                }}
                className="text-xs text-blue-600 hover:text-blue-700"
              >
                {run.session_id && run.session_skill_id ? "进入会话" : "去团队查看"}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
