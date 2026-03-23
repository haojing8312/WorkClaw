import { useEffect, useState } from "react";
import { loadRoutingSettings, saveRoutingSettings, type RoutingSettings } from "./routingSettingsService";

const DEFAULT_ROUTING_SETTINGS: RoutingSettings = {
  max_call_depth: 4,
  node_timeout_seconds: 60,
  retry_count: 0,
};

export function RoutingSettingsSection() {
  const [routeSettings, setRouteSettings] = useState<RoutingSettings>(DEFAULT_ROUTING_SETTINGS);
  const [routeSaveState, setRouteSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [routeError, setRouteError] = useState("");
  const maxDepthId = "routing-max-call-depth";
  const timeoutId = "routing-node-timeout";
  const retryId = "routing-retry-count";

  useEffect(() => {
    if (routeSaveState !== "saved") {
      return;
    }
    const timer = window.setTimeout(() => setRouteSaveState("idle"), 1200);
    return () => window.clearTimeout(timer);
  }, [routeSaveState]);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const settings = await loadRoutingSettings();
        if (!cancelled) {
          setRouteSettings(settings);
        }
      } catch (cause) {
        if (!cancelled) {
          setRouteError("加载自动路由设置失败: " + String(cause));
          setRouteSaveState("error");
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSaveRoutingSettings() {
    setRouteSaveState("saving");
    setRouteError("");
    try {
      await saveRoutingSettings(routeSettings);
      setRouteSaveState("saved");
    } catch (cause) {
      setRouteError("保存自动路由设置失败: " + String(cause));
      setRouteSaveState("error");
    }
  }

  const inputCls = "w-full rounded-lg border border-gray-200 px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-400";
  const labelCls = "block text-sm font-medium text-gray-700 mb-1";

  return (
    <div className="bg-white rounded-lg p-4 space-y-3">
      <div className="text-xs font-medium text-gray-500 mb-2">子 Skill 自动路由</div>
      <div>
        <label className={labelCls} htmlFor={maxDepthId}>
          最大调用深度 (2-8)
        </label>
        <input
          id={maxDepthId}
          className={inputCls}
          type="number"
          min={2}
          max={8}
          value={routeSettings.max_call_depth}
          onChange={(event) => setRouteSettings((current) => ({ ...current, max_call_depth: Number(event.target.value || 4) }))}
        />
      </div>
      <div>
        <label className={labelCls} htmlFor={timeoutId}>
          节点超时秒数 (5-600)
        </label>
        <input
          id={timeoutId}
          className={inputCls}
          type="number"
          min={5}
          max={600}
          value={routeSettings.node_timeout_seconds}
          onChange={(event) =>
            setRouteSettings((current) => ({ ...current, node_timeout_seconds: Number(event.target.value || 60) }))
          }
        />
      </div>
      <div>
        <label className={labelCls} htmlFor={retryId}>
          失败重试次数 (0-2)
        </label>
        <input
          id={retryId}
          className={inputCls}
          type="number"
          min={0}
          max={2}
          value={routeSettings.retry_count}
          onChange={(event) => setRouteSettings((current) => ({ ...current, retry_count: Number(event.target.value || 0) }))}
        />
      </div>
      {routeError && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{routeError}</div>}
      {routeSaveState === "saved" && <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">已保存</div>}
      <button
        onClick={() => void handleSaveRoutingSettings()}
        disabled={routeSaveState === "saving"}
        className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
      >
        {routeSaveState === "saving" ? "保存中..." : "保存自动路由设置"}
      </button>
    </div>
  );
}
