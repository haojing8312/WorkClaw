import { AgentEmployee, EmployeeGroup, EmployeeGroupRule } from "../../../types";
import {
  EmployeeGroupExecutionMode,
  EmployeeGroupReviewMode,
  EmployeeGroupVisibilityMode,
  EmployeeHubTeamFilter,
  resolveEmployeeHubDisplayName,
} from "../employeeHubOverview";

type GroupTemplateRole = {
  role_type?: string;
  employee_key?: string;
  employee_id?: string;
};

type GroupTemplateConfig = {
  roles?: GroupTemplateRole[];
};

interface EmployeeTeamsSectionProps {
  employees: AgentEmployee[];
  employeeLabelById: Map<string, string>;
  draft: {
    groupName: string;
    groupCoordinatorId: string;
    groupMemberIds: string[];
    groupEntryId: string;
    groupPlannerId: string;
    groupReviewerId: string;
    groupReviewMode: EmployeeGroupReviewMode;
    groupExecutionMode: EmployeeGroupExecutionMode;
    groupVisibilityMode: EmployeeGroupVisibilityMode;
    groupSubmitting: boolean;
  };
  groupsState: {
    teamFilter: EmployeeHubTeamFilter;
    teamFilterLabel: string;
    filteredGroups: EmployeeGroup[];
    groupDeletingId: string | null;
    groupRunGoalById: Record<string, string>;
    groupRunSubmittingId: string | null;
    groupRunReportById: Record<string, string>;
    groupRulesById: Record<string, EmployeeGroupRule[]>;
    cloningGroupId: string | null;
  };
  actions: {
    onGroupNameChange: (value: string) => void;
    onGroupCoordinatorChange: (value: string) => void;
    onGroupEntryChange: (value: string) => void;
    onGroupPlannerChange: (value: string) => void;
    onGroupReviewerChange: (value: string) => void;
    onGroupReviewModeChange: (value: EmployeeGroupReviewMode) => void;
    onGroupExecutionModeChange: (value: EmployeeGroupExecutionMode) => void;
    onGroupVisibilityModeChange: (value: EmployeeGroupVisibilityMode) => void;
    onGroupMemberToggle: (employeeCode: string, checked: boolean) => void;
    onCreateEmployeeGroup: () => void;
    onDeleteEmployeeGroup: (groupId: string) => void;
    onCloneEmployeeGroup: (group: EmployeeGroup) => void;
    onStartEmployeeGroupRun: (groupId: string) => void;
    onGroupRunGoalChange: (groupId: string, value: string) => void;
    onClearTeamFilter: () => void;
  };
}

function employeeKey(employee: AgentEmployee): string {
  return (employee.employee_id || employee.role_id || "").trim();
}

function parseGroupTemplateConfig(raw?: string | null): GroupTemplateConfig {
  if (!raw?.trim()) return {};
  try {
    return JSON.parse(raw) as GroupTemplateConfig;
  } catch {
    return {};
  }
}

function groupRoleLabel(roleType: string): string {
  const normalized = roleType.trim().toLowerCase();
  switch (normalized) {
    case "entry":
      return "入口";
    case "planner":
      return "规划";
    case "reviewer":
      return "审议";
    case "coordinator":
      return "协调";
    case "executor":
      return "执行";
    default:
      return normalized || "角色";
  }
}

export function EmployeeTeamsSection({
  employees,
  employeeLabelById,
  draft,
  groupsState,
  actions,
}: EmployeeTeamsSectionProps) {
  const resolveEmployeeDisplayName = (employeeId: string) =>
    resolveEmployeeHubDisplayName(employeeLabelById, employeeId);

  return (
    <div className="bg-white border border-gray-200 rounded-xl p-4 space-y-3">
      <div className="flex items-center justify-between gap-2">
        <div>
          <div className="text-sm font-medium text-gray-900">拉群协作（最多 10 人）</div>
          <div className="text-xs text-gray-500 mt-1">创建协作群后，可由协调员按轮次调度成员执行。</div>
        </div>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
        <input
          data-testid="employee-group-name"
          className="border border-gray-200 rounded px-2 py-1.5 text-sm"
          placeholder="群组名称"
          value={draft.groupName}
          onChange={(e) => actions.onGroupNameChange(e.target.value)}
        />
        <select
          data-testid="employee-group-coordinator"
          className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
          value={draft.groupCoordinatorId}
          onChange={(e) => actions.onGroupCoordinatorChange(e.target.value)}
        >
          <option value="">选择协调员</option>
          {employees.map((item) => {
            const id = (item.employee_id || item.role_id || "").trim();
            if (!id) return null;
            return (
              <option key={item.id} value={id}>
                {item.name}（{id}）
              </option>
            );
          })}
        </select>
        <button
          type="button"
          data-testid="employee-group-create"
          disabled={draft.groupSubmitting}
          onClick={actions.onCreateEmployeeGroup}
          className="h-9 rounded bg-indigo-600 hover:bg-indigo-700 disabled:bg-indigo-300 text-white text-sm"
        >
          {draft.groupSubmitting ? "创建中..." : "创建协作群"}
        </button>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
        <select
          data-testid="employee-group-entry"
          className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
          value={draft.groupEntryId}
          onChange={(e) => actions.onGroupEntryChange(e.target.value)}
        >
          <option value="">入口员工（默认协调员）</option>
          {employees.map((item) => {
            const id = employeeKey(item);
            if (!id) return null;
            return (
              <option key={`${item.id}-entry`} value={id}>
                {item.name}（{id}）
              </option>
            );
          })}
        </select>
        <select
          data-testid="employee-group-planner"
          className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
          value={draft.groupPlannerId}
          onChange={(e) => actions.onGroupPlannerChange(e.target.value)}
        >
          <option value="">规划员工（默认入口员工）</option>
          {employees.map((item) => {
            const id = employeeKey(item);
            if (!id) return null;
            return (
              <option key={`${item.id}-planner`} value={id}>
                {item.name}（{id}）
              </option>
            );
          })}
        </select>
        <select
          data-testid="employee-group-reviewer"
          className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
          value={draft.groupReviewerId}
          onChange={(e) => actions.onGroupReviewerChange(e.target.value)}
        >
          <option value="">审核员工（可选）</option>
          {employees.map((item) => {
            const id = employeeKey(item);
            if (!id) return null;
            return (
              <option key={`${item.id}-reviewer`} value={id}>
                {item.name}（{id}）
              </option>
            );
          })}
        </select>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
        <select
          data-testid="employee-group-review-mode"
          className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
          value={draft.groupReviewMode}
          onChange={(e) => actions.onGroupReviewModeChange(e.target.value as EmployeeGroupReviewMode)}
        >
          <option value="none">无需审核</option>
          <option value="soft">建议审核</option>
          <option value="hard">强制审核</option>
        </select>
        <select
          data-testid="employee-group-execution-mode"
          className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
          value={draft.groupExecutionMode}
          onChange={(e) => actions.onGroupExecutionModeChange(e.target.value as EmployeeGroupExecutionMode)}
        >
          <option value="sequential">顺序执行</option>
          <option value="parallel">并行执行</option>
        </select>
        <select
          data-testid="employee-group-visibility-mode"
          className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
          value={draft.groupVisibilityMode}
          onChange={(e) => actions.onGroupVisibilityModeChange(e.target.value as EmployeeGroupVisibilityMode)}
        >
          <option value="internal">内部可见</option>
          <option value="shared">协作共享</option>
        </select>
      </div>
      <div className="rounded border border-gray-200 p-2">
        <div className="text-[11px] text-gray-500 mb-1">选择成员（{draft.groupMemberIds.length}/10）</div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-1.5">
          {employees.map((item) => {
            const employeeCode = employeeKey(item);
            if (!employeeCode) return null;
            const checked = draft.groupMemberIds.includes(employeeCode);
            return (
              <label key={item.id} className="flex items-center gap-2 text-xs text-gray-700">
                <input
                  data-testid={`employee-group-member-${item.id}`}
                  type="checkbox"
                  checked={checked}
                  onChange={(e) => actions.onGroupMemberToggle(employeeCode, e.target.checked)}
                />
                <span>
                  {item.name}（{employeeCode}）
                </span>
              </label>
            );
          })}
        </div>
      </div>
      <div className="space-y-1">
        {groupsState.teamFilter !== "all" && (
          <div className="mb-2 flex items-center justify-between rounded border border-blue-100 bg-blue-50 px-2 py-1.5 text-[11px] text-blue-700">
            <span>当前筛选：{groupsState.teamFilterLabel}</span>
            <button type="button" onClick={actions.onClearTeamFilter} className="text-blue-700 hover:text-blue-800">
              清除筛选
            </button>
          </div>
        )}
        {groupsState.filteredGroups.length === 0 ? (
          <div className="text-xs text-gray-500">{groupsState.teamFilter === "all" ? "暂无协作群组" : "当前筛选下暂无团队"}</div>
        ) : (
          groupsState.filteredGroups.map((group) => (
            <div key={group.id} data-testid={`employee-group-item-${group.id}`} className="rounded border border-gray-200 px-2 py-1.5 space-y-2">
              {(() => {
                const templateId = group.template_id?.trim() || "";
                const entryEmployeeId = group.entry_employee_id?.trim() || group.coordinator_employee_id;
                const reviewMode = group.review_mode?.trim() || "none";
                const executionMode = group.execution_mode?.trim() || "sequential";
                const visibilityMode = group.visibility_mode?.trim() || "internal";
                const groupConfig = parseGroupTemplateConfig(group.config_json);
                const groupRules = groupsState.groupRulesById[group.id] || [];
                return (
                  <>
                    <div className="flex items-center justify-between gap-2">
                      <div className="text-xs text-gray-700">
                        <span className="font-medium">{group.name}</span>
                        <span className="text-gray-500">
                          {" "}
                          · 协调员 {group.coordinator_employee_id} · {group.member_count} 人
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <button
                          type="button"
                          data-testid={`employee-team-clone-${group.id}`}
                          onClick={() => actions.onCloneEmployeeGroup(group)}
                          disabled={groupsState.cloningGroupId === group.id}
                          className="h-7 px-2 rounded border border-blue-200 hover:bg-blue-50 disabled:bg-gray-100 text-blue-700 text-xs"
                        >
                          {groupsState.cloningGroupId === group.id ? "复制中..." : "复制模板"}
                        </button>
                        <button
                          type="button"
                          data-testid={`employee-group-delete-${group.id}`}
                          onClick={() => actions.onDeleteEmployeeGroup(group.id)}
                          disabled={groupsState.groupDeletingId === group.id}
                          className="h-7 px-2 rounded border border-red-200 hover:bg-red-50 disabled:bg-gray-100 text-red-600 text-xs"
                        >
                          {groupsState.groupDeletingId === group.id ? "删除中..." : "删除"}
                        </button>
                      </div>
                    </div>
                    {(group.is_bootstrap_seeded || templateId) && (
                      <div
                        data-testid={`employee-team-seeded-banner-${group.id}`}
                        className="rounded border border-amber-200 bg-amber-50 px-2 py-1 text-[11px] text-amber-800"
                      >
                        已预置默认团队 · 模板 {templateId || "custom"}
                      </div>
                    )}
                    <div className="flex flex-wrap gap-1.5 text-[11px] text-gray-700">
                      <span className="rounded border border-gray-200 bg-gray-50 px-2 py-0.5">
                        入口：{resolveEmployeeDisplayName(entryEmployeeId)}
                      </span>
                      <span className="rounded border border-gray-200 bg-gray-50 px-2 py-0.5">
                        协调：{resolveEmployeeDisplayName(group.coordinator_employee_id)}
                      </span>
                      <span className="rounded border border-gray-200 bg-gray-50 px-2 py-0.5">
                        审核：{reviewMode}
                      </span>
                      <span className="rounded border border-gray-200 bg-gray-50 px-2 py-0.5">
                        执行：{executionMode}
                      </span>
                      <span className="rounded border border-gray-200 bg-gray-50 px-2 py-0.5">
                        可见性：{visibilityMode}
                      </span>
                    </div>
                    {groupConfig.roles?.length ? (
                      <div className="rounded border border-gray-100 bg-gray-50 px-2 py-1.5 text-[11px] text-gray-700">
                        <div className="text-gray-500 mb-1">角色分工</div>
                        <div className="flex flex-wrap gap-1.5">
                          {(groupConfig.roles || []).map((role, index) => (
                            <span
                              key={`${group.id}-role-${role.role_type || "role"}-${role.employee_id || role.employee_key || index}`}
                              className="rounded border border-indigo-100 bg-indigo-50 px-2 py-0.5 text-indigo-700"
                            >
                              {groupRoleLabel(role.role_type || "")}：
                              {resolveEmployeeDisplayName(role.employee_id || role.employee_key || "")}
                            </span>
                          ))}
                        </div>
                      </div>
                    ) : null}
                    {groupRules.length > 0 && (
                      <div className="rounded border border-gray-100 bg-white px-2 py-1.5">
                        <div className="text-[11px] text-gray-500 mb-1">协作规则</div>
                        <div className="space-y-1">
                          {groupRules.map((rule) => (
                            <div
                              key={rule.id}
                              data-testid={`employee-group-rule-${group.id}-${rule.id}`}
                              className="text-[11px] text-gray-700"
                            >
                              {resolveEmployeeDisplayName(rule.from_employee_id)} -&gt; {resolveEmployeeDisplayName(rule.to_employee_id)} · {rule.relation_type} · {rule.phase_scope || "all"}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                    <div className="flex items-center gap-2">
                      <input
                        data-testid={`employee-group-run-goal-${group.id}`}
                        className="flex-1 border border-gray-200 rounded px-2 py-1.5 text-xs"
                        placeholder="给该团队发送协作指令"
                        value={groupsState.groupRunGoalById[group.id] || ""}
                        onChange={(e) => actions.onGroupRunGoalChange(group.id, e.target.value)}
                      />
                      <button
                        type="button"
                        data-testid={`employee-group-run-start-${group.id}`}
                        onClick={() => actions.onStartEmployeeGroupRun(group.id)}
                        disabled={groupsState.groupRunSubmittingId === group.id}
                        className="h-7 px-2.5 rounded bg-indigo-600 hover:bg-indigo-700 disabled:bg-indigo-300 text-white text-xs"
                      >
                        {groupsState.groupRunSubmittingId === group.id ? "执行中..." : "以团队模式发起任务"}
                      </button>
                    </div>
                    {groupsState.groupRunReportById[group.id] && (
                      <div
                        data-testid={`employee-group-run-report-${group.id}`}
                        className="rounded border border-indigo-100 bg-indigo-50 px-2 py-1.5 text-[11px] text-indigo-900 whitespace-pre-wrap"
                      >
                        {groupsState.groupRunReportById[group.id]}
                      </div>
                    )}
                  </>
                );
              })()}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
