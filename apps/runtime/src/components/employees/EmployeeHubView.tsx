import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  AgentEmployee,
  EmployeeGroup,
  EmployeeGroupRule,
  EmployeeGroupRunResult,
  EmployeeGroupRunSummary,
  AgentProfileFilesView,
  EmployeeMemoryExport,
  EmployeeMemoryStats,
  ImRoutingBinding,
  OpenClawPluginFeishuRuntimeStatus,
  RuntimePreferences,
  SaveFeishuEmployeeAssociationInput,
  SkillManifest,
  UpsertAgentEmployeeInput,
} from "../../types";
import { RiskConfirmDialog } from "../RiskConfirmDialog";
import { EmployeeHubTabNav, type EmployeeHubTab as EmployeeHubTabNavItem } from "./EmployeeHubTabNav";
import {
  EmployeeGroupExecutionMode,
  EmployeeGroupReviewMode,
  EmployeeGroupVisibilityMode,
  EmployeeHubEmployeeFilter,
  EmployeeHubRunFilter,
  EmployeeHubTeamFilter,
  matchesEmployeeHubEmployeeFilter,
  matchesEmployeeHubRunFilter,
  matchesEmployeeHubTeamFilter,
} from "./employeeHubOverview";
import { EmployeeFeishuAssociationSection } from "./EmployeeFeishuAssociationSection";
import { EmployeeOverviewSection } from "./overview/EmployeeOverviewSection";
import { EmployeeRunsSection } from "./runs/EmployeeRunsSection";
import { EmployeeTeamsSection } from "./teams/EmployeeTeamsSection";
import { EmployeeMemoryToolsSection } from "./tools/EmployeeMemoryToolsSection";
import { EmployeeProfileFilesSection } from "./tools/EmployeeProfileFilesSection";

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
  const [officialFeishuRuntimeStatus, setOfficialFeishuRuntimeStatus] =
    useState<OpenClawPluginFeishuRuntimeStatus | null>(null);
  const [globalDefaultWorkDir, setGlobalDefaultWorkDir] = useState("");
  const [savingGlobalWorkDir, setSavingGlobalWorkDir] = useState(false);
  const [pendingDeleteEmployee, setPendingDeleteEmployee] = useState<{ id: string; name: string } | null>(null);
  const [memoryScopeSkillId, setMemoryScopeSkillId] = useState("__all__");
  const [memoryStats, setMemoryStats] = useState<EmployeeMemoryStats | null>(null);
  const [memoryLoading, setMemoryLoading] = useState(false);
  const [memoryActionLoading, setMemoryActionLoading] = useState<"export" | "clear" | null>(null);
  const [pendingClearMemory, setPendingClearMemory] = useState(false);
  const [profileView, setProfileView] = useState<AgentProfileFilesView | null>(null);
  const [profileLoading, setProfileLoading] = useState(false);
  const [routingBindings, setRoutingBindings] = useState<ImRoutingBinding[]>([]);
  const [employeeScopeOverrides, setEmployeeScopeOverrides] = useState<Record<string, string[]>>({});
  const [savingFeishuAssociation, setSavingFeishuAssociation] = useState(false);
  const [groupName, setGroupName] = useState("");
  const [groupCoordinatorId, setGroupCoordinatorId] = useState("");
  const [groupMemberIds, setGroupMemberIds] = useState<string[]>([]);
  const [groupEntryId, setGroupEntryId] = useState("");
  const [groupPlannerId, setGroupPlannerId] = useState("");
  const [groupReviewerId, setGroupReviewerId] = useState("");
  const [groupReviewMode, setGroupReviewMode] = useState<EmployeeGroupReviewMode>("none");
  const [groupExecutionMode, setGroupExecutionMode] = useState<EmployeeGroupExecutionMode>("sequential");
  const [groupVisibilityMode, setGroupVisibilityMode] = useState<EmployeeGroupVisibilityMode>("internal");
  const [employeeGroups, setEmployeeGroups] = useState<EmployeeGroup[]>([]);
  const [recentRuns, setRecentRuns] = useState<EmployeeGroupRunSummary[]>([]);
  const [employeeFilter, setEmployeeFilter] = useState<EmployeeHubEmployeeFilter>("all");
  const [teamFilter, setTeamFilter] = useState<EmployeeHubTeamFilter>("all");
  const [runFilter, setRunFilter] = useState<EmployeeHubRunFilter>("all");
  const [groupSubmitting, setGroupSubmitting] = useState(false);
  const [groupDeletingId, setGroupDeletingId] = useState<string | null>(null);
  const [groupRunGoalById, setGroupRunGoalById] = useState<Record<string, string>>({});
  const [groupRunSubmittingId, setGroupRunSubmittingId] = useState<string | null>(null);
  const [groupRunReportById, setGroupRunReportById] = useState<Record<string, string>>({});
  const [groupRulesById, setGroupRulesById] = useState<Record<string, EmployeeGroupRule[]>>({});
  const [cloningGroupId, setCloningGroupId] = useState<string | null>(null);

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
    (async () => {
      try {
        const prefs = await invoke<RuntimePreferences>("get_runtime_preferences");
        setGlobalDefaultWorkDir(prefs.default_work_dir || "");
      } catch {
        // ignore
      }
    })();
  }, []);

  useEffect(() => {
    if (initialTab) {
      setActiveTab(initialTab);
    }
  }, [initialTab]);

  useEffect(() => {
    const normalized = employees
      .map((item) => (item.employee_id || item.role_id || "").trim())
      .filter((item) => item.length > 0);
    setGroupMemberIds((prev) => prev.filter((id) => normalized.includes(id)));
    setGroupCoordinatorId((prev) => (normalized.includes(prev) ? prev : normalized[0] || ""));
  }, [employees]);

  async function loadEmployeeGroups() {
    try {
      const groups = await invoke<EmployeeGroup[]>("list_employee_groups");
      const normalizedGroups = Array.isArray(groups) ? groups : [];
      setEmployeeGroups(normalizedGroups);
      if (normalizedGroups.length === 0) {
        setGroupRulesById({});
        return;
      }
      const entries = await Promise.all(
        normalizedGroups.map(async (group) => {
          try {
            const rules = await invoke<EmployeeGroupRule[]>("list_employee_group_rules", {
              groupId: group.id,
            });
            return [group.id, Array.isArray(rules) ? rules : []] as const;
          } catch {
            return [group.id, []] as const;
          }
        }),
      );
      setGroupRulesById(Object.fromEntries(entries));
    } catch {
      setEmployeeGroups([]);
      setGroupRulesById({});
    }
  }

  async function loadRecentRuns() {
    try {
      const runs = await invoke<EmployeeGroupRunSummary[]>("list_employee_group_runs", { limit: 10 });
      setRecentRuns(Array.isArray(runs) ? runs : []);
    } catch {
      setRecentRuns([]);
    }
  }

  useEffect(() => {
    void loadEmployeeGroups();
    void loadRecentRuns();
  }, []);

  useEffect(() => {
    let disposed = false;
    const loadStatuses = async () => {
      try {
        const runtimeStatus = await invoke<OpenClawPluginFeishuRuntimeStatus | null>(
          "get_openclaw_plugin_feishu_runtime_status",
          {
            pluginId: "@larksuite/openclaw-lark",
            accountId: "default",
          },
        ).catch(() => null);
        if (!disposed) {
          setOfficialFeishuRuntimeStatus(runtimeStatus);
        }
      } catch {
        if (!disposed) {
          setOfficialFeishuRuntimeStatus(null);
        }
      }
    };
    void loadStatuses();
    const timer = setInterval(() => {
      void loadStatuses();
    }, 5000);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    const loadBindings = async () => {
      try {
        const bindings = await invoke<ImRoutingBinding[]>("list_im_routing_bindings");
        if (!disposed) {
          setRoutingBindings(Array.isArray(bindings) ? bindings : []);
        }
      } catch {
        if (!disposed) {
          setRoutingBindings([]);
        }
      }
    };
    void loadBindings();
    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    setMemoryScopeSkillId("__all__");
    setMemoryStats(null);
    setPendingClearMemory(false);
  }, [selectedEmployeeId]);

  useEffect(() => {
    if (!selectedEmployee) {
      setProfileView(null);
      setProfileLoading(false);
      return;
    }

    let disposed = false;
    setProfileLoading(true);
    invoke<AgentProfileFilesView>("get_agent_profile_files", { employeeDbId: selectedEmployee.id })
      .then((view) => {
        if (!disposed) setProfileView(view);
      })
      .catch(() => {
        if (!disposed) setProfileView(null);
      })
      .finally(() => {
        if (!disposed) setProfileLoading(false);
      });

    return () => {
      disposed = true;
    };
  }, [selectedEmployee]);

  const officialFeishuRuntimeRunning = officialFeishuRuntimeStatus?.running === true;

  function resolveFeishuStatus(employee: AgentEmployee): { dotClass: string; label: string; detail: string; error: string } {
    const enabled = !!employee.enabled;
    const agentId = (employee.openclaw_agent_id || employeeKey(employee)).trim().toLowerCase();
    const hasFeishuBinding = routingBindings.some(
      (binding) =>
        binding.enabled &&
        binding.channel === "feishu" &&
        binding.agent_id.trim().toLowerCase() === agentId,
    );
    const receivesFeishu = employee.enabled_scopes.includes("feishu") || hasFeishuBinding;
    if (!enabled) {
      return { dotClass: "bg-gray-300", label: "未启用飞书消息", detail: "该员工已停用，不接收飞书事件。", error: "" };
    }
    if (!receivesFeishu) {
      return { dotClass: "bg-gray-300", label: "未关联飞书接待", detail: "请在员工详情中启用飞书接待。", error: "" };
    }
    if (officialFeishuRuntimeRunning && !officialFeishuRuntimeStatus?.last_error?.trim()) {
      return {
        dotClass: "bg-emerald-500",
        label: "飞书接入正常",
        detail: "官方插件宿主已运行，飞书接待规则已生效。",
        error: "",
      };
    }
    const error =
      officialFeishuRuntimeStatus?.last_error?.trim() ||
      (!officialFeishuRuntimeRunning ? "官方插件宿主未运行" : "飞书消息桥接未运行");
    if (!officialFeishuRuntimeRunning) {
      return {
        dotClass: "bg-amber-500",
        label: "待启动飞书接入",
        detail: "请前往设置中心中的飞书连接页面检查官方插件状态。",
        error,
      };
    }
    return {
      dotClass: "bg-red-500",
      label: "飞书接入异常",
      detail: "请检查官方插件运行状态或员工接待规则。",
      error,
    };
  }

  async function refreshEmployeeMemoryStats(scopeSkillId?: string) {
    if (!selectedEmployeeMemoryId) {
      setMemoryStats(null);
      return;
    }
    const normalizedSkillId = (scopeSkillId ?? memoryScopeSkillId) === "__all__" ? null : (scopeSkillId ?? memoryScopeSkillId);
    setMemoryLoading(true);
    try {
      const stats = await invoke<EmployeeMemoryStats>("get_employee_memory_stats", {
        employeeId: selectedEmployeeMemoryId,
        skillId: normalizedSkillId,
      });
      setMemoryStats(stats);
    } catch (e) {
      setMessage(`加载长期记忆统计失败: ${String(e)}`);
    } finally {
      setMemoryLoading(false);
    }
  }

  useEffect(() => {
    if (!selectedEmployeeMemoryId) {
      setMemoryStats(null);
      return;
    }
    void refreshEmployeeMemoryStats();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedEmployeeMemoryId, memoryScopeSkillId]);

  async function exportEmployeeMemory() {
    if (!selectedEmployeeMemoryId || memoryActionLoading) return;
    setMemoryActionLoading("export");
    try {
      const skillId = memoryScopeSkillId === "__all__" ? null : memoryScopeSkillId;
      const payload = await invoke<EmployeeMemoryExport>("export_employee_memory", {
        employeeId: selectedEmployeeMemoryId,
        skillId,
      });
      const filePath = await saveDialog({
        defaultPath: `employee-memory-${selectedEmployeeMemoryId}-${skillId || "all"}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!filePath) return;
      await invoke("write_export_file", {
        path: filePath,
        content: JSON.stringify(payload, null, 2),
      });
      setMessage("长期记忆已导出");
    } catch (e) {
      setMessage(`导出长期记忆失败: ${String(e)}`);
    } finally {
      setMemoryActionLoading(null);
    }
  }

  async function confirmClearEmployeeMemory() {
    if (!selectedEmployeeMemoryId || memoryActionLoading) return;
    setMemoryActionLoading("clear");
    try {
      const skillId = memoryScopeSkillId === "__all__" ? null : memoryScopeSkillId;
      const stats = await invoke<EmployeeMemoryStats>("clear_employee_memory", {
        employeeId: selectedEmployeeMemoryId,
        skillId,
      });
      setMemoryStats(stats);
      setMessage("长期记忆已清空");
    } catch (e) {
      setMessage(`清空长期记忆失败: ${String(e)}`);
    } finally {
      setMemoryActionLoading(null);
      setPendingClearMemory(false);
    }
  }

  async function saveFeishuAssociation(input: {
    enabled: boolean;
    mode: "default" | "scoped";
    peerKind: "group" | "channel" | "direct";
    peerId: string;
    priority: number;
  }) {
    if (!selectedEmployee) return;
    if (!selectedEmployee.id.trim()) {
      setMessage("员工编号缺失，无法保存飞书接待");
      return;
    }
    setSavingFeishuAssociation(true);
    setMessage("");
    try {
      const scopes = new Set(selectedEmployee.enabled_scopes?.length ? selectedEmployee.enabled_scopes : ["app"]);
      if (input.enabled) {
        scopes.add("feishu");
      } else {
        scopes.delete("feishu");
      }
      if (scopes.size === 0) {
        scopes.add("app");
      }
      const nextScopes = Array.from(scopes.values());
      const payload: SaveFeishuEmployeeAssociationInput = {
        employee_db_id: selectedEmployee.id,
        enabled: input.enabled,
        mode: input.mode,
        peer_kind: input.mode === "default" ? "group" : input.peerKind,
        peer_id: input.mode === "default" ? "" : input.peerId.trim(),
        priority: input.priority,
      };
      await invoke("save_feishu_employee_association", { input: payload });

      const latestBindings = await invoke<ImRoutingBinding[]>("list_im_routing_bindings");
      setRoutingBindings(Array.isArray(latestBindings) ? latestBindings : []);
      setEmployeeScopeOverrides((current) => ({
        ...current,
        [selectedEmployee.id]: nextScopes,
      }));
      let refreshWarning = "";
      if (onRefreshEmployees) {
        try {
          await onRefreshEmployees();
          setEmployeeScopeOverrides((current) => {
            if (!(selectedEmployee.id in current)) return current;
            const next = { ...current };
            delete next[selectedEmployee.id];
            return next;
          });
        } catch (refreshError) {
          refreshWarning = `，员工列表刷新失败: ${String(refreshError)}`;
        }
      }
      setMessage(
        input.enabled
          ? `飞书接待已保存${refreshWarning}`
          : `已关闭该员工的飞书接待${refreshWarning}`,
      );
    } catch (e) {
      setMessage(`保存飞书接待失败: ${String(e)}`);
    } finally {
      setSavingFeishuAssociation(false);
    }
  }

  async function createEmployeeGroup() {
    const name = groupName.trim();
    const coordinator = groupCoordinatorId.trim();
    const members = Array.from(new Set(groupMemberIds.map((item) => item.trim()).filter((item) => item.length > 0)));
    const entryEmployeeId = (groupEntryId.trim() || coordinator).trim();
    const plannerEmployeeId = (groupPlannerId.trim() || entryEmployeeId || coordinator).trim();
    const reviewerEmployeeId = groupReviewerId.trim();
    const reviewMode = groupReviewMode.trim() || "none";
    const executionMode = groupExecutionMode.trim() || "sequential";
    const visibilityMode = groupVisibilityMode.trim() || "internal";

    if (!name) {
      setMessage("请填写群组名称");
      return;
    }
    if (!coordinator) {
      setMessage("请先选择协调员");
      return;
    }
    if (members.length === 0) {
      setMessage("请至少选择 1 个成员");
      return;
    }
    if (members.length > 10) {
      setMessage("群组成员最多 10 人");
      return;
    }
    if (!members.includes(coordinator)) {
      setMessage("协调员必须包含在群组成员中");
      return;
    }
    if (entryEmployeeId && !members.includes(entryEmployeeId)) {
      setMessage("入口员工必须包含在团队成员中");
      return;
    }
    if (plannerEmployeeId && !members.includes(plannerEmployeeId)) {
      setMessage("规划员工必须包含在团队成员中");
      return;
    }
    if (reviewMode !== "none" && !reviewerEmployeeId) {
      setMessage("开启审核后必须选择审核员工");
      return;
    }
    if (reviewerEmployeeId && !members.includes(reviewerEmployeeId)) {
      setMessage("审核员工必须包含在团队成员中");
      return;
    }

    setGroupSubmitting(true);
    setMessage("");
    try {
      await invoke<string>("create_employee_team", {
        input: {
          name,
          coordinator_employee_id: coordinator,
          member_employee_ids: members,
          entry_employee_id: entryEmployeeId,
          planner_employee_id: plannerEmployeeId,
          reviewer_employee_id: reviewerEmployeeId,
          review_mode: reviewMode,
          execution_mode: executionMode,
          visibility_mode: visibilityMode,
        },
      });
      setGroupName("");
      setGroupCoordinatorId("");
      setGroupMemberIds([]);
      setGroupEntryId("");
      setGroupPlannerId("");
      setGroupReviewerId("");
      setGroupReviewMode("none");
      setGroupExecutionMode("sequential");
      setGroupVisibilityMode("internal");
      await loadEmployeeGroups();
      await loadRecentRuns();
      await onEmployeeGroupsChanged?.();
      setMessage("协作团队已创建");
    } catch (e) {
      setMessage(`创建群组失败: ${String(e)}`);
    } finally {
      setGroupSubmitting(false);
    }
  }

  async function deleteEmployeeGroup(groupId: string) {
    if (!groupId) return;
    setGroupDeletingId(groupId);
    try {
      await invoke("delete_employee_group", { groupId });
      setGroupRunGoalById((prev) => {
        const next = { ...prev };
        delete next[groupId];
        return next;
      });
      setGroupRunReportById((prev) => {
        const next = { ...prev };
        delete next[groupId];
        return next;
      });
      setGroupRulesById((prev) => {
        const next = { ...prev };
        delete next[groupId];
        return next;
      });
      await loadEmployeeGroups();
      await loadRecentRuns();
      await onEmployeeGroupsChanged?.();
      setMessage("协作群组已删除");
    } catch (e) {
      setMessage(`删除群组失败: ${String(e)}`);
    } finally {
      setGroupDeletingId(null);
    }
  }

  async function startEmployeeGroupRun(groupId: string) {
    if (!groupId || groupRunSubmittingId) return;
    const userGoal = (groupRunGoalById[groupId] || "").trim();
    if (!userGoal) {
      setMessage("请先填写协作指令");
      return;
    }
    setGroupRunSubmittingId(groupId);
    setMessage("");
    try {
      const result = await invoke<EmployeeGroupRunResult>("start_employee_group_run", {
        input: {
          group_id: groupId,
          user_goal: userGoal,
          execution_window: 3,
          max_retry_per_step: 1,
          timeout_employee_ids: [],
        },
      });
      setGroupRunReportById((prev) => ({
        ...prev,
        [groupId]: result.final_report || "",
      }));
      await loadRecentRuns();
      if (result.session_id && result.session_skill_id) {
        await onOpenGroupRunSession?.(result.session_id, result.session_skill_id);
      }
      if ((result.state || "").trim().toLowerCase() === "waiting_review") {
        setMessage("协作任务已启动，等待审核");
      } else if ((result.state || "").trim().toLowerCase() === "done") {
        setMessage(`协作任务已完成（第 ${result.current_round || 1} 轮）`);
      } else {
        setMessage(`协作任务已启动，当前状态：${result.state || "执行"}`);
      }
    } catch (e) {
      setMessage(`发起协作失败: ${String(e)}`);
    } finally {
      setGroupRunSubmittingId(null);
    }
  }

  async function cloneEmployeeGroup(group: EmployeeGroup) {
    if (!group.id || cloningGroupId) return;
    const cloneName = `${group.name}（副本）`;
    setCloningGroupId(group.id);
    setMessage("");
    try {
      await invoke<string>("clone_employee_group_template", {
        input: {
          source_group_id: group.id,
          name: cloneName,
        },
      });
      await loadEmployeeGroups();
      await loadRecentRuns();
      await onEmployeeGroupsChanged?.();
      setMessage(`已复制团队：${cloneName}`);
    } catch (e) {
      setMessage(`复制团队失败: ${String(e)}`);
    } finally {
      setCloningGroupId(null);
    }
  }

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

  async function saveGlobalDefaultWorkDir() {
    if (!globalDefaultWorkDir.trim()) {
      setMessage("默认工作目录不能为空");
      return;
    }
    setSavingGlobalWorkDir(true);
    setMessage("");
    try {
      await invoke("set_runtime_preferences", { input: { default_work_dir: globalDefaultWorkDir.trim() } });
      const resolved = await invoke<string>("resolve_default_work_dir");
      setGlobalDefaultWorkDir(resolved);
      setMessage("全局默认工作目录已保存");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSavingGlobalWorkDir(false);
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

  function handleGroupMemberToggle(employeeCode: string, checked: boolean) {
    if (checked) {
      setGroupMemberIds((prev) => {
        if (prev.includes(employeeCode)) return prev;
        if (prev.length >= 10) {
          setMessage("群组成员最多 10 人");
          return prev;
        }
        return [...prev, employeeCode];
      });
      return;
    }
    setGroupMemberIds((prev) => prev.filter((id) => id !== employeeCode));
  }

  function handleGroupRunGoalChange(groupId: string, value: string) {
    setGroupRunGoalById((prev) => ({ ...prev, [groupId]: value }));
  }

  const deleteDialogSummary = pendingDeleteEmployee ? `确定删除员工「${pendingDeleteEmployee.name}」吗？` : "确定删除该员工吗？";
  const deleteDialogImpact = pendingDeleteEmployee ? `员工ID: ${pendingDeleteEmployee.id}` : undefined;
  const clearMemoryScopeLabel = memoryScopeSkillId === "__all__" ? "全部技能" : `技能 ${memoryScopeSkillId}`;
  const clearMemoryDialogSummary = selectedEmployee
    ? `确定清空员工「${selectedEmployee.name}」在${clearMemoryScopeLabel}下的长期记忆吗？`
    : `确定清空${clearMemoryScopeLabel}下的长期记忆吗？`;
  const clearMemoryDialogImpact = selectedEmployeeMemoryId ? `员工编号: ${selectedEmployeeMemoryId}` : undefined;
  const selectedEmployeeFeishuStatus = selectedEmployee ? resolveFeishuStatus(selectedEmployee) : null;
  const selectedEmployeeFeishuRuntimeStatus = officialFeishuRuntimeStatus
    ? {
        queued_events: 0,
        reconnect_attempts: 0,
        last_event_at: officialFeishuRuntimeStatus.last_event_at ?? null,
        last_error: officialFeishuRuntimeStatus.last_error ?? null,
      }
    : null;
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
  const filteredGroups = useMemo(
    () => employeeGroups.filter((group) => matchesEmployeeHubTeamFilter(group, teamFilter)),
    [employeeGroups, teamFilter],
  );
  const filteredRuns = useMemo(
    () => recentRuns.filter((run) => matchesEmployeeHubRunFilter(run, runFilter)),
    [recentRuns, runFilter],
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
          <div
            id="employee-hub-panel-settings"
            role="tabpanel"
            aria-labelledby="employee-hub-tab-settings"
            className="space-y-4"
          >
        <div className="bg-white border border-gray-200 rounded-xl p-4 space-y-2">
          <div className="text-xs text-gray-500">全局默认工作目录（新建会话默认使用）</div>
          <input className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm" placeholder="例如 D:\\workspace\\workclaw" value={globalDefaultWorkDir} onChange={(e) => setGlobalDefaultWorkDir(e.target.value)} />
          <div className="text-[11px] text-gray-500">默认：C:\Users\&lt;用户名&gt;\WorkClaw\workspace。支持 C/D/E 盘路径，目录不存在会自动创建。</div>
          <button disabled={savingGlobalWorkDir} onClick={saveGlobalDefaultWorkDir} className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-xs">保存默认目录</button>
        </div>
          </div>
        )}

        {activeTab === "employees" && (
          <div
            id="employee-hub-panel-employees"
            role="tabpanel"
            aria-labelledby="employee-hub-tab-employees"
            className="space-y-4"
          >
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="bg-white border border-gray-200 rounded-xl p-3 max-h-[640px] overflow-y-auto">
            <div className="text-xs text-gray-500 mb-2">员工列表</div>
            <div className="mb-2"><button type="button" onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "create" })} className="h-8 w-full rounded bg-blue-600 hover:bg-blue-700 text-white text-xs">新建员工</button></div>
            {employeeFilter !== "all" && (
              <div className="mb-2 flex items-center justify-between rounded border border-blue-100 bg-blue-50 px-2 py-1.5 text-[11px] text-blue-700">
                <span>当前筛选：{employeeFilterLabel}</span>
                <button type="button" onClick={() => setEmployeeFilter("all")} className="text-blue-700 hover:text-blue-800">清除筛选</button>
              </div>
            )}
            <div className="space-y-2">
              {filteredEmployees.length === 0 ? (
                <div className="rounded border border-dashed border-gray-300 px-3 py-4 text-xs text-gray-500">当前筛选下暂无员工。</div>
              ) : filteredEmployees.map((employee) => {
                const status = resolveFeishuStatus(employee);
                const isSelected = selectedEmployeeId === employee.id;
                const isHighlighted = highlightEmployeeId === employee.id;
                return (
                  <button key={employee.id} data-testid={`employee-item-${employee.id}`} onClick={() => { onSelectEmployee(employee.id); setMessage(""); }} className={"w-full text-left border rounded p-2 text-xs " + (isHighlighted ? "border-emerald-300 bg-emerald-50 ring-1 ring-emerald-200" : isSelected ? "border-blue-300 bg-blue-50" : "border-gray-200 bg-white")}>
                    <div className="flex items-center gap-2">
                      <span data-testid={`employee-connection-dot-${employee.id}`} className={`inline-block h-2.5 w-2.5 rounded-full ${status.dotClass}`} title={status.label} />
                      <div className="font-medium text-gray-800 truncate">{employee.name} {employee.is_default ? "· 主员工" : ""}</div>
                      {isHighlighted && <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-100 text-emerald-700 border border-emerald-200">新建</span>}
                    </div>
                    <div className="text-gray-500 truncate">{employee.employee_id || employee.role_id}</div>
                  </button>
                );
              })}
            </div>
          </div>

          <div className="md:col-span-2 bg-white border border-gray-200 rounded-xl p-4 space-y-3">
            <div className="text-xs text-gray-500">员工详情</div>
            {selectedEmployee ? (
              <>
                <div className="rounded-lg border border-gray-200 p-3 space-y-2">
                  <div className="flex items-center justify-between gap-2">
                    <div><div className="text-sm font-semibold text-gray-900">{selectedEmployee.name}</div><div className="text-xs text-gray-500">{selectedEmployeeMemoryId || "未设置员工编号"}</div></div>
                    <button
                      type="button"
                      onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "update", employeeId: selectedEmployee.id })}
                      className="h-8 px-3 rounded border border-blue-200 hover:bg-blue-50 text-blue-700 text-xs"
                    >
                      调整员工
                    </button>
                  </div>
                  <div className="text-[11px] text-gray-500">角色职责</div>
                  <div className="text-xs text-gray-700 whitespace-pre-wrap">{selectedEmployee.persona?.trim() || "暂无职责描述，可通过智能体员工助手补充。"}</div>
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                    <div className="rounded border border-gray-100 p-2"><div className="text-[11px] text-gray-500">主技能</div><div className="text-xs text-gray-700">{selectedEmployee.primary_skill_id ? (skillNameById.get(selectedEmployee.primary_skill_id) || selectedEmployee.primary_skill_id) : "通用助手（系统默认）"}</div></div>
                    <div className="rounded border border-gray-100 p-2"><div className="text-[11px] text-gray-500">默认工作目录</div><div className="text-xs text-gray-700 break-all">{selectedEmployee.default_work_dir?.trim() || globalDefaultWorkDir.trim() || "跟随系统默认目录"}</div></div>
                  </div>
                  <div className="text-[11px] text-gray-500">技能合集</div>
                  <div className="flex flex-wrap gap-1">{selectedEmployeeAuthorizedSkills.length === 0 ? <span className="text-[11px] px-2 py-0.5 rounded border border-gray-200 text-gray-500">未配置，默认按主技能执行</span> : selectedEmployeeAuthorizedSkills.map((item) => <span key={item.id} className="text-[11px] px-2 py-0.5 rounded border border-blue-100 bg-blue-50 text-blue-700">{item.name}</span>)}</div>
                </div>

                {selectedEmployeeFeishuStatus && selectedEmployee && (
                  <EmployeeFeishuAssociationSection
                    employee={selectedEmployee}
                    employees={employees}
                    bindings={routingBindings}
                    saving={savingFeishuAssociation}
                    runtimeStatus={selectedEmployeeFeishuRuntimeStatus}
                    onSave={saveFeishuAssociation}
                    onOpenFeishuSettings={onOpenFeishuSettings}
                  />
                )}

                <EmployeeProfileFilesSection
                  profileLoading={profileLoading}
                  profileView={profileView}
                  onOpenEmployeeCreatorSkill={() => onOpenEmployeeCreatorSkill?.({ mode: "update", employeeId: selectedEmployee.id })}
                />

                <div className="flex items-center gap-2 pt-1">
                  <button disabled={saving || !selectedEmployeeId} onClick={requestRemoveCurrent} className="h-8 px-3 rounded bg-red-50 hover:bg-red-100 disabled:bg-gray-100 text-red-600 text-xs">删除员工</button>
                  <button disabled={!selectedEmployeeId} onClick={() => selectedEmployeeId && onSetAsMainAndEnter(selectedEmployeeId)} className="h-8 px-3 rounded bg-emerald-50 hover:bg-emerald-100 disabled:bg-gray-100 text-emerald-700 text-xs">设为主员工并进入首页</button>
                  <button disabled={!selectedEmployeeId || saving} onClick={() => selectedEmployeeId && onStartTaskWithEmployee(selectedEmployeeId)} className="h-8 px-3 rounded bg-indigo-50 hover:bg-indigo-100 disabled:bg-gray-100 text-indigo-700 text-xs">与该员工开始对话</button>
                  <button type="button" onClick={() => openTeamsTab("all")} className="h-8 px-3 rounded bg-violet-50 hover:bg-violet-100 text-violet-700 text-xs">以团队模式发起任务</button>
                </div>
              </>
            ) : (
              <div className="rounded-lg border border-dashed border-gray-300 p-4 space-y-2">
                <div className="text-sm font-medium text-gray-800">请选择一个员工或直接创建</div>
                <div className="text-xs text-gray-600">已移除手动创建流程，请通过「智能体员工助手」对话式完成创建与配置。</div>
                <button type="button" onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "create" })} className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 text-white text-xs">创建员工</button>
              </div>
            )}

            <EmployeeMemoryToolsSection
              memoryLoading={memoryLoading}
              memoryActionLoading={memoryActionLoading}
              memoryScopeSkillId={memoryScopeSkillId}
              memorySkillScopeOptions={memorySkillScopeOptions}
              selectedEmployeeMemoryId={selectedEmployeeMemoryId}
              memoryStats={memoryStats}
              formatBytes={formatBytes}
              onMemoryScopeChange={setMemoryScopeSkillId}
              onRefreshEmployeeMemoryStats={() => refreshEmployeeMemoryStats()}
              onExportEmployeeMemory={exportEmployeeMemory}
              onRequestClearEmployeeMemory={() => setPendingClearMemory(true)}
            />
          </div>
        </div>
          </div>
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
