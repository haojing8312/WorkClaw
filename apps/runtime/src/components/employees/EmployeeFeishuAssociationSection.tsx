import { useEffect, useMemo, useState } from "react";
import { AgentEmployee, ImRoutingBinding } from "../../types";

interface SavePayload {
  enabled: boolean;
  mode: "default" | "scoped";
  peerKind: "group" | "channel" | "direct";
  peerId: string;
  priority: number;
}

interface Props {
  employee: AgentEmployee;
  employees: AgentEmployee[];
  bindings: ImRoutingBinding[];
  saving: boolean;
  runtimeStatus: {
    reconnect_attempts?: number;
    queued_events?: number;
    last_event_at?: string | null;
    last_error?: string | null;
  } | null;
  onSave: (payload: SavePayload) => Promise<void>;
  onOpenFeishuSettings?: () => void;
}

function getAgentId(employee: AgentEmployee) {
  return (employee.openclaw_agent_id || employee.employee_id || employee.role_id || "").trim();
}

function getEmployeeNameByAgentId(employees: AgentEmployee[]) {
  const map = new Map<string, string>();
  for (const employee of employees) {
    const agentId = getAgentId(employee).toLowerCase();
    if (!agentId) continue;
    map.set(agentId, employee.name || agentId);
  }
  return map;
}

export function EmployeeFeishuAssociationSection({
  employee,
  employees,
  bindings,
  saving,
  runtimeStatus,
  onSave,
  onOpenFeishuSettings,
}: Props) {
  const employeeAgentId = getAgentId(employee);
  const employeeBindings = useMemo(
    () =>
      bindings.filter(
        (binding) =>
          binding.channel === "feishu" &&
          binding.agent_id.trim().toLowerCase() === employeeAgentId.toLowerCase(),
      ),
    [bindings, employeeAgentId],
  );
  const defaultBinding = employeeBindings.find((binding) => !binding.peer_id.trim()) ?? null;
  const scopedBinding = employeeBindings.find((binding) => binding.peer_id.trim()) ?? null;
  const employeeNameByAgentId = useMemo(() => getEmployeeNameByAgentId(employees), [employees]);
  const otherDefaultBinding = useMemo(
    () =>
      bindings.find(
        (binding) =>
          binding.channel === "feishu" &&
          !binding.peer_id.trim() &&
          binding.agent_id.trim().toLowerCase() !== employeeAgentId.toLowerCase(),
      ) ?? null,
    [bindings, employeeAgentId],
  );

  const [enabled, setEnabled] = useState(
    employee.enabled_scopes.includes("feishu") || employeeBindings.length > 0,
  );
  const [mode, setMode] = useState<"default" | "scoped">(scopedBinding ? "scoped" : "default");
  const [peerKind, setPeerKind] = useState<"group" | "channel" | "direct">(
    scopedBinding?.peer_kind === "direct" || scopedBinding?.peer_kind === "channel"
      ? scopedBinding.peer_kind
      : "group",
  );
  const [peerId, setPeerId] = useState(scopedBinding?.peer_id || "");
  const [priority, setPriority] = useState(scopedBinding?.priority ?? defaultBinding?.priority ?? employee.routing_priority ?? 100);

  useEffect(() => {
    setEnabled(employee.enabled_scopes.includes("feishu") || employeeBindings.length > 0);
    setMode(scopedBinding ? "scoped" : "default");
    setPeerKind(
      scopedBinding?.peer_kind === "direct" || scopedBinding?.peer_kind === "channel"
        ? scopedBinding.peer_kind
        : "group",
    );
    setPeerId(scopedBinding?.peer_id || "");
    setPriority(scopedBinding?.priority ?? defaultBinding?.priority ?? employee.routing_priority ?? 100);
  }, [defaultBinding, employee, employeeBindings.length, scopedBinding]);

  const otherDefaultEmployeeName = otherDefaultBinding
    ? employeeNameByAgentId.get(otherDefaultBinding.agent_id.trim().toLowerCase()) || otherDefaultBinding.agent_id
    : "";
  const conflictingScopedBinding = useMemo(() => {
    if (!peerId.trim()) return null;
    return (
      bindings.find(
        (binding) =>
          binding.channel === "feishu" &&
          binding.agent_id.trim().toLowerCase() !== employeeAgentId.toLowerCase() &&
          binding.peer_kind.trim().toLowerCase() === peerKind &&
          binding.peer_id.trim() === peerId.trim(),
      ) ?? null
    );
  }, [bindings, employeeAgentId, peerId, peerKind]);
  const conflictingScopedEmployeeName = conflictingScopedBinding
    ? employeeNameByAgentId.get(conflictingScopedBinding.agent_id.trim().toLowerCase()) ||
      conflictingScopedBinding.agent_id
    : "";

  return (
    <div data-testid="employee-feishu-association" className="rounded-lg border border-blue-200 bg-blue-50/60 p-3 space-y-3">
      <div className="space-y-1">
        <div className="text-sm font-semibold text-gray-900">飞书接待</div>
        <div className="text-xs text-gray-600">
          飞书连接在设置中心统一管理。这里仅决定该员工是否接待飞书入口，以及接待哪些会话。
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-2 text-[11px] text-gray-600">
        <div className="rounded border border-blue-100 bg-white px-2 py-1.5">
          <div className="font-medium text-gray-700">飞书连接</div>
          <div>使用设置中心中的默认飞书连接</div>
        </div>
        <div className="rounded border border-blue-100 bg-white px-2 py-1.5">
          <div className="font-medium text-gray-700">默认接待员工</div>
          <div>{otherDefaultEmployeeName || (defaultBinding ? employee.name : "尚未设置")}</div>
        </div>
        <div className="rounded border border-blue-100 bg-white px-2 py-1.5">
          <div className="font-medium text-gray-700">运行状态</div>
          <div>{runtimeStatus?.last_error ? runtimeStatus.last_error : runtimeStatus ? "监听中" : "未监听"}</div>
        </div>
      </div>

      <label className="flex items-center gap-2 text-sm text-gray-800">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => setEnabled(event.target.checked)}
        />
        启用飞书接待
      </label>

      {enabled && (
        <>
          {mode === "default" && otherDefaultEmployeeName && (
            <div className="rounded border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
              当前默认接待员工是 {otherDefaultEmployeeName}，保存后将替换为当前员工。
            </div>
          )}
          <div className="space-y-2">
            <div className="text-xs font-medium text-gray-700">接待方式</div>
            <label className="flex items-center gap-2 text-sm text-gray-800">
              <input
                type="radio"
                name={`feishu-mode-${employee.id}`}
                checked={mode === "default"}
                onChange={() => setMode("default")}
              />
              设为默认接待员工
            </label>
            <label className="flex items-center gap-2 text-sm text-gray-800">
              <input
                type="radio"
                name={`feishu-mode-${employee.id}`}
                checked={mode === "scoped"}
                onChange={() => setMode("scoped")}
              />
              仅处理指定群聊或会话
            </label>
          </div>

          {mode === "default" ? (
            <div className="rounded border border-blue-100 bg-white px-3 py-2 text-xs text-gray-600">
              未命中任何规则的飞书消息，会回退给默认接待员工处理。
            </div>
          ) : (
            <div className="rounded border border-blue-100 bg-white p-3 space-y-2">
              <div className="text-xs font-medium text-gray-700">指定处理范围</div>
              <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
                <select
                  aria-label="飞书处理范围类型"
                  className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
                  value={peerKind}
                  onChange={(event) => setPeerKind(event.target.value as "group" | "channel" | "direct")}
                >
                  <option value="group">群聊</option>
                  <option value="channel">频道</option>
                  <option value="direct">私聊</option>
                </select>
                <input
                  aria-label="飞书处理范围 ID"
                  className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                  placeholder="群聊或会话 ID"
                  value={peerId}
                  onChange={(event) => setPeerId(event.target.value)}
                />
                <input
                  aria-label="飞书处理优先级"
                  className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                  type="number"
                  value={priority}
                  onChange={(event) => setPriority(Number(event.target.value || 100))}
                />
              </div>
              <div className="text-[11px] text-gray-500">指定范围规则会优先于默认接待员工生效。</div>
              {conflictingScopedEmployeeName && (
                <div className="rounded border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
                  群聊/会话 {peerId.trim()} 当前由 {conflictingScopedEmployeeName} 处理，保存后将改为当前员工接待。
                </div>
              )}
            </div>
          )}
        </>
      )}

      {runtimeStatus && (
        <div className="text-[11px] text-gray-500 flex flex-wrap gap-3">
          <span>重连次数：{runtimeStatus.reconnect_attempts ?? 0}</span>
          <span>队列事件：{runtimeStatus.queued_events ?? 0}</span>
          <span>最后事件：{runtimeStatus.last_event_at?.trim() || "暂无"}</span>
        </div>
      )}

      <div className="flex items-center justify-between gap-2">
        <div className="text-[11px] text-gray-500">
          官方插件未运行或授权未完成时，请前往设置中心中的飞书连接页面处理。
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => onOpenFeishuSettings?.()}
            className="h-8 px-3 rounded border border-blue-200 bg-white text-blue-700 hover:bg-blue-50 text-xs"
          >
            前往飞书设置
          </button>
          <button
            type="button"
            disabled={saving || (enabled && mode === "scoped" && !peerId.trim())}
            onClick={() =>
              onSave({
                enabled,
                mode,
                peerKind,
                peerId,
                priority,
              })
            }
            className="h-8 px-3 rounded bg-blue-600 hover:bg-blue-700 disabled:bg-blue-300 text-white text-xs"
          >
            {saving ? "保存中..." : "保存飞书接待"}
          </button>
        </div>
      </div>
    </div>
  );
}
