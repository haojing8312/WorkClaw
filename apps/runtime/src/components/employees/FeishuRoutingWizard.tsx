import { useMemo, useState } from "react";
import {
  ImRouteSimulationPayload,
  ImRoutingBinding,
  UpsertImRoutingBindingInput,
} from "../../types";

interface Props {
  bindings: ImRoutingBinding[];
  onSaveRule: (input: UpsertImRoutingBindingInput) => Promise<void>;
  onDeleteRule: (id: string) => Promise<void>;
  onSimulate: (payload: ImRouteSimulationPayload) => Promise<unknown>;
}

const blankRule: UpsertImRoutingBindingInput = {
  id: undefined,
  agent_id: "",
  channel: "feishu",
  account_id: "*",
  peer_kind: "group",
  peer_id: "",
  guild_id: "",
  team_id: "",
  role_ids: [],
  connector_meta: {},
  priority: 100,
  enabled: true,
};

function toOpenclawBinding(input: UpsertImRoutingBindingInput) {
  const match: {
    channel: string;
    accountId?: string;
    peer?: { kind: "group" | "channel" | "direct"; id: string };
    guildId?: string;
    teamId?: string;
    roles?: string[];
  } = {
    channel: input.channel || "feishu",
  };
  if (input.account_id.trim()) {
    match.accountId = input.account_id.trim();
  }
  if (input.peer_kind.trim() && input.peer_id.trim()) {
    match.peer = {
      kind: input.peer_kind.trim().toLowerCase() as "group" | "channel" | "direct",
      id: input.peer_id.trim(),
    };
  }
  if (input.guild_id.trim()) {
    match.guildId = input.guild_id.trim();
  }
  if (input.team_id.trim()) {
    match.teamId = input.team_id.trim();
  }
  if (input.role_ids.length > 0) {
    match.roles = input.role_ids;
  }
  return {
    agentId: input.agent_id.trim(),
    match,
  };
}

export function FeishuRoutingWizard({ bindings, onSaveRule, onDeleteRule, onSimulate }: Props) {
  const [rule, setRule] = useState<UpsertImRoutingBindingInput>(blankRule);
  const [simulateAccountId, setSimulateAccountId] = useState("tenant-a");
  const [simulatePeerId, setSimulatePeerId] = useState("chat-1");
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);

  const sortedBindings = useMemo(
    () => [...bindings].sort((a, b) => a.priority - b.priority),
    [bindings],
  );

  async function handleSave() {
    if (!rule.agent_id.trim()) {
      setMessage("请先填写 agent_id");
      return;
    }
    setSaving(true);
    setMessage("");
    try {
      await onSaveRule({
        ...rule,
        agent_id: rule.agent_id.trim(),
        channel: "feishu",
        account_id: rule.account_id.trim() || "*",
        peer_kind: rule.peer_kind.trim().toLowerCase(),
        peer_id: rule.peer_id.trim(),
        role_ids: rule.role_ids.map((x) => x.trim()).filter(Boolean),
        connector_meta: {
          ...(rule.connector_meta || {}),
          connector_id: "feishu",
        },
      });
      setRule(blankRule);
      setMessage("路由规则已保存");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleSimulate() {
    setSaving(true);
    setMessage("");
    try {
      const payload: ImRouteSimulationPayload = {
        channel: "feishu",
        account_id: simulateAccountId.trim(),
        peer: {
          kind: "group",
          id: simulatePeerId.trim(),
        },
        default_agent_id: "main",
        bindings: [
          ...sortedBindings.map((item) => toOpenclawBinding({
            id: item.id,
            agent_id: item.agent_id,
            channel: item.channel,
            account_id: item.account_id,
            peer_kind: item.peer_kind,
            peer_id: item.peer_id,
            guild_id: item.guild_id,
            team_id: item.team_id,
            role_ids: item.role_ids,
            connector_meta: item.connector_meta || {},
            priority: item.priority,
            enabled: item.enabled,
          })),
          ...(rule.agent_id.trim() ? [toOpenclawBinding(rule)] : []),
        ],
      };
      const result = await onSimulate(payload);
      setMessage(`模拟完成：${JSON.stringify(result)}`);
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="bg-white rounded-lg p-4 space-y-3">
      <div className="text-xs font-medium text-gray-500">渠道连接器路由向导（当前：飞书）</div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
        <input
          className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
          placeholder="agent_id（如 main）"
          value={rule.agent_id}
          onChange={(e) => setRule((s) => ({ ...s, agent_id: e.target.value }))}
        />
        <input
          className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
          placeholder="account_id（默认 *）"
          value={rule.account_id}
          onChange={(e) => setRule((s) => ({ ...s, account_id: e.target.value }))}
        />
      </div>
      <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
        <select
          className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
          value={rule.peer_kind}
          onChange={(e) => setRule((s) => ({ ...s, peer_kind: e.target.value }))}
        >
          <option value="group">group</option>
          <option value="channel">channel</option>
          <option value="direct">direct</option>
        </select>
        <input
          className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
          placeholder="peer_id（可空）"
          value={rule.peer_id}
          onChange={(e) => setRule((s) => ({ ...s, peer_id: e.target.value }))}
        />
        <input
          className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
          type="number"
          placeholder="priority"
          value={rule.priority}
          onChange={(e) => setRule((s) => ({ ...s, priority: Number(e.target.value || 100) }))}
        />
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
        <input
          className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
          placeholder="模拟 account_id"
          value={simulateAccountId}
          onChange={(e) => setSimulateAccountId(e.target.value)}
        />
        <input
          className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
          placeholder="模拟 peer_id"
          value={simulatePeerId}
          onChange={(e) => setSimulatePeerId(e.target.value)}
        />
      </div>
      <div className="flex gap-2">
        <button
          disabled={saving}
          onClick={handleSave}
          className="bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-sm py-1.5 px-3 rounded-lg"
        >
          保存规则
        </button>
        <button
          disabled={saving}
          onClick={handleSimulate}
          className="bg-gray-100 hover:bg-gray-200 text-sm py-1.5 px-3 rounded-lg"
        >
          模拟路由
        </button>
      </div>
      <div className="text-xs text-gray-600 border border-gray-100 rounded p-2 max-h-36 overflow-y-auto">
        {sortedBindings.length === 0 ? (
          <div className="text-gray-400">暂无规则</div>
        ) : (
          sortedBindings.map((item) => (
            <div key={item.id} className="flex items-center justify-between gap-2 py-1 border-b border-gray-50 last:border-b-0">
              <div>
                <div className="font-medium">{item.agent_id}</div>
                <div className="text-[11px] text-gray-500">
                  channel={item.channel} account={item.account_id || "*"} peer={item.peer_kind || "-"}:{item.peer_id || "-"} priority={item.priority}
                </div>
              </div>
              <button
                onClick={() => onDeleteRule(item.id)}
                className="text-red-600 hover:text-red-700 text-xs"
              >
                删除
              </button>
            </div>
          ))
        )}
      </div>
      {message && <div className="text-xs text-blue-700 bg-blue-50 border border-blue-100 rounded px-2 py-1">{message}</div>}
    </div>
  );
}
