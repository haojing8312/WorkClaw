import { useMemo } from "react";
import { AgentEmployee, EmployeeGroup, EmployeeGroupRunSummary } from "../../../types";
import {
  buildEmployeeHubMetrics,
  buildEmployeeHubPendingItems,
  EmployeeHubEmployeeFilter,
  EmployeeHubRunFilter,
  EmployeeHubTeamFilter,
  formatEmployeeHubRunTimestamp,
  resolveEmployeeHubDisplayName,
  resolveEmployeeHubRunStatusLabel,
} from "../employeeHubOverview";

interface EmployeeOverviewSectionProps {
  employees: AgentEmployee[];
  groups: EmployeeGroup[];
  runs: EmployeeGroupRunSummary[];
  employeeLabelById: Map<string, string>;
  onSelectEmployee: (id: string) => void;
  onOpenEmployeesTab: (filter: EmployeeHubEmployeeFilter) => void;
  onOpenTeamsTab: (filter: EmployeeHubTeamFilter) => void;
  onOpenRunsTab: (filter: EmployeeHubRunFilter) => void;
  onOpenSettingsTab: () => void;
  onOpenGroupRunSession?: (sessionId: string, skillId: string) => Promise<void> | void;
}

export function EmployeeOverviewSection({
  employees,
  groups,
  runs,
  employeeLabelById,
  onSelectEmployee,
  onOpenEmployeesTab,
  onOpenTeamsTab,
  onOpenRunsTab,
  onOpenSettingsTab,
  onOpenGroupRunSession,
}: EmployeeOverviewSectionProps) {
  const overviewMetrics = useMemo(
    () => buildEmployeeHubMetrics({ employees, groups, runs }),
    [employees, groups, runs],
  );
  const pendingItems = useMemo(
    () => buildEmployeeHubPendingItems({ employees, groups }),
    [employees, groups],
  );
  const recentEmployees = employees.slice(0, 5);
  const recentGroups = groups.slice(0, 5);
  const recentRunsForOverview = runs.slice(0, 5);

  return (
    <div
      id="employee-hub-panel-overview"
      role="tabpanel"
      aria-labelledby="employee-hub-tab-overview"
      className="space-y-4"
    >
      <div className="grid grid-cols-1 gap-3 md:grid-cols-5">
        <button
          type="button"
          aria-label="查看全部员工"
          onClick={() => onOpenEmployeesTab("all")}
          className="rounded-xl border border-gray-200 bg-white p-4 text-left hover:bg-gray-50"
        >
          <div className="text-xs text-gray-500">员工总数</div>
          <div
            data-testid="employee-overview-metric-employees"
            className="mt-2 text-2xl font-semibold text-gray-900"
          >
            {overviewMetrics.employees}
          </div>
        </button>
        <button
          type="button"
          aria-label="查看全部团队"
          onClick={() => onOpenTeamsTab("all")}
          className="rounded-xl border border-gray-200 bg-white p-4 text-left hover:bg-gray-50"
        >
          <div className="text-xs text-gray-500">团队总数</div>
          <div data-testid="employee-overview-metric-teams" className="mt-2 text-2xl font-semibold text-gray-900">
            {overviewMetrics.teams}
          </div>
        </button>
        <button
          type="button"
          aria-label="查看可用员工"
          onClick={() => onOpenEmployeesTab("available")}
          className="rounded-xl border border-gray-200 bg-white p-4 text-left hover:bg-gray-50"
        >
          <div className="text-xs text-gray-500">可用员工</div>
          <div
            data-testid="employee-overview-metric-available-employees"
            className="mt-2 text-2xl font-semibold text-gray-900"
          >
            {overviewMetrics.availableEmployees}
          </div>
        </button>
        <button
          type="button"
          aria-label="查看运行中团队"
          onClick={() => onOpenRunsTab("running")}
          className="rounded-xl border border-gray-200 bg-white p-4 text-left hover:bg-gray-50"
        >
          <div className="text-xs text-gray-500">运行中团队</div>
          <div data-testid="employee-overview-metric-running-teams" className="mt-2 text-2xl font-semibold text-gray-900">
            {overviewMetrics.runningTeams}
          </div>
        </button>
        <button
          type="button"
          aria-label="查看待处理事项"
          onClick={() => {
            const first = pendingItems[0];
            if (!first) return;
            if (first.id === "incomplete-team") {
              onOpenTeamsTab("incomplete-team");
              return;
            }
            onOpenEmployeesTab(first.id);
          }}
          className="rounded-xl border border-gray-200 bg-white p-4 text-left hover:bg-gray-50"
        >
          <div className="text-xs text-gray-500">待处理事项</div>
          <div data-testid="employee-overview-metric-pending-items" className="mt-2 text-2xl font-semibold text-gray-900">
            {overviewMetrics.pendingItems}
          </div>
        </button>
      </div>

      <div className="rounded-xl border border-gray-200 bg-white p-4 space-y-3">
        <div className="flex items-center justify-between gap-2">
          <div>
            <div className="text-sm font-medium text-gray-900">待处理事项</div>
            <div className="text-xs text-gray-500 mt-1">优先处理影响员工可用性和团队协作的问题。</div>
          </div>
          <button type="button" onClick={onOpenSettingsTab} className="text-xs text-blue-600 hover:text-blue-700">
            去设置
          </button>
        </div>
        {pendingItems.length === 0 ? (
          <div className="rounded-lg border border-emerald-100 bg-emerald-50 px-3 py-2 text-xs text-emerald-700">
            当前配置完整，可直接开始任务。
          </div>
        ) : (
          <div className="space-y-2">
            {pendingItems.map((item) => (
              <div
                key={item.id}
                className="flex items-center justify-between gap-3 rounded-lg border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-800"
              >
                <span>{item.label}</span>
                <button
                  type="button"
                  onClick={() => {
                    if (item.id === "incomplete-team") {
                      onOpenTeamsTab("incomplete-team");
                      return;
                    }
                    onOpenEmployeesTab(item.id);
                  }}
                  className="rounded border border-amber-200 bg-white px-2 py-1 text-[11px] text-amber-700 hover:bg-amber-100"
                >
                  去处理
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <div className="rounded-xl border border-gray-200 bg-white p-4 space-y-3">
          <div className="flex items-center justify-between gap-2">
            <div className="text-sm font-medium text-gray-900">员工概览</div>
            <button type="button" onClick={() => onOpenEmployeesTab("all")} className="text-xs text-blue-600 hover:text-blue-700">
              查看全部员工
            </button>
          </div>
          <div className="space-y-2">
            {recentEmployees.length === 0 ? (
              <div className="rounded-lg border border-dashed border-gray-300 px-3 py-4 text-xs text-gray-500 space-y-1">
                <div>还没有智能体员工，先创建第一个员工。</div>
                <div>已移除手动创建流程，请通过「智能体员工助手」对话式完成创建与配置。</div>
              </div>
            ) : (
              recentEmployees.map((employee) => (
                <button
                  key={employee.id}
                  type="button"
                  onClick={() => {
                    onSelectEmployee(employee.id);
                    onOpenEmployeesTab("all");
                  }}
                  className="flex w-full items-center justify-between rounded-lg border border-gray-200 px-3 py-2 text-left hover:bg-gray-50"
                >
                  <div>
                    <div className="text-sm text-gray-900">{employee.name}</div>
                    <div className="text-xs text-gray-500">{employee.employee_id || employee.role_id || "未设置员工编号"}</div>
                  </div>
                  <div className="text-xs text-gray-500">{employee.enabled ? "正常" : "停用"}</div>
                </button>
              ))
            )}
          </div>
        </div>

        <div className="rounded-xl border border-gray-200 bg-white p-4 space-y-3">
          <div className="flex items-center justify-between gap-2">
            <div className="text-sm font-medium text-gray-900">团队概览</div>
            <button type="button" onClick={() => onOpenTeamsTab("all")} className="text-xs text-blue-600 hover:text-blue-700">
              查看全部团队
            </button>
          </div>
          <div className="space-y-2">
            {recentGroups.length === 0 ? (
              <div className="rounded-lg border border-dashed border-gray-300 px-3 py-4 text-xs text-gray-500">
                还没有团队，创建团队后可分工协作。
              </div>
            ) : (
              recentGroups.map((group) => (
                <button
                  key={group.id}
                  type="button"
                  onClick={() => onOpenTeamsTab("all")}
                  className="flex w-full items-center justify-between rounded-lg border border-gray-200 px-3 py-2 text-left hover:bg-gray-50"
                >
                  <div>
                    <div className="text-sm text-gray-900">{group.name}</div>
                    <div className="text-xs text-gray-500">
                      {group.member_count || group.member_employee_ids.length} 人 · {resolveEmployeeHubDisplayName(employeeLabelById, group.coordinator_employee_id)}
                    </div>
                  </div>
                  <div className="text-xs text-gray-500">查看详情</div>
                </button>
              ))
            )}
          </div>
        </div>
      </div>

      <div className="rounded-xl border border-gray-200 bg-white p-4 space-y-3">
        <div className="flex items-center justify-between gap-2">
          <div>
            <div className="text-sm font-medium text-gray-900">最近运行</div>
            <div className="text-xs text-gray-500 mt-1">最近发起的团队任务会集中展示在这里。</div>
          </div>
          <button type="button" onClick={() => onOpenRunsTab("all")} className="text-xs text-blue-600 hover:text-blue-700">
            查看全部
          </button>
        </div>
        {recentRunsForOverview.length === 0 ? (
          <div className="rounded-lg border border-dashed border-gray-300 px-3 py-4 text-xs text-gray-500">
            还没有运行记录，发起一次团队任务后会显示在这里。
          </div>
        ) : (
          <div className="space-y-2">
            {recentRunsForOverview.map((run) => (
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
                    onOpenTeamsTab("all");
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
    </div>
  );
}
