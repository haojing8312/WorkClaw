import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  AgentEmployee,
  EmployeeMemoryExport,
  EmployeeMemoryStats,
  FeishuEmployeeConnectionStatuses,
  FeishuEmployeeWsStatus,
  RecentImThread,
  RuntimePreferences,
  SkillManifest,
  ThreadEmployeeBinding,
  UpsertAgentEmployeeInput,
} from "../../types";
import { RiskConfirmDialog } from "../RiskConfirmDialog";
import { AgentProfileChatWizard } from "./AgentProfileChatWizard";

interface Props {
  employees: AgentEmployee[];
  skills: SkillManifest[];
  selectedEmployeeId: string | null;
  onSelectEmployee: (id: string) => void;
  onSaveEmployee: (input: UpsertAgentEmployeeInput) => Promise<void>;
  onDeleteEmployee: (employeeId: string) => Promise<void>;
  onSetAsMainAndEnter: (employeeId: string) => void;
  onStartTaskWithEmployee: (employeeId: string) => Promise<void> | void;
  onOpenEmployeeCreatorSkill?: () => Promise<void> | void;
  highlightEmployeeId?: string | null;
  highlightMessage?: string | null;
  onDismissHighlight?: () => void;
}

const blankForm: UpsertAgentEmployeeInput = {
  id: undefined,
  employee_id: "",
  name: "",
  role_id: "",
  persona: "",
  feishu_open_id: "",
  feishu_app_id: "",
  feishu_app_secret: "",
  primary_skill_id: "",
  default_work_dir: "",
  openclaw_agent_id: "",
  routing_priority: 100,
  enabled_scopes: ["feishu"],
  enabled: true,
  is_default: false,
  skill_ids: [],
};

const employeeTemplates: Array<{ name: string; employeeId: string; persona: string }> = [
  {
    name: "项目经理",
    employeeId: "project_manager",
    persona: "负责需求澄清、任务拆解、里程碑推进与风险管理，优先输出可执行计划与验收标准。",
  },
  {
    name: "技术负责人",
    employeeId: "tech_lead",
    persona: "负责技术方案评审、架构决策和质量把关，强调可维护性、测试覆盖和交付稳定性。",
  },
  {
    name: "运营专员",
    employeeId: "operations",
    persona: "负责运营数据分析、活动复盘与流程优化，输出可落地行动项和指标跟踪方案。",
  },
  {
    name: "客服专员",
    employeeId: "customer_success",
    persona: "负责用户问题分级、解决路径设计与满意度提升，提供清晰且可执行的处理建议。",
  },
];

function toEmployeeIdBase(input: string): string {
  const normalized = input
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .replace(/_+/g, "_");
  return normalized || "employee";
}

function ensureUniqueEmployeeId(base: string, employees: AgentEmployee[], currentDbId?: string): string {
  const taken = new Set(
    employees
      .filter((item) => item.id !== currentDbId)
      .map((item) => (item.employee_id || item.role_id || "").trim().toLowerCase())
      .filter((id) => id.length > 0),
  );
  if (!taken.has(base.toLowerCase())) {
    return base;
  }
  let index = 2;
  while (taken.has(`${base}_${index}`.toLowerCase())) {
    index += 1;
  }
  return `${base}_${index}`;
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
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
  onOpenEmployeeCreatorSkill,
  highlightEmployeeId,
  highlightMessage,
  onDismissHighlight,
}: Props) {
  const [form, setForm] = useState<UpsertAgentEmployeeInput>(blankForm);
  const [employeeIdEdited, setEmployeeIdEdited] = useState(false);
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
  const [recentThreads, setRecentThreads] = useState<RecentImThread[]>([]);
  const [threadBindingsByThread, setThreadBindingsByThread] = useState<Record<string, string[]>>({});
  const [threadBindingLoading, setThreadBindingLoading] = useState(false);
  const [selectedThreadId, setSelectedThreadId] = useState("");
  const [selectedThreadEmployeeId, setSelectedThreadEmployeeId] = useState("");
  const [threadBindingSaving, setThreadBindingSaving] = useState(false);
  const [threadBindingMessage, setThreadBindingMessage] = useState("");

  const skillOptions = useMemo(
    () => skills.filter((s) => s.id !== "builtin-general"),
    [skills],
  );
  const selectedEmployee = useMemo(
    () => employees.find((item) => item.id === selectedEmployeeId) ?? null,
    [employees, selectedEmployeeId],
  );
  const employeeNameByDbId = useMemo(() => {
    const map = new Map<string, string>();
    for (const employee of employees) {
      map.set(employee.id, employee.name || employee.employee_id || employee.role_id || employee.id);
    }
    return map;
  }, [employees]);
  const selectedEmployeeMemoryId = useMemo(
    () => (selectedEmployee?.employee_id || selectedEmployee?.role_id || "").trim(),
    [selectedEmployee],
  );
  const memorySkillScopeOptions = useMemo(() => {
    if (!selectedEmployee) return [];
    const ids = new Set<string>();
    if (selectedEmployee.primary_skill_id.trim()) {
      ids.add(selectedEmployee.primary_skill_id.trim());
    }
    for (const id of selectedEmployee.skill_ids) {
      const normalized = id.trim();
      if (normalized) ids.add(normalized);
    }
    return Array.from(ids.values());
  }, [selectedEmployee]);

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
    let disposed = false;
    const loadStatuses = async () => {
      try {
        const snapshot = await invoke<FeishuEmployeeConnectionStatuses>(
          "get_feishu_employee_connection_statuses",
          { sidecarBaseUrl: null },
        );
        if (!disposed) {
          setFeishuStatuses(snapshot);
        }
      } catch {
        if (!disposed) {
          setFeishuStatuses(null);
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

  async function loadRecentThreadsAndBindings() {
    setThreadBindingLoading(true);
    setThreadBindingMessage("");
    try {
      const threads = await invoke<RecentImThread[]>("list_recent_im_threads", { limit: 20 });
      const safeThreads = Array.isArray(threads) ? threads : [];
      setRecentThreads(safeThreads);

      const entries: Array<[string, string[]]> = await Promise.all(
        safeThreads.map(async (thread) => {
          try {
            const binding = await invoke<ThreadEmployeeBinding>("get_thread_employee_bindings", {
              threadId: thread.thread_id,
            });
            const employeeIds = Array.isArray(binding?.employee_ids)
              ? binding.employee_ids.filter((id) => id.trim().length > 0).slice(0, 1)
              : [];
            return [thread.thread_id, employeeIds];
          } catch {
            return [thread.thread_id, []];
          }
        }),
      );
      const nextMap: Record<string, string[]> = {};
      for (const [threadId, employeeIds] of entries) {
        nextMap[threadId] = employeeIds;
      }
      setThreadBindingsByThread(nextMap);
      setSelectedThreadId((prev) => {
        if (prev && safeThreads.some((thread) => thread.thread_id === prev)) {
          return prev;
        }
        return safeThreads[0]?.thread_id || "";
      });
    } catch (e) {
      setThreadBindingMessage(`加载飞书线程失败: ${String(e)}`);
      setRecentThreads([]);
      setThreadBindingsByThread({});
      setSelectedThreadId("");
      setSelectedThreadEmployeeId("");
    } finally {
      setThreadBindingLoading(false);
    }
  }

  useEffect(() => {
    void loadRecentThreadsAndBindings();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!selectedThreadId) {
      setSelectedThreadEmployeeId("");
      return;
    }
    const ownerId = threadBindingsByThread[selectedThreadId]?.[0] || "";
    setSelectedThreadEmployeeId(ownerId);
  }, [selectedThreadId, threadBindingsByThread]);

  useEffect(() => {
    setMemoryScopeSkillId("__all__");
    setMemoryStats(null);
    setPendingClearMemory(false);
  }, [selectedEmployeeId]);

  const feishuStatusByEmployeeId = useMemo(() => {
    const map = new Map<string, FeishuEmployeeWsStatus>();
    for (const item of feishuStatuses?.sidecar?.items || []) {
      map.set((item.employee_id || "").trim().toLowerCase(), item);
    }
    return map;
  }, [feishuStatuses]);

  const relayRunning = feishuStatuses?.relay?.running ?? false;

  function resolveFeishuStatus(employee: AgentEmployee): {
    dotClass: string;
    label: string;
    error: string;
  } {
    const enabled = !!employee.enabled;
    const hasCredentials = !!employee.feishu_app_id.trim() && !!employee.feishu_app_secret.trim();
    if (!enabled) {
      return { dotClass: "bg-gray-300", label: "未启用", error: "" };
    }
    if (!hasCredentials) {
      return { dotClass: "bg-gray-300", label: "未绑定飞书凭据", error: "" };
    }
    const key = (employee.employee_id || employee.role_id || "").trim().toLowerCase();
    const sidecarStatus = key ? feishuStatusByEmployeeId.get(key) : undefined;
    if (sidecarStatus?.running && relayRunning) {
      return { dotClass: "bg-emerald-500", label: "飞书连接正常", error: "" };
    }
    const error =
      sidecarStatus?.last_error?.trim() ||
      feishuStatuses?.relay?.last_error?.trim() ||
      (!relayRunning ? "事件 relay 未运行" : "飞书长连接未运行");
    return {
      dotClass: "bg-red-500",
      label: "飞书连接异常",
      error,
    };
  }

  async function refreshEmployeeMemoryStats(scopeSkillId?: string) {
    if (!selectedEmployeeMemoryId) {
      setMemoryStats(null);
      return;
    }
    const nextScope = scopeSkillId ?? memoryScopeSkillId;
    const normalizedSkillId = nextScope === "__all__" ? null : nextScope;
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

  function pickEmployee(id: string) {
    onSelectEmployee(id);
    const e = employees.find((x) => x.id === id);
    if (!e) return;
    setForm({
      id: e.id,
      employee_id: e.employee_id || e.role_id || "",
      name: e.name,
      role_id: e.role_id,
      persona: e.persona,
      feishu_open_id: e.feishu_open_id,
      feishu_app_id: e.feishu_app_id,
      feishu_app_secret: e.feishu_app_secret,
      primary_skill_id: e.primary_skill_id || "",
      default_work_dir: e.default_work_dir || "",
      openclaw_agent_id: e.openclaw_agent_id || e.employee_id || e.role_id || "",
      routing_priority: Number.isFinite(e.routing_priority) ? e.routing_priority : 100,
      enabled_scopes: e.enabled_scopes?.length > 0 ? e.enabled_scopes : ["feishu"],
      enabled: e.enabled,
      is_default: e.is_default,
      skill_ids: e.is_default ? [] : (e.skill_ids.length > 0 ? e.skill_ids : []),
    });
    setEmployeeIdEdited(true);
  }

  function resetForm() {
    setForm(blankForm);
    setEmployeeIdEdited(false);
    setMessage("");
  }

  function buildEmployeeIdForSave(): string {
    const raw = form.employee_id.trim();
    if (raw) {
      return raw.toLowerCase();
    }
    const generated = toEmployeeIdBase(form.name);
    return ensureUniqueEmployeeId(generated, employees, form.id);
  }

  async function save() {
    setSaving(true);
    setMessage("");
    try {
      const employeeId = buildEmployeeIdForSave();
      await onSaveEmployee({
        ...form,
        employee_id: employeeId,
        role_id: employeeId,
        openclaw_agent_id: employeeId,
        routing_priority: Number.isFinite(form.routing_priority) ? form.routing_priority : 100,
        enabled_scopes: form.enabled_scopes?.length > 0 ? form.enabled_scopes : ["feishu"],
        skill_ids: form.is_default ? [] : form.skill_ids,
      });
      setForm((s) => ({
        ...s,
        employee_id: employeeId,
        role_id: employeeId,
        openclaw_agent_id: employeeId,
      }));
      setEmployeeIdEdited(true);
      setMessage("员工已保存");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
    }
  }

  function requestRemoveCurrent() {
    if (!selectedEmployeeId || saving) return;
    const target = employees.find((x) => x.id === selectedEmployeeId);
    setPendingDeleteEmployee({
      id: selectedEmployeeId,
      name: target?.name ?? selectedEmployeeId,
    });
  }

  async function confirmRemoveCurrent() {
    if (!pendingDeleteEmployee || saving) return;
    setSaving(true);
    setMessage("");
    try {
      await onDeleteEmployee(pendingDeleteEmployee.id);
      resetForm();
      setMessage("员工已删除");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
      setPendingDeleteEmployee(null);
    }
  }

  function cancelRemoveCurrent() {
    if (saving) return;
    setPendingDeleteEmployee(null);
  }

  async function saveGlobalDefaultWorkDir() {
    if (!globalDefaultWorkDir.trim()) {
      setMessage("默认工作目录不能为空");
      return;
    }
    setSavingGlobalWorkDir(true);
    setMessage("");
    try {
      await invoke("set_runtime_preferences", {
        input: { default_work_dir: globalDefaultWorkDir.trim() },
      });
      const resolved = await invoke<string>("resolve_default_work_dir");
      setGlobalDefaultWorkDir(resolved);
      setMessage("全局默认工作目录已保存");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSavingGlobalWorkDir(false);
    }
  }

  function applyEmployeeTemplate(employeeId: string) {
    const tpl = employeeTemplates.find((x) => x.employeeId === employeeId);
    if (!tpl) return;
    const safeEmployeeId = ensureUniqueEmployeeId(tpl.employeeId, employees, form.id);
    setForm((s) => ({
      ...s,
      name: s.name.trim() ? s.name : tpl.name,
      employee_id: safeEmployeeId,
      role_id: safeEmployeeId,
      openclaw_agent_id: safeEmployeeId,
      persona: tpl.persona,
    }));
    setEmployeeIdEdited(true);
  }

  async function saveThreadBinding() {
    if (!selectedThreadId || threadBindingSaving) return;
    setThreadBindingSaving(true);
    setThreadBindingMessage("");
    const employeeIds = selectedThreadEmployeeId ? [selectedThreadEmployeeId] : [];
    try {
      await invoke("bind_thread_employees", {
        threadId: selectedThreadId,
        employeeIds,
      });
      const refreshed = await invoke<ThreadEmployeeBinding>("get_thread_employee_bindings", {
        threadId: selectedThreadId,
      });
      const normalized = Array.isArray(refreshed?.employee_ids)
        ? refreshed.employee_ids.filter((id) => id.trim().length > 0).slice(0, 1)
        : [];
      setThreadBindingsByThread((prev) => ({
        ...prev,
        [selectedThreadId]: normalized,
      }));
      if (normalized.length > 0) {
        const ownerName = employeeNameByDbId.get(normalized[0]) || normalized[0];
        setThreadBindingMessage(`线程 ${selectedThreadId} 已绑定到 ${ownerName}`);
      } else {
        setThreadBindingMessage(`线程 ${selectedThreadId} 已取消绑定`);
      }
    } catch (e) {
      setThreadBindingMessage(`保存 1:1 绑定失败: ${String(e)}`);
    } finally {
      setThreadBindingSaving(false);
    }
  }

  const deleteDialogSummary = pendingDeleteEmployee
    ? `确定删除员工「${pendingDeleteEmployee.name}」吗？`
    : "确定删除该员工吗？";
  const deleteDialogImpact = pendingDeleteEmployee
    ? `员工ID: ${pendingDeleteEmployee.id}`
    : undefined;
  const clearMemoryScopeLabel =
    memoryScopeSkillId === "__all__" ? "全部技能" : `技能 ${memoryScopeSkillId}`;
  const clearMemoryDialogSummary = selectedEmployee
    ? `确定清空员工「${selectedEmployee.name}」在${clearMemoryScopeLabel}下的长期记忆吗？`
    : `确定清空${clearMemoryScopeLabel}下的长期记忆吗？`;
  const clearMemoryDialogImpact = selectedEmployeeMemoryId
    ? `员工编号: ${selectedEmployeeMemoryId}`
    : undefined;
  const selectedThread = recentThreads.find((thread) => thread.thread_id === selectedThreadId) || null;
  const selectedThreadBoundEmployeeId = selectedThread
    ? threadBindingsByThread[selectedThread.thread_id]?.[0] || ""
    : "";
  const selectedThreadBoundEmployeeName = selectedThreadBoundEmployeeId
    ? employeeNameByDbId.get(selectedThreadBoundEmployeeId) || selectedThreadBoundEmployeeId
    : "未绑定";
  const selectedEmployeeFeishuStatus = selectedEmployee
    ? resolveFeishuStatus(selectedEmployee)
    : null;

  return (
    <div className="h-full overflow-y-auto bg-gray-50">
      <div className="max-w-6xl mx-auto px-8 pt-10 pb-12 space-y-4">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900">智能体员工</h1>
          <p className="text-sm text-gray-600 mt-2">
            用员工编号统一管理 OpenClaw 与飞书路由。主员工默认进入且拥有全技能权限。
          </p>
        </div>

        <div className="rounded-xl border border-blue-200 bg-blue-50 px-4 py-3 flex flex-col md:flex-row md:items-center md:justify-between gap-3">
          <div>
            <div className="text-sm font-medium text-blue-900">推荐：使用内置「创建员工」技能</div>
            <div className="text-xs text-blue-700 mt-1">
              通过对话描述岗位需求，系统会自动给出技能匹配与配置建议，并在你确认后创建员工。
            </div>
          </div>
          <button
            type="button"
            data-testid="open-employee-creator-skill"
            onClick={() => onOpenEmployeeCreatorSkill?.()}
            className="h-9 px-4 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-sm"
          >
            使用创建员工助手
          </button>
        </div>

        {highlightMessage && (
          <div
            data-testid="employee-creator-highlight"
            className="rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 flex items-center justify-between gap-3"
          >
            <div className="text-xs text-emerald-800">{highlightMessage}</div>
            <button
              type="button"
              data-testid="employee-creator-highlight-dismiss"
              onClick={() => onDismissHighlight?.()}
              className="h-7 px-2.5 rounded border border-emerald-200 hover:bg-emerald-100 text-emerald-700 text-xs"
            >
              知道了
            </button>
          </div>
        )}

        <div className="bg-white border border-gray-200 rounded-xl p-4 space-y-2">
          <div className="text-xs text-gray-500">全局默认工作目录（新建会话默认使用）</div>
          <input
            className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
            placeholder="例如 D:\\workspace\\workclaw"
            value={globalDefaultWorkDir}
            onChange={(e) => setGlobalDefaultWorkDir(e.target.value)}
          />
          <div className="text-[11px] text-gray-500">
            默认：C:\Users\&lt;用户名&gt;\WorkClaw\workspace。支持 C/D/E 盘路径，目录不存在会自动创建。
          </div>
          <button
            disabled={savingGlobalWorkDir}
            onClick={saveGlobalDefaultWorkDir}
            className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-xs"
          >
            保存默认目录
          </button>
        </div>

        <div className="bg-white border border-gray-200 rounded-xl p-4 space-y-3">
          <div className="flex items-center justify-between gap-2">
            <div>
              <div className="text-sm font-medium text-gray-800">飞书线程 1:1 绑定</div>
              <div className="text-xs text-gray-500 mt-1">
                每个飞书线程只绑定一个智能体员工，后续消息默认由该员工接管。
              </div>
            </div>
            <button
              type="button"
              onClick={() => void loadRecentThreadsAndBindings()}
              disabled={threadBindingLoading || threadBindingSaving}
              className="h-8 px-3 rounded border border-gray-200 hover:bg-gray-50 disabled:bg-gray-100 text-xs text-gray-700"
            >
              {threadBindingLoading ? "刷新中..." : "刷新线程"}
            </button>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div className="rounded-lg border border-gray-200 p-2 max-h-56 overflow-y-auto space-y-1">
              {recentThreads.length === 0 ? (
                <div className="text-xs text-gray-400 px-2 py-4">暂无最近飞书线程</div>
              ) : (
                recentThreads.map((thread) => {
                  const currentOwnerId = threadBindingsByThread[thread.thread_id]?.[0] || "";
                  const currentOwnerName = currentOwnerId
                    ? employeeNameByDbId.get(currentOwnerId) || currentOwnerId
                    : "未绑定";
                  const selected = selectedThreadId === thread.thread_id;
                  return (
                    <button
                      key={thread.thread_id}
                      type="button"
                      data-testid={`thread-binding-row-${thread.thread_id}`}
                      onClick={() => setSelectedThreadId(thread.thread_id)}
                      className={
                        "w-full text-left rounded border px-2 py-2 text-xs transition-colors " +
                        (selected
                          ? "border-blue-300 bg-blue-50"
                          : "border-gray-200 bg-white hover:border-blue-200 hover:bg-blue-50/40")
                      }
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="font-mono text-gray-700 truncate">{thread.thread_id}</span>
                        <span
                          data-testid={`thread-binding-owner-${thread.thread_id}`}
                          className="text-[11px] text-gray-500 truncate"
                        >
                          {currentOwnerName}
                        </span>
                      </div>
                      <div className="text-[11px] text-gray-500 mt-1 line-clamp-1">{thread.last_text_preview || "-"}</div>
                    </button>
                  );
                })
              )}
            </div>
            <div className="rounded-lg border border-gray-200 p-3 space-y-2">
              {selectedThread ? (
                <>
                  <div className="text-xs text-gray-500">线程 ID</div>
                  <div className="text-xs font-mono text-gray-700 break-all">{selectedThread.thread_id}</div>
                  <div className="text-xs text-gray-500">最近消息</div>
                  <div className="text-xs text-gray-700">{selectedThread.last_text_preview || "-"}</div>
                  <div className="text-xs text-gray-500">当前绑定</div>
                  <div className="text-xs text-gray-700">{selectedThreadBoundEmployeeName}</div>
                  <div className="text-xs text-gray-500">绑定员工</div>
                  <select
                    data-testid="thread-binding-employee-select"
                    className="w-full border border-gray-200 rounded px-2 py-1.5 text-xs bg-white"
                    value={selectedThreadEmployeeId}
                    onChange={(e) => setSelectedThreadEmployeeId(e.target.value)}
                    disabled={threadBindingSaving}
                  >
                    <option value="">未绑定</option>
                    {employees.map((employee) => (
                      <option key={employee.id} value={employee.id}>
                        {employee.name} ({employee.employee_id || employee.role_id || employee.id})
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    onClick={() => void saveThreadBinding()}
                    disabled={threadBindingSaving || threadBindingLoading}
                    className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-xs"
                  >
                    {threadBindingSaving ? "保存中..." : "保存 1:1 绑定"}
                  </button>
                </>
              ) : (
                <div className="text-xs text-gray-400">选择左侧线程后设置绑定员工</div>
              )}
            </div>
          </div>
          {threadBindingMessage && (
            <div data-testid="thread-binding-message" className="text-xs text-blue-700 bg-blue-50 border border-blue-100 rounded px-2 py-1">
              {threadBindingMessage}
            </div>
          )}
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="bg-white border border-gray-200 rounded-xl p-3 max-h-[640px] overflow-y-auto">
            <div className="text-xs text-gray-500 mb-2">员工列表</div>
            <div className="mb-2 grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => onOpenEmployeeCreatorSkill?.()}
                className="h-8 rounded bg-blue-600 hover:bg-blue-700 text-white text-xs"
              >
                新建员工
              </button>
              <button
                type="button"
                onClick={resetForm}
                className="h-8 rounded bg-blue-50 hover:bg-blue-100 text-blue-700 text-xs"
              >
                手动新建
              </button>
            </div>
            <div className="space-y-2">
              {employees.map((e) => {
                const status = resolveFeishuStatus(e);
                const isSelected = selectedEmployeeId === e.id;
                const isHighlighted = highlightEmployeeId === e.id;
                return (
                  <button
                    key={e.id}
                    data-testid={`employee-item-${e.id}`}
                    onClick={() => pickEmployee(e.id)}
                    className={
                      "w-full text-left border rounded p-2 text-xs " +
                      (
                        isHighlighted
                          ? "border-emerald-300 bg-emerald-50 ring-1 ring-emerald-200"
                          : isSelected
                          ? "border-blue-300 bg-blue-50"
                          : "border-gray-200 bg-white"
                      )
                    }
                  >
                    <div className="flex items-center gap-2">
                      <span
                        data-testid={`employee-connection-dot-${e.id}`}
                        className={`inline-block h-2.5 w-2.5 rounded-full ${status.dotClass}`}
                        title={status.label}
                      />
                      <div className="font-medium text-gray-800 truncate">
                        {e.name} {e.is_default ? "· 主员工" : ""}
                      </div>
                      {isHighlighted && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-100 text-emerald-700 border border-emerald-200">
                          新建
                        </span>
                      )}
                    </div>
                    <div className="text-gray-500 truncate">{e.employee_id || e.role_id}</div>
                  </button>
                );
              })}
            </div>
            </div>

          <div className="md:col-span-2 bg-white border border-gray-200 rounded-xl p-4 space-y-3">
            <div className="text-xs text-gray-500">员工配置</div>
            <>
            <div className="rounded-lg border border-gray-200 p-3 space-y-2">
              <div className="text-xs font-medium text-gray-700">步骤 1 / 3 · 基础信息</div>
              <input
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                placeholder="员工名称"
                value={form.name}
                onChange={(e) => {
                  const name = e.target.value;
                  setForm((s) => {
                    if (employeeIdEdited) {
                      return { ...s, name };
                    }
                    const base = ensureUniqueEmployeeId(toEmployeeIdBase(name), employees, s.id);
                    return {
                      ...s,
                      name,
                      employee_id: base,
                      role_id: base,
                      openclaw_agent_id: base,
                    };
                  });
                }}
              />
              <input
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                placeholder="员工编号（自动生成，可编辑）"
                value={form.employee_id}
                onChange={(e) => {
                  const employeeId = e.target.value;
                  setEmployeeIdEdited(true);
                  setForm((s) => ({
                    ...s,
                    employee_id: employeeId,
                    role_id: employeeId,
                    openclaw_agent_id: employeeId,
                  }));
                }}
              />
              <textarea
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                rows={2}
                placeholder="角色人格/职责描述"
                value={form.persona}
                onChange={(e) => setForm((s) => ({ ...s, persona: e.target.value }))}
              />
              <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
                {employeeTemplates.map((tpl) => (
                  <button
                    key={tpl.employeeId}
                    type="button"
                    onClick={() => applyEmployeeTemplate(tpl.employeeId)}
                    className="h-8 rounded border border-gray-200 hover:border-blue-300 hover:bg-blue-50 text-xs text-gray-700"
                  >
                    填充{tpl.name}
                  </button>
                ))}
              </div>
            </div>

            <div className="rounded-lg border border-gray-200 p-3 space-y-2">
              <div className="text-xs font-medium text-gray-700">步骤 2 / 3 · 飞书连接</div>
              <input
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                placeholder="飞书机器人 open_id（可空，仅用于飞书@精准路由）"
                value={form.feishu_open_id}
                onChange={(e) => setForm((s) => ({ ...s, feishu_open_id: e.target.value }))}
              />
              <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                <input
                  className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                  placeholder="机器人 App ID（可空）"
                  value={form.feishu_app_id}
                  onChange={(e) => setForm((s) => ({ ...s, feishu_app_id: e.target.value }))}
                />
                <input
                  className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                  type="password"
                  placeholder="机器人 App Secret（可空）"
                  value={form.feishu_app_secret}
                  onChange={(e) => setForm((s) => ({ ...s, feishu_app_secret: e.target.value }))}
                />
              </div>
              {selectedEmployeeFeishuStatus && (
                <div className="space-y-1">
                  <div
                    className={
                      "text-xs " +
                      (selectedEmployeeFeishuStatus.dotClass === "bg-emerald-500"
                        ? "text-emerald-700"
                        : selectedEmployeeFeishuStatus.dotClass === "bg-red-500"
                          ? "text-red-600"
                          : "text-gray-500")
                    }
                  >
                    {selectedEmployeeFeishuStatus.label}
                  </div>
                  {selectedEmployeeFeishuStatus.error && (
                    <div className="text-xs text-red-600">{selectedEmployeeFeishuStatus.error}</div>
                  )}
                </div>
              )}
            </div>

            <div className="rounded-lg border border-gray-200 p-3 space-y-2">
              <div className="text-xs font-medium text-gray-700">步骤 3 / 3 · 技能与智能体配置</div>
              <div className="text-[11px] text-gray-500">主技能（用于新会话默认技能路由）</div>
              <select
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
                value={form.primary_skill_id}
                onChange={(e) => setForm((s) => ({ ...s, primary_skill_id: e.target.value }))}
              >
                <option value="">通用助手（系统默认）</option>
                {skillOptions.map((skill) => (
                  <option key={skill.id} value={skill.id}>
                    {skill.name}
                  </option>
                ))}
              </select>
              <div className="text-[11px] text-gray-500">默认工作目录（该员工新会话默认目录）</div>
              <input
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                placeholder="默认工作目录"
                value={form.default_work_dir}
                onChange={(e) => setForm((s) => ({ ...s, default_work_dir: e.target.value }))}
              />
              <div className="text-xs text-gray-500 mt-1">
                技能合集（补充授权能力；当前会话默认仍优先使用“主技能”）
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-2 max-h-40 overflow-y-auto border border-gray-100 rounded p-2">
                {skillOptions.map((skill) => {
                  const checked = form.is_default || form.skill_ids.includes(skill.id);
                  return (
                    <label key={skill.id} className="inline-flex items-center gap-2 text-xs text-gray-700">
                      <input
                        type="checkbox"
                        checked={checked}
                        disabled={form.is_default}
                        onChange={(e) => {
                          setForm((s) => {
                            if (s.is_default) return s;
                            if (e.target.checked) {
                              return { ...s, skill_ids: Array.from(new Set([...s.skill_ids, skill.id])) };
                            }
                            return { ...s, skill_ids: s.skill_ids.filter((id) => id !== skill.id) };
                          });
                        }}
                      />
                      <span className="truncate">{skill.name}</span>
                    </label>
                  );
                })}
              </div>
            </div>

            <AgentProfileChatWizard employee={selectedEmployee} />

            <div className="flex items-center gap-4 text-xs text-gray-700">
              <label className="inline-flex items-center gap-1">
                <input
                  type="checkbox"
                  checked={form.enabled}
                  onChange={(e) => setForm((s) => ({ ...s, enabled: e.target.checked }))}
                />
                启用
              </label>
              <label className="inline-flex items-center gap-1">
                <input
                  type="checkbox"
                  checked={form.is_default}
                  onChange={(e) => setForm((s) => ({ ...s, is_default: e.target.checked }))}
                />
                设为主员工
              </label>
            </div>

            {message && (
              <div className="text-xs text-blue-700 bg-blue-50 border border-blue-100 rounded px-2 py-1">
                {message}
              </div>
            )}

            <div className="flex items-center gap-2 pt-1">
              <button
                disabled={saving}
                onClick={save}
                className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-xs"
              >
                保存员工
              </button>
              <button
                disabled={saving || !selectedEmployeeId}
                onClick={requestRemoveCurrent}
                className="h-8 px-3 rounded bg-red-50 hover:bg-red-100 disabled:bg-gray-100 text-red-600 text-xs"
              >
                删除员工
              </button>
              <button
                disabled={!selectedEmployeeId}
                onClick={() => selectedEmployeeId && onSetAsMainAndEnter(selectedEmployeeId)}
                className="h-8 px-3 rounded bg-emerald-50 hover:bg-emerald-100 disabled:bg-gray-100 text-emerald-700 text-xs"
              >
                设为主员工并进入首页
              </button>
              <button
                disabled={!selectedEmployeeId || saving}
                onClick={() => selectedEmployeeId && onStartTaskWithEmployee(selectedEmployeeId)}
                className="h-8 px-3 rounded bg-indigo-50 hover:bg-indigo-100 disabled:bg-gray-100 text-indigo-700 text-xs"
              >
                与该员工对话开始任务
              </button>
            </div>
            </>
            <div className="rounded-lg border border-indigo-200 bg-indigo-50 p-3 space-y-2">
              <div className="flex items-center justify-between gap-2">
                <div className="text-xs font-medium text-indigo-900">长期记忆管理</div>
                {memoryLoading && <div className="text-[11px] text-indigo-600">统计刷新中...</div>}
              </div>
              <div className="grid grid-cols-1 md:grid-cols-4 gap-2">
                <select
                  data-testid="employee-memory-scope"
                  className="border border-indigo-200 rounded px-2 py-1.5 text-xs bg-white"
                  value={memoryScopeSkillId}
                  onChange={(e) => setMemoryScopeSkillId(e.target.value)}
                >
                  <option value="__all__">全部技能</option>
                  {memorySkillScopeOptions.map((id) => (
                    <option key={id} value={id}>
                      {id}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  data-testid="employee-memory-refresh"
                  onClick={() => refreshEmployeeMemoryStats()}
                  disabled={memoryLoading || memoryActionLoading !== null || !selectedEmployeeMemoryId}
                  className="h-8 rounded border border-indigo-200 hover:bg-indigo-100 disabled:bg-gray-100 text-indigo-700 text-xs"
                >
                  刷新统计
                </button>
                <button
                  type="button"
                  data-testid="employee-memory-export"
                  onClick={exportEmployeeMemory}
                  disabled={memoryLoading || memoryActionLoading !== null || !selectedEmployeeMemoryId}
                  className="h-8 rounded border border-indigo-200 hover:bg-indigo-100 disabled:bg-gray-100 text-indigo-700 text-xs"
                >
                  {memoryActionLoading === "export" ? "导出中..." : "导出 JSON"}
                </button>
                <button
                  type="button"
                  data-testid="employee-memory-clear"
                  onClick={() => setPendingClearMemory(true)}
                  disabled={memoryLoading || memoryActionLoading !== null || !selectedEmployeeMemoryId}
                  className="h-8 rounded border border-red-200 hover:bg-red-50 disabled:bg-gray-100 text-red-600 text-xs"
                >
                  清空记忆
                </button>
              </div>
              <div className="text-xs text-indigo-800 flex items-center gap-4">
                <span data-testid="employee-memory-total-files">文件数：{memoryStats?.total_files ?? 0}</span>
                <span data-testid="employee-memory-total-bytes">大小：{memoryStats?.total_bytes ?? 0}</span>
                <span>({formatBytes(memoryStats?.total_bytes ?? 0)})</span>
              </div>
              <div className="max-h-32 overflow-y-auto rounded border border-indigo-100 bg-white p-2 space-y-1">
                {(memoryStats?.skills || []).length === 0 ? (
                  <div className="text-[11px] text-gray-500">暂无长期记忆文件</div>
                ) : (
                  (memoryStats?.skills || []).map((item) => (
                    <div
                      key={item.skill_id}
                      data-testid={`employee-memory-skill-${item.skill_id}`}
                      className="text-[11px] text-gray-700 flex items-center justify-between"
                    >
                      <span>{item.skill_id}</span>
                      <span>
                        {item.total_files} 文件 / {formatBytes(item.total_bytes)}
                      </span>
                    </div>
                  ))
                )}
              </div>
            </div>
          </div>
        </div>
      </div>
      <RiskConfirmDialog
        open={pendingClearMemory}
        level="high"
        title="清空长期记忆"
        summary={clearMemoryDialogSummary}
        impact={clearMemoryDialogImpact}
        irreversible
        confirmLabel="确认清空"
        cancelLabel="取消"
        loading={memoryActionLoading === "clear"}
        onConfirm={confirmClearEmployeeMemory}
        onCancel={() => setPendingClearMemory(false)}
      />
      <RiskConfirmDialog
        open={Boolean(pendingDeleteEmployee)}
        level="high"
        title="删除员工"
        summary={deleteDialogSummary}
        impact={deleteDialogImpact}
        irreversible
        confirmLabel="确认删除"
        cancelLabel="取消"
        loading={saving}
        onConfirm={confirmRemoveCurrent}
        onCancel={cancelRemoveCurrent}
      />
    </div>
  );
}
