import { useEffect, useMemo, useState } from "react";
import {
  AgentEmployee,
  SkillManifest,
  UpsertAgentEmployeeInput,
} from "../../types";
import { RiskConfirmDialog } from "../RiskConfirmDialog";
import { EmployeeHubTabNav, type EmployeeHubTab as EmployeeHubTabNavItem } from "./EmployeeHubTabNav";
import {
  EmployeeHubEmployeeFilter,
  EmployeeHubRunFilter,
  EmployeeHubTeamFilter,
  matchesEmployeeHubEmployeeFilter,
} from "./employeeHubOverview";
import { EmployeeHubEmployeesSection } from "./employee-details/EmployeeHubEmployeesSection";
import { EmployeeOverviewSection } from "./overview/EmployeeOverviewSection";
import { EmployeeRunsSection } from "./runs/EmployeeRunsSection";
import { EmployeeHubSettingsSection } from "./settings/EmployeeHubSettingsSection";
import { EmployeeTeamsSection } from "./teams/EmployeeTeamsSection";
import { useEmployeeHubGroups } from "./hooks/useEmployeeHubGroups";
import { toEmployeeHubFeishuRuntimeStatus, useEmployeeHubFeishu } from "./hooks/useEmployeeHubFeishu";
import { useEmployeeHubRuntimeState } from "./hooks/useEmployeeHubRuntimeState";
import { useEmployeeHubTools } from "./hooks/useEmployeeHubTools";
import { brandDefaultWorkspacePathExample } from "../../lib/branding";

export interface EmployeeHubViewProps {
  employees: AgentEmployee[];
  skills: SkillManifest[];
  initialTab?: EmployeeHubTab;
  selectedEmployeeId: string | null;
  onSelectEmployee: (id: string) => void;
  onSaveEmployee?: (input: UpsertAgentEmployeeInput) => Promise<void>;
  onRefreshEmployees?: () => Promise<AgentEmployee[] | void> | AgentEmployee[] | void;
  onDeleteEmployee: (employeeId: string) => Promise<void>;
  onSetAsMainAndEnter: (employeeId: string) => void;
  onStartTaskWithEmployee: (employeeId: string) => Promise<void> | void;
  onOpenGroupRunSession?: (sessionId: string, skillId: string) => Promise<void> | void;
  onEmployeeGroupsChanged?: () => Promise<void> | void;
  onOpenEmployeeCreatorSkill?: (options?: { mode?: "create" | "update"; employeeId?: string }) => Promise<void> | void;
  onOpenFeishuSettings?: () => void;
  highlightEmployeeId?: string | null;
  highlightMessage?: string | null;
  onDismissHighlight?: () => void;
}

export type EmployeeHubTab = EmployeeHubTabNavItem;

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function employeeKey(employee: AgentEmployee): string {
  return (employee.employee_id || employee.role_id || "").trim();
}

export function EmployeeHubView({
  employees,
  skills,
  initialTab,
  selectedEmployeeId,
  onSelectEmployee,
  onRefreshEmployees,
  onDeleteEmployee,
  onSetAsMainAndEnter,
  onStartTaskWithEmployee,
  onOpenGroupRunSession,
  onEmployeeGroupsChanged,
  onOpenEmployeeCreatorSkill,
  onOpenFeishuSettings,
  highlightEmployeeId,
  highlightMessage,
  onDismissHighlight,
}: EmployeeHubViewProps) {
  const [activeTab, setActiveTab] = useState<EmployeeHubTab>(initialTab ?? (selectedEmployeeId ? "employees" : "overview"));
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [pendingDeleteEmployee, setPendingDeleteEmployee] = useState<{ id: string; name: string } | null>(null);
  const [employeeScopeOverrides, setEmployeeScopeOverrides] = useState<Record<string, string[]>>({});
  const [employeeFilter, setEmployeeFilter] = useState<EmployeeHubEmployeeFilter>("all");
  const {
    groupName,
    setGroupName,
    groupCoordinatorId,
    setGroupCoordinatorId,
    groupMemberIds,
    groupEntryId,
    setGroupEntryId,
    groupPlannerId,
    setGroupPlannerId,
    groupReviewerId,
    setGroupReviewerId,
    groupReviewMode,
    setGroupReviewMode,
    groupExecutionMode,
    setGroupExecutionMode,
    groupVisibilityMode,
    setGroupVisibilityMode,
    employeeGroups,
    recentRuns,
    teamFilter,
    setTeamFilter,
    runFilter,
    setRunFilter,
    groupSubmitting,
    groupDeletingId,
    groupRunGoalById,
    groupRunSubmittingId,
    groupRunReportById,
    groupRulesById,
    cloningGroupId,
    filteredGroups,
    filteredRuns,
    createEmployeeGroup,
    deleteEmployeeGroup,
    startEmployeeGroupRun,
    cloneEmployeeGroup,
    handleGroupMemberToggle,
    handleGroupRunGoalChange,
  } = useEmployeeHubGroups({
    employees,
    onOpenGroupRunSession,
    onEmployeeGroupsChanged,
    setMessage,
  });
  const effectiveEmployees = useMemo(
    () =>
      employees.map((employee) => {
        const override = employeeScopeOverrides[employee.id];
        return override ? { ...employee, enabled_scopes: override } : employee;
      }),
    [employeeScopeOverrides, employees],
  );
  const selectedEmployee = useMemo(
    () => effectiveEmployees.find((item) => item.id === selectedEmployeeId) ?? null,
    [effectiveEmployees, selectedEmployeeId],
  );
  const selectedEmployeeMemoryId = useMemo(
    () => (selectedEmployee?.employee_id || selectedEmployee?.role_id || "").trim(),
    [selectedEmployee],
  );
  const {
    globalDefaultWorkDir,
    setGlobalDefaultWorkDir,
    savingGlobalWorkDir,
    officialFeishuRuntimeStatus,
    saveGlobalDefaultWorkDir,
  } = useEmployeeHubRuntimeState({
    setMessage,
  });
  const {
    memoryScopeSkillId,
    setMemoryScopeSkillId,
    memoryStats,
    memoryLoading,
    memoryActionLoading,
    pendingClearMemory,
    setPendingClearMemory,
    profileView,
    profileLoading,
    refreshEmployeeMemoryStats,
    exportEmployeeMemory,
    confirmClearEmployeeMemory,
  } = useEmployeeHubTools({
    selectedEmployee,
    selectedEmployeeId,
    selectedEmployeeMemoryId,
    setMessage,
  });
  const {
    routingBindings,
    savingFeishuAssociation,
    resolveFeishuStatus,
    saveFeishuAssociation,
  } = useEmployeeHubFeishu({
    selectedEmployee,
    onRefreshEmployees,
    setMessage,
    setEmployeeScopeOverrides,
  });
  const skillNameById = useMemo(() => new Map(skills.map((skill) => [skill.id, skill.name])), [skills]);
  const memorySkillScopeOptions = useMemo(() => {
    if (!selectedEmployee) return [];
    const ids = new Set<string>();
    if (selectedEmployee.primary_skill_id.trim()) ids.add(selectedEmployee.primary_skill_id.trim());
    for (const id of selectedEmployee.skill_ids) {
      const normalized = id.trim();
      if (normalized) ids.add(normalized);
    }
    return Array.from(ids.values());
  }, [selectedEmployee]);
  const selectedEmployeeAuthorizedSkills = useMemo(() => {
    if (!selectedEmployee) return [];
    const ids = new Set<string>();
    if (selectedEmployee.primary_skill_id.trim()) ids.add(selectedEmployee.primary_skill_id.trim());
    for (const id of selectedEmployee.skill_ids) {
      const normalized = id.trim();
      if (normalized) ids.add(normalized);
    }
    return Array.from(ids.values()).map((id) => ({ id, name: skillNameById.get(id) || id }));
  }, [selectedEmployee, skillNameById]);
  const employeeLabelById = useMemo(() => {
    const map = new Map<string, string>();
    for (const item of effectiveEmployees) {
      const key = employeeKey(item).toLowerCase();
      if (!key) continue;
      map.set(key, item.name || key);
    }
    return map;
  }, [effectiveEmployees]);

  useEffect(() => {
    if (initialTab) {
      setActiveTab(initialTab);
    }
  }, [initialTab]);

  const officialFeishuRuntimeRunning = officialFeishuRuntimeStatus?.running === true;

  function requestRemoveCurrent() {
    if (!selectedEmployeeId || saving) return;
    const target = employees.find((x) => x.id === selectedEmployeeId);
    setPendingDeleteEmployee({ id: selectedEmployeeId, name: target?.name ?? selectedEmployeeId });
  }

  async function confirmRemoveCurrent() {
    if (!pendingDeleteEmployee || saving) return;
    setSaving(true);
    setMessage("");
    try {
      await onDeleteEmployee(pendingDeleteEmployee.id);
      setMessage("员工已删除");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
      setPendingDeleteEmployee(null);
    }
  }

  function openEmployeesTab(filter: EmployeeHubEmployeeFilter = "all") {
    setEmployeeFilter(filter);
    setActiveTab("employees");
  }

  function openTeamsTab(filter: EmployeeHubTeamFilter = "all") {
    setTeamFilter(filter);
    setActiveTab("teams");
  }

  function openRunsTab(filter: EmployeeHubRunFilter = "all") {
    setRunFilter(filter);
    setActiveTab("runs");
  }

  function openTabFromNav(tab: EmployeeHubTab) {
    switch (tab) {
      case "employees":
        openEmployeesTab("all");
        break;
      case "teams":
        openTeamsTab("all");
        break;
      case "runs":
        openRunsTab("all");
        break;
      default:
        setActiveTab(tab);
        break;
    }
  }

  const deleteDialogSummary = pendingDeleteEmployee ? `确定删除员工「${pendingDeleteEmployee.name}」吗？` : "确定删除该员工吗？";
  const deleteDialogImpact = pendingDeleteEmployee ? `员工ID: ${pendingDeleteEmployee.id}` : undefined;
  const clearMemoryScopeLabel = memoryScopeSkillId === "__all__" ? "全部技能" : `技能 ${memoryScopeSkillId}`;
  const clearMemoryDialogSummary = selectedEmployee
    ? `确定清空员工「${selectedEmployee.name}」在${clearMemoryScopeLabel}下的长期记忆吗？`
    : `确定清空${clearMemoryScopeLabel}下的长期记忆吗？`;
  const clearMemoryDialogImpact = selectedEmployeeMemoryId ? `员工编号: ${selectedEmployeeMemoryId}` : undefined;
  const selectedEmployeeFeishuStatus = selectedEmployee ? resolveFeishuStatus(selectedEmployee, officialFeishuRuntimeStatus) : null;
  const selectedEmployeeFeishuRuntimeStatus = toEmployeeHubFeishuRuntimeStatus(officialFeishuRuntimeStatus);
  const defaultWorkspacePathExample = brandDefaultWorkspacePathExample();
  const tabs: Array<{ id: EmployeeHubTab; label: string }> = [
    { id: "overview", label: "总览" },
    { id: "employees", label: "员工" },
    { id: "teams", label: "团队" },
    { id: "runs", label: "运行" },
    { id: "settings", label: "设置" },
  ];
  const filteredEmployees = useMemo(
    () => effectiveEmployees.filter((employee) => matchesEmployeeHubEmployeeFilter(employee, employeeFilter)),
    [effectiveEmployees, employeeFilter],
  );
  const employeeFilterLabel =
    employeeFilter === "available"
      ? "可用员工"
      : employeeFilter === "missing-skills"
        ? "待补技能"
        : employeeFilter === "pending-connection"
          ? "待完善连接"
          : "全部员工";
  const teamFilterLabel = teamFilter === "incomplete-team" ? "角色不完整团队" : "全部团队";
  const runFilterLabel = runFilter === "running" ? "运行中" : "全部运行";

  return (
    <div className="h-full overflow-y-auto bg-[var(--sm-bg)]">
      <div className="max-w-6xl mx-auto px-8 pt-10 pb-12 space-y-4">
        <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
          <div>
            <h1 className="text-2xl font-semibold text-[var(--sm-text)]">智能体员工</h1>
            <p className="mt-2 text-sm text-[var(--sm-text-muted)]">用员工编号统一管理 OpenClaw 与多渠道路由。主员工默认进入且拥有全技能权限。</p>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "create" })}
              className="sm-btn sm-btn-primary h-9 rounded-lg px-4 text-sm"
            >
              新建员工
            </button>
            <button
              type="button"
              onClick={() => openTeamsTab("all")}
              className="sm-btn sm-btn-secondary h-9 rounded-lg px-4 text-sm"
            >
              新建团队
            </button>
            <button
              type="button"
              onClick={() => openRunsTab("all")}
              className="sm-btn sm-btn-secondary h-9 rounded-lg px-4 text-sm"
            >
              查看运行记录
            </button>
          </div>
        </div>
        <div className="flex flex-col gap-3 rounded-xl border border-[var(--sm-primary-soft)] bg-[var(--sm-primary-soft)] px-4 py-3 md:flex-row md:items-center md:justify-between">
          <div>
            <div className="text-sm font-medium text-[var(--sm-primary-strong)]">推荐：使用内置「智能体员工助手」技能</div>
            <div className="mt-1 text-xs text-[var(--sm-primary-strong)]/85">通过对话描述岗位需求，系统会自动给出技能匹配与配置建议，并在你确认后创建员工。</div>
          </div>
          <button type="button" data-testid="open-employee-creator-skill" onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "create" })} className="sm-btn sm-btn-primary h-9 rounded-lg px-4 text-sm">打开员工助手</button>
        </div>
        {highlightMessage && (
          <div data-testid="employee-creator-highlight" className="rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 flex items-center justify-between gap-3">
            <div className="text-xs text-emerald-800">{highlightMessage}</div>
            <button type="button" data-testid="employee-creator-highlight-dismiss" onClick={() => onDismissHighlight?.()} className="h-7 px-2.5 rounded border border-emerald-200 hover:bg-emerald-100 text-emerald-700 text-xs">知道了</button>
          </div>
        )}
        <EmployeeHubTabNav tabs={tabs} activeTab={activeTab} onTabChange={openTabFromNav} />
        {message && <div className="text-xs text-blue-700 bg-blue-50 border border-blue-100 rounded px-3 py-2">{message}</div>}
        {activeTab === "overview" && (
          <EmployeeOverviewSection
            employees={effectiveEmployees}
            groups={employeeGroups}
            runs={recentRuns}
            employeeLabelById={employeeLabelById}
            onSelectEmployee={onSelectEmployee}
            onOpenEmployeesTab={openEmployeesTab}
            onOpenTeamsTab={openTeamsTab}
            onOpenRunsTab={openRunsTab}
            onOpenSettingsTab={() => setActiveTab("settings")}
            onOpenGroupRunSession={onOpenGroupRunSession}
          />
        )}
        {activeTab === "teams" && (
          <div
            id="employee-hub-panel-teams"
            role="tabpanel"
            aria-labelledby="employee-hub-tab-teams"
            className="space-y-4"
          >
            <EmployeeTeamsSection
              employees={employees}
              employeeLabelById={employeeLabelById}
              draft={{
                groupName,
                groupCoordinatorId,
                groupMemberIds,
                groupEntryId,
                groupPlannerId,
                groupReviewerId,
                groupReviewMode,
                groupExecutionMode,
                groupVisibilityMode,
                groupSubmitting,
              }}
              groupsState={{
                teamFilter,
                teamFilterLabel,
                filteredGroups,
                groupDeletingId,
                groupRunGoalById,
                groupRunSubmittingId,
                groupRunReportById,
                groupRulesById,
                cloningGroupId,
              }}
              actions={{
                onGroupNameChange: setGroupName,
                onGroupCoordinatorChange: setGroupCoordinatorId,
                onGroupEntryChange: setGroupEntryId,
                onGroupPlannerChange: setGroupPlannerId,
                onGroupReviewerChange: setGroupReviewerId,
                onGroupReviewModeChange: setGroupReviewMode,
                onGroupExecutionModeChange: setGroupExecutionMode,
                onGroupVisibilityModeChange: setGroupVisibilityMode,
                onGroupMemberToggle: handleGroupMemberToggle,
                onCreateEmployeeGroup: createEmployeeGroup,
                onDeleteEmployeeGroup: deleteEmployeeGroup,
                onCloneEmployeeGroup: cloneEmployeeGroup,
                onStartEmployeeGroupRun: startEmployeeGroupRun,
                onGroupRunGoalChange: handleGroupRunGoalChange,
                onClearTeamFilter: () => setTeamFilter("all"),
              }}
            />
          </div>
        )}
        {activeTab === "settings" && (
          <EmployeeHubSettingsSection
            defaultWorkspacePathExample={defaultWorkspacePathExample}
            globalDefaultWorkDir={globalDefaultWorkDir}
            savingGlobalWorkDir={savingGlobalWorkDir}
            onGlobalDefaultWorkDirChange={setGlobalDefaultWorkDir}
            onSaveGlobalDefaultWorkDir={saveGlobalDefaultWorkDir}
          />
        )}

        {activeTab === "employees" && (
          <EmployeeHubEmployeesSection
            employeeFilter={employeeFilter}
            employeeFilterLabel={employeeFilterLabel}
            employees={employees}
            filteredEmployees={filteredEmployees}
            formatBytes={formatBytes}
            globalDefaultWorkDir={globalDefaultWorkDir}
            highlightEmployeeId={highlightEmployeeId}
            memoryActionLoading={memoryActionLoading}
            memoryLoading={memoryLoading}
            memoryScopeSkillId={memoryScopeSkillId}
            memorySkillScopeOptions={memorySkillScopeOptions}
            memoryStats={memoryStats}
            officialFeishuRuntimeStatus={officialFeishuRuntimeStatus}
            onClearEmployeeFilter={() => setEmployeeFilter("all")}
            onExportEmployeeMemory={exportEmployeeMemory}
            onMemoryScopeChange={setMemoryScopeSkillId}
            onOpenEmployeeCreatorSkill={onOpenEmployeeCreatorSkill}
            onOpenFeishuSettings={onOpenFeishuSettings}
            onOpenTeamsTab={() => openTeamsTab("all")}
            onRefreshEmployeeMemoryStats={() => refreshEmployeeMemoryStats()}
            onRequestClearEmployeeMemory={() => setPendingClearMemory(true)}
            onRequestRemoveCurrent={requestRemoveCurrent}
            onSelectEmployee={onSelectEmployee}
            onSetAsMainAndEnter={onSetAsMainAndEnter}
            onStartTaskWithEmployee={onStartTaskWithEmployee}
            profileLoading={profileLoading}
            profileView={profileView}
            resolveFeishuStatus={resolveFeishuStatus}
            routingBindings={routingBindings}
            saveFeishuAssociation={saveFeishuAssociation}
            saving={saving}
            savingFeishuAssociation={savingFeishuAssociation}
            selectedEmployee={selectedEmployee}
            selectedEmployeeAuthorizedSkills={selectedEmployeeAuthorizedSkills}
            selectedEmployeeFeishuRuntimeStatus={selectedEmployeeFeishuRuntimeStatus}
            selectedEmployeeFeishuStatus={selectedEmployeeFeishuStatus}
            selectedEmployeeId={selectedEmployeeId}
            selectedEmployeeMemoryId={selectedEmployeeMemoryId}
            setMessage={setMessage}
            skillNameById={skillNameById}
          />
        )}
        {activeTab === "runs" && (
          <div
            id="employee-hub-panel-runs"
            role="tabpanel"
            aria-labelledby="employee-hub-tab-runs"
            className="space-y-4"
          >
            <EmployeeRunsSection
              runFilter={runFilter}
              runFilterLabel={runFilterLabel}
              filteredRuns={filteredRuns}
              onClearRunFilter={() => setRunFilter("all")}
              onOpenGroupRunSession={onOpenGroupRunSession}
              onOpenTeamsTab={() => openTeamsTab("all")}
            />
          </div>
        )}
      </div>
      <RiskConfirmDialog open={pendingClearMemory} level="high" title="清空长期记忆" summary={clearMemoryDialogSummary} impact={clearMemoryDialogImpact} irreversible confirmLabel="确认清空" cancelLabel="取消" loading={memoryActionLoading === "clear"} onConfirm={confirmClearEmployeeMemory} onCancel={() => setPendingClearMemory(false)} />
      <RiskConfirmDialog open={Boolean(pendingDeleteEmployee)} level="high" title="删除员工" summary={deleteDialogSummary} impact={deleteDialogImpact} irreversible confirmLabel="确认删除" cancelLabel="取消" loading={saving} onConfirm={confirmRemoveCurrent} onCancel={() => setPendingDeleteEmployee(null)} />
    </div>
  );
}
