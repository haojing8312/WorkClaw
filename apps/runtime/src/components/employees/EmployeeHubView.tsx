import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  AgentEmployee,
  EmployeeGroup,
  EmployeeGroupRunResult,
  AgentProfileFilesView,
  EmployeeMemoryExport,
  EmployeeMemoryStats,
  FeishuEmployeeConnectionStatuses,
  FeishuEmployeeWsStatus,
  RuntimePreferences,
  SkillManifest,
  UpsertAgentEmployeeInput,
} from "../../types";
import { RiskConfirmDialog } from "../RiskConfirmDialog";

interface Props {
  employees: AgentEmployee[];
  skills: SkillManifest[];
  selectedEmployeeId: string | null;
  onSelectEmployee: (id: string) => void;
  onSaveEmployee: (input: UpsertAgentEmployeeInput) => Promise<void>;
  onDeleteEmployee: (employeeId: string) => Promise<void>;
  onSetAsMainAndEnter: (employeeId: string) => void;
  onStartTaskWithEmployee: (employeeId: string) => Promise<void> | void;
  onOpenGroupRunSession?: (sessionId: string, skillId: string) => Promise<void> | void;
  onOpenEmployeeCreatorSkill?: (options?: { mode?: "create" | "update"; employeeId?: string }) => Promise<void> | void;
  highlightEmployeeId?: string | null;
  highlightMessage?: string | null;
  onDismissHighlight?: () => void;
}

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
  selectedEmployeeId,
  onSelectEmployee,
  onSaveEmployee,
  onDeleteEmployee,
  onSetAsMainAndEnter,
  onStartTaskWithEmployee,
  onOpenGroupRunSession,
  onOpenEmployeeCreatorSkill,
  highlightEmployeeId,
  highlightMessage,
  onDismissHighlight,
}: Props) {
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [feishuStatuses, setFeishuStatuses] = useState<FeishuEmployeeConnectionStatuses | null>(null);
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
  const [savingFeishuConfig, setSavingFeishuConfig] = useState(false);
  const [retryingFeishuConnection, setRetryingFeishuConnection] = useState(false);
  const [feishuForm, setFeishuForm] = useState({
    openId: "",
    appId: "",
    appSecret: "",
  });
  const [groupName, setGroupName] = useState("");
  const [groupCoordinatorId, setGroupCoordinatorId] = useState("");
  const [groupMemberIds, setGroupMemberIds] = useState<string[]>([]);
  const [employeeGroups, setEmployeeGroups] = useState<EmployeeGroup[]>([]);
  const [groupSubmitting, setGroupSubmitting] = useState(false);
  const [groupDeletingId, setGroupDeletingId] = useState<string | null>(null);
  const [groupRunGoalById, setGroupRunGoalById] = useState<Record<string, string>>({});
  const [groupRunSubmittingId, setGroupRunSubmittingId] = useState<string | null>(null);
  const [groupRunReportById, setGroupRunReportById] = useState<Record<string, string>>({});

  const selectedEmployee = useMemo(
    () => employees.find((item) => item.id === selectedEmployeeId) ?? null,
    [employees, selectedEmployeeId],
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
    const normalized = employees
      .map((item) => (item.employee_id || item.role_id || "").trim())
      .filter((item) => item.length > 0);
    setGroupMemberIds((prev) => prev.filter((id) => normalized.includes(id)));
    setGroupCoordinatorId((prev) => (normalized.includes(prev) ? prev : normalized[0] || ""));
  }, [employees]);

  async function loadEmployeeGroups() {
    try {
      const groups = await invoke<EmployeeGroup[]>("list_employee_groups");
      setEmployeeGroups(Array.isArray(groups) ? groups : []);
    } catch {
      setEmployeeGroups([]);
    }
  }

  useEffect(() => {
    void loadEmployeeGroups();
  }, []);

  useEffect(() => {
    let disposed = false;
    const loadStatuses = async () => {
      try {
        const snapshot = await invoke<FeishuEmployeeConnectionStatuses>("get_feishu_employee_connection_statuses", {
          sidecarBaseUrl: null,
        });
        if (!disposed) setFeishuStatuses(snapshot);
      } catch {
        if (!disposed) setFeishuStatuses(null);
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
    setMemoryScopeSkillId("__all__");
    setMemoryStats(null);
    setPendingClearMemory(false);
  }, [selectedEmployeeId]);

  useEffect(() => {
    if (!selectedEmployee) {
      setFeishuForm({ openId: "", appId: "", appSecret: "" });
      setProfileView(null);
      setProfileLoading(false);
      return;
    }
    setFeishuForm({
      openId: selectedEmployee.feishu_open_id || "",
      appId: selectedEmployee.feishu_app_id || "",
      appSecret: selectedEmployee.feishu_app_secret || "",
    });

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

  const feishuStatusByEmployeeId = useMemo(() => {
    const map = new Map<string, FeishuEmployeeWsStatus>();
    for (const item of feishuStatuses?.sidecar?.items || []) {
      map.set((item.employee_id || "").trim().toLowerCase(), item);
    }
    return map;
  }, [feishuStatuses]);

  const relayRunning = feishuStatuses?.relay?.running ?? false;

  function resolveFeishuStatus(employee: AgentEmployee): { dotClass: string; label: string; detail: string; error: string } {
    const enabled = !!employee.enabled;
    const hasCredentials = !!employee.feishu_app_id.trim() && !!employee.feishu_app_secret.trim();
    if (!enabled) {
      return { dotClass: "bg-gray-300", label: "未启用飞书消息", detail: "该员工已停用，不接收飞书事件。", error: "" };
    }
    if (!hasCredentials) {
      return { dotClass: "bg-gray-300", label: "待配置飞书凭据", detail: "请填写 App ID / App Secret 并保存。", error: "" };
    }
    const key = employeeKey(employee).toLowerCase();
    const sidecarStatus = key ? feishuStatusByEmployeeId.get(key) : undefined;
    if (sidecarStatus?.running && relayRunning) {
      return { dotClass: "bg-emerald-500", label: "飞书连接正常", detail: "长连接与事件转发器均已运行。", error: "" };
    }
    const error =
      sidecarStatus?.last_error?.trim() ||
      feishuStatuses?.relay?.last_error?.trim() ||
      (!relayRunning ? "事件 relay 未运行" : "飞书长连接未运行");
    if (!relayRunning) {
      return { dotClass: "bg-amber-500", label: "待启动飞书事件转发", detail: "可点击“重试连接”自动拉起转发器。", error };
    }
    return { dotClass: "bg-red-500", label: "飞书连接异常", detail: "请检查飞书凭据并重试连接。", error };
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

  async function saveFeishuConfig() {
    if (!selectedEmployee) return;
    const employeeId = employeeKey(selectedEmployee);
    if (!employeeId) {
      setMessage("员工编号缺失，无法保存飞书配置");
      return;
    }
    setSavingFeishuConfig(true);
    setMessage("");
    try {
      await onSaveEmployee({
        id: selectedEmployee.id,
        employee_id: employeeId,
        name: selectedEmployee.name,
        role_id: employeeId,
        persona: selectedEmployee.persona,
        feishu_open_id: feishuForm.openId.trim(),
        feishu_app_id: feishuForm.appId.trim(),
        feishu_app_secret: feishuForm.appSecret.trim(),
        primary_skill_id: selectedEmployee.primary_skill_id || "",
        default_work_dir: selectedEmployee.default_work_dir || "",
        openclaw_agent_id: selectedEmployee.openclaw_agent_id || employeeId,
        routing_priority: Number.isFinite(selectedEmployee.routing_priority)
          ? selectedEmployee.routing_priority
          : 100,
        enabled_scopes: selectedEmployee.enabled_scopes?.length ? selectedEmployee.enabled_scopes : ["feishu"],
        enabled: selectedEmployee.enabled,
        is_default: selectedEmployee.is_default,
        skill_ids: selectedEmployee.skill_ids,
      });
      setMessage("飞书配置已保存");
    } catch (e) {
      setMessage(`保存飞书配置失败: ${String(e)}`);
    } finally {
      setSavingFeishuConfig(false);
    }
  }

  async function retryFeishuConnection() {
    const appId = feishuForm.appId.trim();
    const appSecret = feishuForm.appSecret.trim();
    if (!appId || !appSecret) {
      setMessage("请先填写并保存 App ID / App Secret，再重试连接");
      return;
    }
    setRetryingFeishuConnection(true);
    setMessage("");
    try {
      await invoke("start_feishu_long_connection", { sidecarBaseUrl: null, appId, appSecret });
      await invoke("start_feishu_event_relay", { sidecarBaseUrl: null, intervalMs: 1500, limit: 50 });
      const latest = await invoke<FeishuEmployeeConnectionStatuses>("get_feishu_employee_connection_statuses", {
        sidecarBaseUrl: null,
      });
      setFeishuStatuses(latest);
      setMessage("已触发飞书重连，请等待几秒后查看状态");
    } catch (e) {
      setMessage(`重试飞书连接失败: ${String(e)}`);
    } finally {
      setRetryingFeishuConnection(false);
    }
  }

  async function createEmployeeGroup() {
    const name = groupName.trim();
    const coordinator = groupCoordinatorId.trim();
    const members = Array.from(new Set(groupMemberIds.map((item) => item.trim()).filter((item) => item.length > 0)));

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

    setGroupSubmitting(true);
    setMessage("");
    try {
      await invoke<string>("create_employee_group", {
        input: {
          name,
          coordinator_employee_id: coordinator,
          member_employee_ids: members,
        },
      });
      setGroupName("");
      setGroupMemberIds([coordinator]);
      await loadEmployeeGroups();
      setMessage("协作群组已创建");
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
      await loadEmployeeGroups();
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
      setGroupRunReportById((prev) => ({ ...prev, [groupId]: result.final_report || "" }));
      if (result.session_id && result.session_skill_id) {
        await onOpenGroupRunSession?.(result.session_id, result.session_skill_id);
      }
      setMessage(`协作任务已完成（第 ${result.current_round || 1} 轮）`);
    } catch (e) {
      setMessage(`发起协作失败: ${String(e)}`);
    } finally {
      setGroupRunSubmittingId(null);
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

  const deleteDialogSummary = pendingDeleteEmployee ? `确定删除员工「${pendingDeleteEmployee.name}」吗？` : "确定删除该员工吗？";
  const deleteDialogImpact = pendingDeleteEmployee ? `员工ID: ${pendingDeleteEmployee.id}` : undefined;
  const clearMemoryScopeLabel = memoryScopeSkillId === "__all__" ? "全部技能" : `技能 ${memoryScopeSkillId}`;
  const clearMemoryDialogSummary = selectedEmployee
    ? `确定清空员工「${selectedEmployee.name}」在${clearMemoryScopeLabel}下的长期记忆吗？`
    : `确定清空${clearMemoryScopeLabel}下的长期记忆吗？`;
  const clearMemoryDialogImpact = selectedEmployeeMemoryId ? `员工编号: ${selectedEmployeeMemoryId}` : undefined;
  const selectedEmployeeFeishuStatus = selectedEmployee ? resolveFeishuStatus(selectedEmployee) : null;

  return (
    <div className="h-full overflow-y-auto bg-gray-50">
      <div className="max-w-6xl mx-auto px-8 pt-10 pb-12 space-y-4">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900">智能体员工</h1>
          <p className="text-sm text-gray-600 mt-2">用员工编号统一管理 OpenClaw 与飞书路由。主员工默认进入且拥有全技能权限。</p>
        </div>
        <div className="rounded-xl border border-blue-200 bg-blue-50 px-4 py-3 flex flex-col md:flex-row md:items-center md:justify-between gap-3">
          <div>
            <div className="text-sm font-medium text-blue-900">推荐：使用内置「智能体员工助手」技能</div>
            <div className="text-xs text-blue-700 mt-1">通过对话描述岗位需求，系统会自动给出技能匹配与配置建议，并在你确认后创建员工。</div>
          </div>
          <button type="button" data-testid="open-employee-creator-skill" onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "create" })} className="h-9 px-4 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-sm">打开员工助手</button>
        </div>
        {highlightMessage && (
          <div data-testid="employee-creator-highlight" className="rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 flex items-center justify-between gap-3">
            <div className="text-xs text-emerald-800">{highlightMessage}</div>
            <button type="button" data-testid="employee-creator-highlight-dismiss" onClick={() => onDismissHighlight?.()} className="h-7 px-2.5 rounded border border-emerald-200 hover:bg-emerald-100 text-emerald-700 text-xs">知道了</button>
          </div>
        )}
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
            {employeeGroups.length === 0 ? (
              <div className="text-xs text-gray-500">暂无协作群组</div>
            ) : (
              employeeGroups.map((group) => (
                <div key={group.id} data-testid={`employee-group-item-${group.id}`} className="rounded border border-gray-200 px-2 py-1.5 space-y-2">
                  <div className="flex items-center justify-between gap-2">
                    <div className="text-xs text-gray-700">
                      <span className="font-medium">{group.name}</span>
                      <span className="text-gray-500"> · 协调员 {group.coordinator_employee_id} · {group.member_count} 人</span>
                    </div>
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
                  <div className="flex items-center gap-2">
                    <input
                      data-testid={`employee-group-run-goal-${group.id}`}
                      className="flex-1 border border-gray-200 rounded px-2 py-1.5 text-xs"
                      placeholder="给该群组发送协作指令"
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
                      {groupRunSubmittingId === group.id ? "执行中..." : "发起协作"}
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
                </div>
              ))
            )}
          </div>
        </div>
        <div className="bg-white border border-gray-200 rounded-xl p-4 space-y-2">
          <div className="text-xs text-gray-500">全局默认工作目录（新建会话默认使用）</div>
          <input className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm" placeholder="例如 D:\\workspace\\workclaw" value={globalDefaultWorkDir} onChange={(e) => setGlobalDefaultWorkDir(e.target.value)} />
          <div className="text-[11px] text-gray-500">默认：C:\Users\&lt;用户名&gt;\WorkClaw\workspace。支持 C/D/E 盘路径，目录不存在会自动创建。</div>
          <button disabled={savingGlobalWorkDir} onClick={saveGlobalDefaultWorkDir} className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-xs">保存默认目录</button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="bg-white border border-gray-200 rounded-xl p-3 max-h-[640px] overflow-y-auto">
            <div className="text-xs text-gray-500 mb-2">员工列表</div>
            <div className="mb-2"><button type="button" onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "create" })} className="h-8 w-full rounded bg-blue-600 hover:bg-blue-700 text-white text-xs">新建员工</button></div>
            <div className="space-y-2">
              {employees.map((employee) => {
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

                {selectedEmployeeFeishuStatus && (
                  <div className="rounded-lg border border-gray-200 p-3 space-y-2">
                    <div className="text-xs font-medium text-gray-700">飞书连接与配置</div>
                    <div className="flex items-center gap-2"><span className={`inline-block h-2.5 w-2.5 rounded-full ${selectedEmployeeFeishuStatus.dotClass}`} /><span className="text-xs text-gray-900">{selectedEmployeeFeishuStatus.label}</span></div>
                    <div className="text-[11px] text-gray-500">{selectedEmployeeFeishuStatus.detail}</div>
                    {selectedEmployeeFeishuStatus.error && <div className="text-xs text-red-600">{selectedEmployeeFeishuStatus.error}</div>}
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                      <input className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm md:col-span-2" placeholder="飞书机器人 open_id（可空，仅用于飞书@精准路由）" value={feishuForm.openId} onChange={(e) => setFeishuForm((s) => ({ ...s, openId: e.target.value }))} />
                      <input className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm" placeholder="机器人 App ID" value={feishuForm.appId} onChange={(e) => setFeishuForm((s) => ({ ...s, appId: e.target.value }))} />
                      <input className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm" type="password" placeholder="机器人 App Secret" value={feishuForm.appSecret} onChange={(e) => setFeishuForm((s) => ({ ...s, appSecret: e.target.value }))} />
                    </div>
                    <div className="flex items-center gap-2">
                      <button type="button" onClick={saveFeishuConfig} disabled={savingFeishuConfig} className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-xs">{savingFeishuConfig ? "保存中..." : "保存飞书配置"}</button>
                      <button type="button" onClick={retryFeishuConnection} disabled={retryingFeishuConnection} className="h-8 px-3 rounded border border-blue-200 hover:bg-blue-50 disabled:bg-gray-100 text-blue-700 text-xs">{retryingFeishuConnection ? "重试中..." : "重试连接"}</button>
                    </div>
                  </div>
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
                  <button disabled={!selectedEmployeeId || saving} onClick={() => selectedEmployeeId && onStartTaskWithEmployee(selectedEmployeeId)} className="h-8 px-3 rounded bg-indigo-50 hover:bg-indigo-100 disabled:bg-gray-100 text-indigo-700 text-xs">与该员工对话开始任务</button>
                </div>
              </>
            ) : (
              <div className="rounded-lg border border-dashed border-gray-300 p-4 space-y-2">
                <div className="text-sm font-medium text-gray-800">请选择一个员工或直接创建</div>
                <div className="text-xs text-gray-600">已移除手动创建流程，请通过「智能体员工助手」对话式完成创建与配置。</div>
                <button type="button" onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "create" })} className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 text-white text-xs">创建员工</button>
              </div>
            )}

            {message && <div className="text-xs text-blue-700 bg-blue-50 border border-blue-100 rounded px-2 py-1">{message}</div>}

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
      <RiskConfirmDialog open={pendingClearMemory} level="high" title="清空长期记忆" summary={clearMemoryDialogSummary} impact={clearMemoryDialogImpact} irreversible confirmLabel="确认清空" cancelLabel="取消" loading={memoryActionLoading === "clear"} onConfirm={confirmClearEmployeeMemory} onCancel={() => setPendingClearMemory(false)} />
      <RiskConfirmDialog open={Boolean(pendingDeleteEmployee)} level="high" title="删除员工" summary={deleteDialogSummary} impact={deleteDialogImpact} irreversible confirmLabel="确认删除" cancelLabel="取消" loading={saving} onConfirm={confirmRemoveCurrent} onCancel={() => setPendingDeleteEmployee(null)} />
    </div>
  );
}
