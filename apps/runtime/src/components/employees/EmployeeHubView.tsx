import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AgentEmployee, RuntimePreferences, SkillManifest, UpsertAgentEmployeeInput } from "../../types";
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

export function EmployeeHubView({
  employees,
  skills,
  selectedEmployeeId,
  onSelectEmployee,
  onSaveEmployee,
  onDeleteEmployee,
  onSetAsMainAndEnter,
}: Props) {
  const [form, setForm] = useState<UpsertAgentEmployeeInput>(blankForm);
  const [employeeIdEdited, setEmployeeIdEdited] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [globalDefaultWorkDir, setGlobalDefaultWorkDir] = useState("");
  const [savingGlobalWorkDir, setSavingGlobalWorkDir] = useState(false);
  const [pendingDeleteEmployee, setPendingDeleteEmployee] = useState<{ id: string; name: string } | null>(null);

  const skillOptions = useMemo(
    () => skills.filter((s) => s.id !== "builtin-general"),
    [skills],
  );
  const selectedEmployee = useMemo(
    () => employees.find((item) => item.id === selectedEmployeeId) ?? null,
    [employees, selectedEmployeeId],
  );

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

  const deleteDialogSummary = pendingDeleteEmployee
    ? `确定删除员工「${pendingDeleteEmployee.name}」吗？`
    : "确定删除该员工吗？";
  const deleteDialogImpact = pendingDeleteEmployee
    ? `员工ID: ${pendingDeleteEmployee.id}`
    : undefined;

  return (
    <div className="h-full overflow-y-auto bg-gray-50">
      <div className="max-w-6xl mx-auto px-8 pt-10 pb-12 space-y-4">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900">智能体员工</h1>
          <p className="text-sm text-gray-600 mt-2">
            用员工编号统一管理 OpenClaw 与飞书路由。主员工默认进入且拥有全技能权限。
          </p>
        </div>

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

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="bg-white border border-gray-200 rounded-xl p-3 max-h-[640px] overflow-y-auto">
            <div className="text-xs text-gray-500 mb-2">员工列表</div>
            <button
              onClick={resetForm}
              className="w-full mb-2 h-8 rounded bg-blue-50 hover:bg-blue-100 text-blue-700 text-xs"
            >
              新建员工
            </button>
            <div className="space-y-2">
              {employees.map((e) => (
                <button
                  key={e.id}
                  onClick={() => pickEmployee(e.id)}
                  className={
                    "w-full text-left border rounded p-2 text-xs " +
                    (selectedEmployeeId === e.id ? "border-blue-300 bg-blue-50" : "border-gray-200 bg-white")
                  }
                >
                  <div className="font-medium text-gray-800 truncate">
                    {e.name} {e.is_default ? "· 主员工" : ""}
                  </div>
                  <div className="text-gray-500 truncate">{e.employee_id || e.role_id}</div>
                </button>
              ))}
            </div>
          </div>

          <div className="md:col-span-2 bg-white border border-gray-200 rounded-xl p-4 space-y-3">
            <div className="text-xs text-gray-500">员工配置</div>

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
            </div>

            <div className="rounded-lg border border-gray-200 p-3 space-y-2">
              <div className="text-xs font-medium text-gray-700">步骤 3 / 3 · 技能与智能体配置</div>
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
              <input
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                placeholder="默认工作目录"
                value={form.default_work_dir}
                onChange={(e) => setForm((s) => ({ ...s, default_work_dir: e.target.value }))}
              />
              <input
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                type="number"
                placeholder="路由优先级（默认100）"
                value={form.routing_priority}
                onChange={(e) =>
                  setForm((s) => ({ ...s, routing_priority: Number(e.target.value || 100) }))
                }
              />
              <div className="text-xs text-gray-500 mt-1">技能集合（主员工自动拥有全部技能，无需手动选择）</div>
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
            </div>
          </div>
        </div>
      </div>
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
