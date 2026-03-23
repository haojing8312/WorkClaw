import { useEffect, useState } from "react";
import { MCP_PRESETS, addMcpServer, listMcpServers, parseMcpEnvJson, removeMcpServer, type McpFormState, type McpServerRecord } from "./mcpSettingsService";

const EMPTY_MCP_FORM: McpFormState = { name: "", command: "", args: "", env: "" };

export function McpSettingsSection() {
  const [mcpServers, setMcpServers] = useState<McpServerRecord[]>([]);
  const [mcpForm, setMcpForm] = useState(EMPTY_MCP_FORM);
  const [mcpError, setMcpError] = useState("");
  const [showMcpEnvJson, setShowMcpEnvJson] = useState(false);

  async function refreshMcpServers() {
    try {
      const list = await listMcpServers();
      setMcpServers((current) => (current.length === 0 && list.length === 0 ? current : list));
    } catch (cause) {
      console.error("加载 MCP 服务器失败:", cause);
    }
  }

  useEffect(() => {
    let cancelled = false;

    async function loadInitialMcpServers() {
      try {
        const list = await listMcpServers();
        if (!cancelled) {
          setMcpServers((current) => (current.length === 0 && list.length === 0 ? current : list));
        }
      } catch (cause) {
        console.error("加载 MCP 服务器失败:", cause);
      }
    }

    void loadInitialMcpServers();
    return () => {
      cancelled = true;
    };
  }, []);

  function applyMcpPreset(value: string) {
    const preset = MCP_PRESETS.find((item) => item.value === value);
    if (!preset || !preset.value) return;
    setShowMcpEnvJson(false);
    setMcpForm({
      name: preset.name,
      command: preset.command,
      args: preset.args,
      env: preset.env,
    });
  }

  function updateMcpEnvField(envKey: string, value: string) {
    const parsed = parseMcpEnvJson(mcpForm.env);
    const next = { ...parsed.env, [envKey]: value };
    setMcpForm((current) => ({ ...current, env: JSON.stringify(next) }));
  }

  async function handleAddMcp() {
    setMcpError("");
    try {
      await addMcpServer(mcpForm);
      setMcpForm(EMPTY_MCP_FORM);
      setShowMcpEnvJson(false);
      await refreshMcpServers();
    } catch (cause) {
      setMcpError(String(cause));
    }
  }

  async function handleRemoveMcp(id: string) {
    await removeMcpServer(id);
    await refreshMcpServers();
  }

  const parsedMcpEnv = parseMcpEnvJson(mcpForm.env);
  const mcpApiKeyEnvKeys = Object.keys(parsedMcpEnv.env).filter((key) => key.toUpperCase().includes("API_KEY"));
  const inputCls = "sm-input w-full text-sm py-1.5";
  const labelCls = "sm-field-label";

  return (
    <div className="bg-white rounded-lg p-4 space-y-3">
      <div className="text-xs font-medium text-gray-500 mb-2">MCP 服务器</div>

      {mcpServers.length > 0 && (
        <div className="space-y-2 mb-3">
          {mcpServers.map((server) => (
            <div key={server.id} className="flex items-center justify-between bg-gray-100 rounded px-3 py-2 text-sm">
              <div>
                <span className="font-medium">{server.name}</span>
                <span className="text-gray-500 ml-2 text-xs">{server.command} {server.args?.join(" ")}</span>
              </div>
              <button onClick={() => void handleRemoveMcp(server.id)} className="text-red-400 hover:text-red-300 text-xs">
                删除
              </button>
            </div>
          ))}
        </div>
      )}

      <div>
        <label className={labelCls}>快速选择 MCP 服务器</label>
        <select className={inputCls} defaultValue="" onChange={(event) => applyMcpPreset(event.target.value)}>
          {MCP_PRESETS.map((preset) => (
            <option key={preset.value} value={preset.value}>
              {preset.label}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label className={labelCls}>名称</label>
        <input className={inputCls} placeholder="例: filesystem" value={mcpForm.name} onChange={(event) => setMcpForm({ ...mcpForm, name: event.target.value })} />
      </div>
      <div>
        <label className={labelCls}>命令</label>
        <input className={inputCls} placeholder="例: npx" value={mcpForm.command} onChange={(event) => setMcpForm({ ...mcpForm, command: event.target.value })} />
      </div>
      <div>
        <label className={labelCls}>参数（空格分隔）</label>
        <input className={inputCls} placeholder="例: @anthropic/mcp-server-filesystem /tmp" value={mcpForm.args} onChange={(event) => setMcpForm({ ...mcpForm, args: event.target.value })} />
      </div>
      {mcpApiKeyEnvKeys.map((envKey) => (
        <div key={envKey}>
          <label className={labelCls}>API Key（可选）</label>
          <input
            className={inputCls}
            type="password"
            placeholder={`请输入 ${envKey}`}
            value={parsedMcpEnv.env[envKey] || ""}
            onChange={(event) => updateMcpEnvField(envKey, event.target.value)}
          />
          <div className="text-[11px] text-gray-400 mt-1">变量名：{envKey}</div>
        </div>
      ))}
      <div className="space-y-2">
        <button
          type="button"
          onClick={() => setShowMcpEnvJson((value) => !value)}
          className="text-xs text-blue-500 hover:text-blue-600"
        >
          {showMcpEnvJson ? "收起高级 JSON 配置" : "高级：环境变量 JSON 配置"}
        </button>
        {showMcpEnvJson && (
          <div>
            <label className={labelCls}>环境变量（JSON 格式，可选）</label>
            <input
              className={inputCls}
              placeholder='例: {"API_KEY": "xxx"}'
              value={mcpForm.env}
              onChange={(event) => setMcpForm({ ...mcpForm, env: event.target.value })}
            />
            {parsedMcpEnv.error && <div className="text-[11px] text-red-500 mt-1">{parsedMcpEnv.error}</div>}
          </div>
        )}
      </div>
      {mcpError && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{mcpError}</div>}
      <button
        onClick={() => void handleAddMcp()}
        disabled={!mcpForm.name || !mcpForm.command}
        className="w-full bg-blue-500 hover:bg-blue-600 disabled:bg-gray-200 disabled:text-gray-400 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
      >
        添加 MCP 服务器
      </button>
    </div>
  );
}
