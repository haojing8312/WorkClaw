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
  channel: "",
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

const CONNECTOR_LABELS: Record<string, string> = {
  feishu: "飞书",
};

function resolveRuleChannel(channel: string) {
  return channel.trim().toLowerCase();
}

function getRuleChannelLabel(channel: string) {
  const normalizedChannel = resolveRuleChannel(channel);
  if (!normalizedChannel) {
    return "未选择";
  }
  return CONNECTOR_LABELS[normalizedChannel] || normalizedChannel;
}

function getBindingConnectorId(binding: ImRoutingBinding) {
  const connectorId = String(binding.connector_meta?.connector_id || "").trim();
  return connectorId || resolveRuleChannel(binding.channel) || "n/a";
}

function getPeerKindLabel(peerKind: string) {
  const normalizedPeerKind = peerKind.trim().toLowerCase();
  switch (normalizedPeerKind) {
    case "direct":
      return "私聊";
    case "channel":
      return "频道";
    case "group":
    default:
      return "群聊";
  }
}

function describeBinding(binding: ImRoutingBinding) {
  const channelLabel = getRuleChannelLabel(binding.channel);
  const peerKindLabel = getPeerKindLabel(binding.peer_kind);
  return `来自 ${channelLabel} 的${peerKindLabel}消息，交给 ${binding.agent_id} 处理`;
}

function describeBindingScope(binding: ImRoutingBinding) {
  const connectorId = getBindingConnectorId(binding);
  const accountId = binding.account_id.trim() || "*";
  const peerId = binding.peer_id.trim() || "任意会话";
  return `连接器：${connectorId} · 账号：${accountId} · 会话：${peerId}`;
}

type SimulationResultSummary = {
  agentId: string;
  matchedBy: string;
  channel: string;
  reasonLabel: string;
  sourceLabel: string;
  raw: string;
};

function getMatchedByLabel(matchedBy: string) {
  switch (matchedBy.trim().toLowerCase()) {
    case "binding.channel":
      return "渠道规则";
    case "binding.peer":
      return "指定会话规则";
    case "binding.account":
    case "binding.account_id":
      return "账号规则";
    case "default":
      return "默认员工";
    default:
      return "规则匹配";
  }
}

function buildSimulationResultSummary(channel: string, result: unknown): SimulationResultSummary | null {
  if (!result || typeof result !== "object") {
    return null;
  }
  const payload = result as { agentId?: string; matchedBy?: string };
  const agentId = String(payload.agentId || "").trim();
  const matchedBy = String(payload.matchedBy || "").trim();
  if (!agentId && !matchedBy) {
    return null;
  }
  return {
    agentId: agentId || "unknown",
    matchedBy: matchedBy || "unknown",
    channel,
    reasonLabel: getMatchedByLabel(matchedBy),
    sourceLabel: getRuleChannelLabel(channel),
    raw: JSON.stringify(result),
  };
}

function toOpenclawBinding(input: UpsertImRoutingBindingInput) {
  const channel = resolveRuleChannel(input.channel);
  const match: {
    channel: string;
    accountId?: string;
    peer?: { kind: "group" | "channel" | "direct"; id: string };
    guildId?: string;
    teamId?: string;
    roles?: string[];
  } = {
    channel,
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

export function ImRoutingWizard({ bindings, onSaveRule, onDeleteRule, onSimulate }: Props) {
  const [rule, setRule] = useState<UpsertImRoutingBindingInput>(blankRule);
  const [simulateAccountId, setSimulateAccountId] = useState("tenant-a");
  const [simulatePeerId, setSimulatePeerId] = useState("chat-1");
  const [message, setMessage] = useState("");
  const [simulationResult, setSimulationResult] = useState<SimulationResultSummary | null>(null);
  const [showTechnicalDetails, setShowTechnicalDetails] = useState(false);
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
    const channel = resolveRuleChannel(rule.channel);
    if (!channel) {
      setMessage("请先选择路由渠道");
      return;
    }
    setSaving(true);
    setMessage("");
    setSimulationResult(null);
    setShowTechnicalDetails(false);
    try {
      await onSaveRule({
        ...rule,
        agent_id: rule.agent_id.trim(),
        channel,
        account_id: rule.account_id.trim() || "*",
        peer_kind: rule.peer_kind.trim().toLowerCase(),
        peer_id: rule.peer_id.trim(),
        role_ids: rule.role_ids.map((x) => x.trim()).filter(Boolean),
        connector_meta: {
          ...(rule.connector_meta || {}),
          connector_id: channel,
        },
      });
      setRule((current) => ({
        ...blankRule,
        channel: resolveRuleChannel(current.channel),
      }));
      setMessage("路由规则已保存");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleSimulate() {
    const channel = resolveRuleChannel(rule.channel);
    if (!channel) {
      setMessage("请先选择路由渠道");
      return;
    }
    setSaving(true);
    setMessage("");
    setSimulationResult(null);
    setShowTechnicalDetails(false);
    try {
      const payload: ImRouteSimulationPayload = {
        channel,
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
      const summary = buildSimulationResultSummary(channel, result);
      if (summary) {
        setSimulationResult(summary);
      } else {
        setMessage(`模拟完成：${JSON.stringify(result)}`);
      }
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="bg-white rounded-lg p-4 space-y-3">
      <div className="space-y-1">
        <div className="text-sm font-medium text-gray-900">消息处理规则</div>
        <div className="text-xs text-gray-500">设置不同渠道的消息应该交给谁处理。</div>
        <div className="text-[11px] font-medium text-gray-500">当前渠道：{getRuleChannelLabel(rule.channel)}</div>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
        <label className="sr-only" htmlFor="routing-channel-select">路由渠道</label>
        <select
          id="routing-channel-select"
          aria-label="路由渠道"
          className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
          value={rule.channel}
          onChange={(e) => setRule((s) => ({ ...s, channel: e.target.value }))}
        >
          <option value="">请选择渠道连接器</option>
          <option value="feishu">feishu / 飞书</option>
        </select>
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
                <div className="font-medium">{describeBinding(item)}</div>
                <div className="text-[11px] text-gray-500">
                  {describeBindingScope(item)}
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
      {simulationResult && (
        <div className="rounded border border-blue-100 bg-blue-50 px-3 py-2 space-y-2 text-xs text-blue-900">
          <div>将由：{simulationResult.agentId}</div>
          <div>命中原因：{simulationResult.reasonLabel}</div>
          <div>规则来源：{simulationResult.sourceLabel}</div>
          <button
            type="button"
            className="text-blue-700 hover:text-blue-800 underline underline-offset-2"
            onClick={() => setShowTechnicalDetails((value) => !value)}
          >
            {showTechnicalDetails ? "隐藏技术详情" : "查看技术详情"}
          </button>
          {showTechnicalDetails && (
            <div className="rounded border border-blue-200 bg-white px-2 py-2 space-y-1 text-[11px] text-gray-700">
              <div>matchedBy: {simulationResult.matchedBy}</div>
              <div>channel: {simulationResult.channel}</div>
              <div>raw: {simulationResult.raw}</div>
            </div>
          )}
        </div>
      )}
      {message && <div className="text-xs text-blue-700 bg-blue-50 border border-blue-100 rounded px-2 py-1">{message}</div>}
    </div>
  );
}
