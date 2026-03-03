import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AgentEmployee, RuntimePreferences, SkillManifest, UpsertAgentEmployeeInput } from "../../types";
import { RiskConfirmDialog } from "../RiskConfirmDialog";

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
  name: "",
  role_id: "",
  persona: "",
  feishu_open_id: "",
  feishu_app_id: "",
  feishu_app_secret: "",
  primary_skill_id: "",
  default_work_dir: "",
  enabled: true,
  is_default: false,
  skill_ids: [],
};

const roleTemplates: Array<{ name: string; roleId: string; persona: string }> = [
  {
    name: "项目经理",
    roleId: "project_manager",
    persona: "负责需求澄清、任务拆解、里程碑推进与风险管理，优先输出可执行计划与验收标准。",
  },
  {
    name: "技术负责人",
    roleId: "tech_lead",
    persona: "负责技术方案评审、架构决策和质量把关，强调可维护性、测试覆盖和交付稳定性。",
  },
  {
    name: "运营专员",
    roleId: "operations",
    persona: "负责运营数据分析、活动复盘与流程优化，输出可落地行动项和指标跟踪方案。",
  },
  {
    name: "客服专员",
    roleId: "customer_success",
    persona: "负责用户问题分级、解决路径设计与满意度提升，提供清晰且可执行的处理建议。",
  },
];

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
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [globalDefaultWorkDir, setGlobalDefaultWorkDir] = useState("");
  const [savingGlobalWorkDir, setSavingGlobalWorkDir] = useState(false);
  const [pendingDeleteEmployee, setPendingDeleteEmployee] = useState<{ id: string; name: string } | null>(null);

  const skillOptions = useMemo(
    () => skills.filter((s) => s.id !== "builtin-general"),
    [skills],
  );

  useEffect(() => {
    (async () => {
      try {
        const prefs = await invoke<RuntimePreferences>("get_runtime_preferences");
        setGlobalDefaultWorkDir(prefs.default_work_dir || "");
      } catch {
        // ignore: settings panel remains editable manually
      }
    })();
  }, []);

  function pickEmployee(id: string) {
    onSelectEmployee(id);
    const e = employees.find((x) => x.id === id);
    if (!e) return;
    setForm({
      id: e.id,
      name: e.name,
      role_id: e.role_id,
      persona: e.persona,
      feishu_open_id: e.feishu_open_id,
      feishu_app_id: e.feishu_app_id,
      feishu_app_secret: e.feishu_app_secret,
      primary_skill_id: e.primary_skill_id || "",
      default_work_dir: e.default_work_dir || "",
      enabled: e.enabled,
      is_default: e.is_default,
      skill_ids: e.is_default
        ? []
        : (e.skill_ids.length > 0 ? e.skill_ids : []),
    });
  }

  function resetForm() {
    setForm(blankForm);
    setMessage("");
  }

  async function save() {
    setSaving(true);
    setMessage("");
    try {
      await onSaveEmployee({
        ...form,
        skill_ids: form.is_default ? [] : form.skill_ids,
      });
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

  function applyRoleTemplate(roleId: string) {
    const tpl = roleTemplates.find((x) => x.roleId === roleId);
    if (!tpl) return;
    setForm((s) => ({
      ...s,
      role_id: tpl.roleId,
      persona: tpl.persona,
    }));
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
            员工独立于飞书。每个员工可绑定多技能和独立飞书机器人配置；主员工默认进入且拥有全技能权限。
          </p>
        </div>

        <div className="bg-white border border-gray-200 rounded-xl p-4 space-y-2">
          <div className="text-xs text-gray-500">全局默认工作目录（新建会话默认使用）</div>
          <input
            className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
            placeholder="例如 D:\\workspace\\skillmint"
            value={globalDefaultWorkDir}
            onChange={(e) => setGlobalDefaultWorkDir(e.target.value)}
          />
          <div className="text-[11px] text-gray-500">
            默认：C:\Users\&lt;用户名&gt;\SkillMint\workspace。支持 C/D/E 盘路径，目录不存在会自动创建。
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
                  <div className="text-gray-500 truncate">{e.role_id}</div>
                </button>
              ))}
            </div>
          </div>

          <div className="md:col-span-2 bg-white border border-gray-200 rounded-xl p-4 space-y-2">
            <div className="text-xs text-gray-500">员工配置</div>
            <input
              className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
              placeholder="员工名称"
              value={form.name}
              onChange={(e) => setForm((s) => ({ ...s, name: e.target.value }))}
            />
            <input
              className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
              placeholder="角色ID（如 project_manager）"
              value={form.role_id}
              onChange={(e) => setForm((s) => ({ ...s, role_id: e.target.value }))}
            />
            <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
              {roleTemplates.map((tpl) => (
                <button
                  key={tpl.roleId}
                  type="button"
                  onClick={() => applyRoleTemplate(tpl.roleId)}
                  className="h-8 rounded border border-gray-200 hover:border-blue-300 hover:bg-blue-50 text-xs text-gray-700"
                >
                  填充{tpl.name}
                </button>
              ))}
            </div>
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
            <textarea
              className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
              rows={2}
              placeholder="角色人格/职责描述"
              value={form.persona}
              onChange={(e) => setForm((s) => ({ ...s, persona: e.target.value }))}
            />
            <input
              className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
              placeholder="飞书机器人 open_id（可空，仅用于飞书@精准路由）"
              value={form.feishu_open_id}
              onChange={(e) => setForm((s) => ({ ...s, feishu_open_id: e.target.value }))}
            />
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              <input
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                placeholder="员工飞书 app_id（可空）"
                value={form.feishu_app_id}
                onChange={(e) => setForm((s) => ({ ...s, feishu_app_id: e.target.value }))}
              />
              <input
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                type="password"
                placeholder="员工飞书 app_secret（可空）"
                value={form.feishu_app_secret}
                onChange={(e) => setForm((s) => ({ ...s, feishu_app_secret: e.target.value }))}
              />
            </div>

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

            {message && <div className="text-xs text-blue-700 bg-blue-50 border border-blue-100 rounded px-2 py-1">{message}</div>}

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
