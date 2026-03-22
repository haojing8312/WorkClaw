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
import {
  EmployeeHubEmployeeFilter,
  EmployeeHubRunFilter,
  EmployeeHubTeamFilter,
  buildEmployeeHubMetrics,
  buildEmployeeHubPendingItems,
  matchesEmployeeHubEmployeeFilter,
  matchesEmployeeHubRunFilter,
  matchesEmployeeHubTeamFilter,
} from "./employeeHubOverview";
import { EmployeeFeishuAssociationSection } from "./EmployeeFeishuAssociationSection";

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

type GroupTemplateRole = {
  role_type?: string;
  employee_key?: string;
  employee_id?: string;
};

type GroupTemplateConfig = {
  roles?: GroupTemplateRole[];
};

export type EmployeeHubTab = "overview" | "employees" | "teams" | "runs" | "settings";

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
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
  const [groupReviewMode, setGroupReviewMode] = useState("none");
  const [groupExecutionMode, setGroupExecutionMode] = useState("sequential");
  const [groupVisibilityMode, setGroupVisibilityMode] = useState("internal");
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
  const overviewMetrics = useMemo(
    () => buildEmployeeHubMetrics({ employees: effectiveEmployees, groups: employeeGroups, runs: recentRuns }),
    [effectiveEmployees, employeeGroups, recentRuns],
  );
  const pendingItems = useMemo(
    () => buildEmployeeHubPendingItems({ employees: effectiveEmployees, groups: employeeGroups }),
    [effectiveEmployees, employeeGroups],
  );
  const recentEmployees = effectiveEmployees.slice(0, 5);
  const recentGroups = employeeGroups.slice(0, 5);
  const recentRunsForOverview = recentRuns.slice(0, 5);
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
  const resolveEmployeeDisplayName = (employeeId: string) => {
    const normalized = employeeId.trim().toLowerCase();
    if (!normalized) return "未设置";
    return employeeLabelById.get(normalized) || employeeId.trim();
  };
  const resolveRunStatusLabel = (status: string) => {
    switch (status.trim().toLowerCase()) {
      case "running":
        return "运行中";
      case "completed":
        return "已完成";
      case "failed":
        return "失败";
      case "waiting_review":
        return "等待审核";
      case "cancelled":
        return "已取消";
      default:
        return status.trim() || "未知";
    }
  };
  const formatRunTimestamp = (value: string) => {
    if (!value.trim()) return "刚刚";
    return value.replace("T", " ").replace("Z", "").slice(0, 16);
  };
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
        <div className="rounded-xl border border-[var(--sm-border)] bg-[var(--sm-surface)] p-2 shadow-[var(--sm-shadow-sm)]">
          <div role="tablist" aria-label="智能体员工导航" className="flex flex-wrap gap-2">
            {tabs.map((tab) => {
              const selected = activeTab === tab.id;
              return (
                <button
                  key={tab.id}
                  id={`employee-hub-tab-${tab.id}`}
                  type="button"
                  role="tab"
                  aria-selected={selected}
                  aria-controls={`employee-hub-panel-${tab.id}`}
                  tabIndex={selected ? 0 : -1}
                  onClick={() => openTabFromNav(tab.id)}
                  className={
                    "h-9 px-4 rounded-lg text-sm transition " +
                    (selected
                      ? "border-[var(--sm-primary-soft)] bg-[var(--sm-primary-soft)] text-[var(--sm-primary-strong)] shadow-[var(--sm-shadow-sm)]"
                      : "border border-[var(--sm-border)] bg-[var(--sm-surface)] text-[var(--sm-text-muted)] hover:bg-[var(--sm-surface-muted)]")
                  }
                >
                  {tab.label}
                </button>
              );
            })}
          </div>
        </div>
        {message && <div className="text-xs text-blue-700 bg-blue-50 border border-blue-100 rounded px-3 py-2">{message}</div>}
        {activeTab === "overview" && (
          <div
            id="employee-hub-panel-overview"
            role="tabpanel"
            aria-labelledby="employee-hub-tab-overview"
            className="space-y-4"
          >
            <div className="grid grid-cols-1 gap-3 md:grid-cols-5">
              <button type="button" aria-label="查看全部员工" onClick={() => openEmployeesTab("all")} className="rounded-xl border border-gray-200 bg-white p-4 text-left hover:bg-gray-50">
                <div className="text-xs text-gray-500">员工总数</div>
                <div data-testid="employee-overview-metric-employees" className="mt-2 text-2xl font-semibold text-gray-900">{overviewMetrics.employees}</div>
              </button>
              <button type="button" aria-label="查看全部团队" onClick={() => openTeamsTab("all")} className="rounded-xl border border-gray-200 bg-white p-4 text-left hover:bg-gray-50">
                <div className="text-xs text-gray-500">团队总数</div>
                <div data-testid="employee-overview-metric-teams" className="mt-2 text-2xl font-semibold text-gray-900">{overviewMetrics.teams}</div>
              </button>
              <button type="button" aria-label="查看可用员工" onClick={() => openEmployeesTab("available")} className="rounded-xl border border-gray-200 bg-white p-4 text-left hover:bg-gray-50">
                <div className="text-xs text-gray-500">可用员工</div>
                <div data-testid="employee-overview-metric-available-employees" className="mt-2 text-2xl font-semibold text-gray-900">{overviewMetrics.availableEmployees}</div>
              </button>
              <button type="button" aria-label="查看运行中团队" onClick={() => openRunsTab("running")} className="rounded-xl border border-gray-200 bg-white p-4 text-left hover:bg-gray-50">
                <div className="text-xs text-gray-500">运行中团队</div>
                <div data-testid="employee-overview-metric-running-teams" className="mt-2 text-2xl font-semibold text-gray-900">{overviewMetrics.runningTeams}</div>
              </button>
              <button
                type="button"
                aria-label="查看待处理事项"
                onClick={() => {
                  const first = pendingItems[0];
                  if (!first) return;
                  if (first.id === "incomplete-team") {
                    openTeamsTab("incomplete-team");
                    return;
                  }
                  openEmployeesTab(first.id);
                }}
                className="rounded-xl border border-gray-200 bg-white p-4 text-left hover:bg-gray-50"
              >
                <div className="text-xs text-gray-500">待处理事项</div>
                <div data-testid="employee-overview-metric-pending-items" className="mt-2 text-2xl font-semibold text-gray-900">{overviewMetrics.pendingItems}</div>
              </button>
            </div>
            <div className="rounded-xl border border-gray-200 bg-white p-4 space-y-3">
              <div className="flex items-center justify-between gap-2">
                <div>
                  <div className="text-sm font-medium text-gray-900">待处理事项</div>
                  <div className="text-xs text-gray-500 mt-1">优先处理影响员工可用性和团队协作的问题。</div>
                </div>
                <button type="button" onClick={() => setActiveTab("settings")} className="text-xs text-blue-600 hover:text-blue-700">
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
                    <div key={item.id} className="flex items-center justify-between gap-3 rounded-lg border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-800">
                      <span>{item.label}</span>
                      <button
                        type="button"
                        onClick={() => {
                          if (item.id === "incomplete-team") {
                            openTeamsTab("incomplete-team");
                            return;
                          }
                          openEmployeesTab(item.id);
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
                  <button type="button" onClick={() => openEmployeesTab("all")} className="text-xs text-blue-600 hover:text-blue-700">查看全部员工</button>
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
                          openEmployeesTab("all");
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
                  <button type="button" onClick={() => openTeamsTab("all")} className="text-xs text-blue-600 hover:text-blue-700">查看全部团队</button>
                </div>
                <div className="space-y-2">
                  {recentGroups.length === 0 ? (
                    <div className="rounded-lg border border-dashed border-gray-300 px-3 py-4 text-xs text-gray-500">还没有团队，创建团队后可分工协作。</div>
                  ) : (
                    recentGroups.map((group) => (
                      <button
                        key={group.id}
                        type="button"
                        onClick={() => openTeamsTab("all")}
                        className="flex w-full items-center justify-between rounded-lg border border-gray-200 px-3 py-2 text-left hover:bg-gray-50"
                      >
                        <div>
                          <div className="text-sm text-gray-900">{group.name}</div>
                          <div className="text-xs text-gray-500">{group.member_count || group.member_employee_ids.length} 人 · {resolveEmployeeDisplayName(group.coordinator_employee_id)}</div>
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
                <button type="button" onClick={() => openRunsTab("all")} className="text-xs text-blue-600 hover:text-blue-700">
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
                          {run.group_name || "未命名团队"} · {resolveRunStatusLabel(run.status)} · {formatRunTimestamp(run.started_at)}
                        </div>
                      </div>
                      <button
                        type="button"
                        onClick={() => {
                          if (run.session_id && run.session_skill_id) {
                            void onOpenGroupRunSession?.(run.session_id, run.session_skill_id);
                            return;
                          }
                          openTeamsTab("all");
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
        )}
        {activeTab === "teams" && (
          <div
            id="employee-hub-panel-teams"
            role="tabpanel"
            aria-labelledby="employee-hub-tab-teams"
            className="space-y-4"
          >
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
              value={groupName}
              onChange={(e) => setGroupName(e.target.value)}
            />
            <select
              data-testid="employee-group-coordinator"
              className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
              value={groupCoordinatorId}
              onChange={(e) => setGroupCoordinatorId(e.target.value)}
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
              disabled={groupSubmitting}
              onClick={createEmployeeGroup}
              className="h-9 rounded bg-indigo-600 hover:bg-indigo-700 disabled:bg-indigo-300 text-white text-sm"
            >
              {groupSubmitting ? "创建中..." : "创建协作群"}
            </button>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
            <select
              data-testid="employee-group-entry"
              className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
              value={groupEntryId}
              onChange={(e) => setGroupEntryId(e.target.value)}
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
              value={groupPlannerId}
              onChange={(e) => setGroupPlannerId(e.target.value)}
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
              value={groupReviewerId}
              onChange={(e) => setGroupReviewerId(e.target.value)}
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
              value={groupReviewMode}
              onChange={(e) => setGroupReviewMode(e.target.value)}
            >
              <option value="none">无需审核</option>
              <option value="soft">建议审核</option>
              <option value="hard">强制审核</option>
            </select>
            <select
              data-testid="employee-group-execution-mode"
              className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
              value={groupExecutionMode}
              onChange={(e) => setGroupExecutionMode(e.target.value)}
            >
              <option value="sequential">顺序执行</option>
              <option value="parallel">并行执行</option>
            </select>
            <select
              data-testid="employee-group-visibility-mode"
              className="border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
              value={groupVisibilityMode}
              onChange={(e) => setGroupVisibilityMode(e.target.value)}
            >
              <option value="internal">内部可见</option>
              <option value="shared">协作共享</option>
            </select>
          </div>
          <div className="rounded border border-gray-200 p-2">
            <div className="text-[11px] text-gray-500 mb-1">选择成员（{groupMemberIds.length}/10）</div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-1.5">
              {employees.map((item) => {
                const employeeCode = (item.employee_id || item.role_id || "").trim();
                if (!employeeCode) return null;
                const checked = groupMemberIds.includes(employeeCode);
                return (
                  <label key={item.id} className="flex items-center gap-2 text-xs text-gray-700">
                    <input
                      data-testid={`employee-group-member-${item.id}`}
                      type="checkbox"
                      checked={checked}
                      onChange={(e) => {
                        if (e.target.checked) {
                          setGroupMemberIds((prev) => {
                            if (prev.includes(employeeCode)) return prev;
                            if (prev.length >= 10) {
                              setMessage("群组成员最多 10 人");
                              return prev;
                            }
                            return [...prev, employeeCode];
                          });
                        } else {
                          setGroupMemberIds((prev) => prev.filter((id) => id !== employeeCode));
                        }
                      }}
                    />
                    <span>{item.name}（{employeeCode}）</span>
                  </label>
                );
              })}
            </div>
          </div>
          <div className="space-y-1">
            {teamFilter !== "all" && (
              <div className="mb-2 flex items-center justify-between rounded border border-blue-100 bg-blue-50 px-2 py-1.5 text-[11px] text-blue-700">
                <span>当前筛选：{teamFilterLabel}</span>
                <button type="button" onClick={() => setTeamFilter("all")} className="text-blue-700 hover:text-blue-800">清除筛选</button>
              </div>
            )}
            {filteredGroups.length === 0 ? (
              <div className="text-xs text-gray-500">{teamFilter === "all" ? "暂无协作群组" : "当前筛选下暂无团队"}</div>
            ) : (
              filteredGroups.map((group) => (
                <div key={group.id} data-testid={`employee-group-item-${group.id}`} className="rounded border border-gray-200 px-2 py-1.5 space-y-2">
                  {(() => {
                    const templateId = group.template_id?.trim() || "";
                    const entryEmployeeId = group.entry_employee_id?.trim() || group.coordinator_employee_id;
                    const reviewMode = group.review_mode?.trim() || "none";
                    const executionMode = group.execution_mode?.trim() || "sequential";
                    const visibilityMode = group.visibility_mode?.trim() || "internal";
                    const groupConfig = parseGroupTemplateConfig(group.config_json);
                    const groupRules = groupRulesById[group.id] || [];
                    return (
                      <>
                  <div className="flex items-center justify-between gap-2">
                    <div className="text-xs text-gray-700">
                      <span className="font-medium">{group.name}</span>
                      <span className="text-gray-500"> · 协调员 {group.coordinator_employee_id} · {group.member_count} 人</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <button
                        type="button"
                        data-testid={`employee-team-clone-${group.id}`}
                        onClick={() => cloneEmployeeGroup(group)}
                        disabled={cloningGroupId === group.id}
                        className="h-7 px-2 rounded border border-blue-200 hover:bg-blue-50 disabled:bg-gray-100 text-blue-700 text-xs"
                      >
                        {cloningGroupId === group.id ? "复制中..." : "复制模板"}
                      </button>
                      <button
                        type="button"
                        data-testid={`employee-group-delete-${group.id}`}
                        onClick={() => deleteEmployeeGroup(group.id)}
                        disabled={groupDeletingId === group.id}
                        className="h-7 px-2 rounded border border-red-200 hover:bg-red-50 disabled:bg-gray-100 text-red-600 text-xs"
                      >
                        {groupDeletingId === group.id ? "删除中..." : "删除"}
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
                      value={groupRunGoalById[group.id] || ""}
                      onChange={(e) =>
                        setGroupRunGoalById((prev) => ({ ...prev, [group.id]: e.target.value }))
                      }
                    />
                    <button
                      type="button"
                      data-testid={`employee-group-run-start-${group.id}`}
                      onClick={() => startEmployeeGroupRun(group.id)}
                      disabled={groupRunSubmittingId === group.id}
                      className="h-7 px-2.5 rounded bg-indigo-600 hover:bg-indigo-700 disabled:bg-indigo-300 text-white text-xs"
                    >
                      {groupRunSubmittingId === group.id ? "执行中..." : "以团队模式发起任务"}
                    </button>
                  </div>
                  {groupRunReportById[group.id] && (
                    <div
                      data-testid={`employee-group-run-report-${group.id}`}
                      className="rounded border border-indigo-100 bg-indigo-50 px-2 py-1.5 text-[11px] text-indigo-900 whitespace-pre-wrap"
                    >
                      {groupRunReportById[group.id]}
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

                <div className="rounded-lg border border-gray-200 p-3 space-y-2">
                  <div className="flex items-center justify-between gap-2">
                    <div className="text-xs font-medium text-gray-700">AGENTS / SOUL / USER（只读）</div>
                    <button
                      type="button"
                      onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "update", employeeId: selectedEmployee.id })}
                      className="h-7 px-2.5 rounded border border-blue-200 hover:bg-blue-50 text-blue-700 text-xs"
                    >
                      更新画像
                    </button>
                  </div>
                  {profileLoading ? <div className="text-xs text-gray-500">正在加载配置文件...</div> : profileView ? <>
                    <div className="text-[11px] text-gray-500 break-all">目录：{profileView.profile_dir}</div>
                    <div className="grid grid-cols-1 md:grid-cols-3 gap-2">{profileView.files.map((file) => (
                      <div key={file.name} className="border border-gray-100 rounded p-2 space-y-1">
                        <div className="text-xs font-medium text-gray-700">{file.name} {file.exists ? "" : "（未生成）"}</div>
                        {file.error ? <div className="text-[11px] text-red-600">读取失败：{file.error}</div> : file.exists ? <pre className="text-[11px] text-gray-600 whitespace-pre-wrap max-h-56 overflow-y-auto">{file.content}</pre> : <div className="text-[11px] text-gray-500">尚未生成。可使用“智能体员工助手”补齐配置。</div>}
                      </div>
                    ))}</div>
                  </> : <div className="text-xs text-gray-500">暂无可展示的配置文件。</div>}
                </div>

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

            <div className="rounded-lg border border-indigo-200 bg-indigo-50 p-3 space-y-2">
              <div className="flex items-center justify-between gap-2"><div className="text-xs font-medium text-indigo-900">长期记忆管理</div>{memoryLoading && <div className="text-[11px] text-indigo-600">统计刷新中...</div>}</div>
              <div className="grid grid-cols-1 md:grid-cols-4 gap-2">
                <select data-testid="employee-memory-scope" className="border border-indigo-200 rounded px-2 py-1.5 text-xs bg-white" value={memoryScopeSkillId} onChange={(e) => setMemoryScopeSkillId(e.target.value)}>
                  <option value="__all__">全部技能</option>
                  {memorySkillScopeOptions.map((id) => <option key={id} value={id}>{id}</option>)}
                </select>
                <button type="button" data-testid="employee-memory-refresh" onClick={() => refreshEmployeeMemoryStats()} disabled={memoryLoading || memoryActionLoading !== null || !selectedEmployeeMemoryId} className="h-8 rounded border border-indigo-200 hover:bg-indigo-100 disabled:bg-gray-100 text-indigo-700 text-xs">刷新统计</button>
                <button type="button" data-testid="employee-memory-export" onClick={exportEmployeeMemory} disabled={memoryLoading || memoryActionLoading !== null || !selectedEmployeeMemoryId} className="h-8 rounded border border-indigo-200 hover:bg-indigo-100 disabled:bg-gray-100 text-indigo-700 text-xs">{memoryActionLoading === "export" ? "导出中..." : "导出 JSON"}</button>
                <button type="button" data-testid="employee-memory-clear" onClick={() => setPendingClearMemory(true)} disabled={memoryLoading || memoryActionLoading !== null || !selectedEmployeeMemoryId} className="h-8 rounded border border-red-200 hover:bg-red-50 disabled:bg-gray-100 text-red-600 text-xs">清空记忆</button>
              </div>
              <div className="text-xs text-indigo-800 flex items-center gap-4"><span data-testid="employee-memory-total-files">文件数：{memoryStats?.total_files ?? 0}</span><span data-testid="employee-memory-total-bytes">大小：{memoryStats?.total_bytes ?? 0}</span><span>({formatBytes(memoryStats?.total_bytes ?? 0)})</span></div>
              <div className="max-h-32 overflow-y-auto rounded border border-indigo-100 bg-white p-2 space-y-1">
                {(memoryStats?.skills || []).length === 0 ? <div className="text-[11px] text-gray-500">暂无长期记忆文件</div> : (memoryStats?.skills || []).map((item) => <div key={item.skill_id} data-testid={`employee-memory-skill-${item.skill_id}`} className="text-[11px] text-gray-700 flex items-center justify-between"><span>{item.skill_id}</span><span>{item.total_files} 文件 / {formatBytes(item.total_bytes)}</span></div>)}
              </div>
            </div>
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
            <div className="rounded-xl border border-gray-200 bg-white p-4 space-y-3">
              <div>
                <div className="text-sm font-medium text-gray-900">最近运行</div>
                <div className="text-xs text-gray-500 mt-1">统一查看最近发起的团队任务与执行状态。</div>
              </div>
              {runFilter !== "all" && (
                <div className="flex items-center justify-between rounded border border-blue-100 bg-blue-50 px-3 py-2 text-xs text-blue-700">
                  <span>当前筛选：{runFilterLabel}</span>
                  <button type="button" onClick={() => setRunFilter("all")} className="text-blue-700 hover:text-blue-800">
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
                          {run.group_name || "未命名团队"} · {resolveRunStatusLabel(run.status)} · {formatRunTimestamp(run.started_at)}
                        </div>
                      </div>
                      <button
                        type="button"
                        onClick={() => {
                          if (run.session_id && run.session_skill_id) {
                            void onOpenGroupRunSession?.(run.session_id, run.session_skill_id);
                            return;
                          }
                          openTeamsTab("all");
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
        )}
      </div>
      <RiskConfirmDialog open={pendingClearMemory} level="high" title="清空长期记忆" summary={clearMemoryDialogSummary} impact={clearMemoryDialogImpact} irreversible confirmLabel="确认清空" cancelLabel="取消" loading={memoryActionLoading === "clear"} onConfirm={confirmClearEmployeeMemory} onCancel={() => setPendingClearMemory(false)} />
      <RiskConfirmDialog open={Boolean(pendingDeleteEmployee)} level="high" title="删除员工" summary={deleteDialogSummary} impact={deleteDialogImpact} irreversible confirmLabel="确认删除" cancelLabel="取消" loading={saving} onConfirm={confirmRemoveCurrent} onCancel={() => setPendingDeleteEmployee(null)} />
    </div>
  );
}
