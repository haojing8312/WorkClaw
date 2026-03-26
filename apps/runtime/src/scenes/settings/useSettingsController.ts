import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  listModelConfigs,
  listProviderConfigs,
} from "../../components/settings/models/modelSettingsService";
import type {
  CapabilityRouteTemplateInfo,
  CapabilityRoutingPolicy,
  ModelConfig,
  ProviderConfig,
  ProviderHealthInfo,
  RouteAttemptLog,
  RouteAttemptStat,
} from "../../types";

type RouteLogRow = {
  provider_id: string;
  model: string;
};

function getCapabilityRecommendedDefaults(capability: string): { timeout_ms: number; retry_count: number } {
  switch (capability) {
    case "vision":
      return { timeout_ms: 90000, retry_count: 1 };
    case "image_gen":
      return { timeout_ms: 120000, retry_count: 1 };
    case "audio_stt":
      return { timeout_ms: 90000, retry_count: 1 };
    case "audio_tts":
      return { timeout_ms: 60000, retry_count: 1 };
    default:
      return { timeout_ms: 60000, retry_count: 1 };
  }
}

function parseFallbackRows(fallbackChainJson: string | null | undefined): RouteLogRow[] {
  try {
    const parsed = JSON.parse(fallbackChainJson || "[]");
    return Array.isArray(parsed)
      ? parsed.map((item) => ({
          provider_id: String(item?.provider_id || ""),
          model: String(item?.model || ""),
        }))
      : [];
  } catch {
    return [];
  }
}

function buildDefaultRoutingPolicy(capability: string): CapabilityRoutingPolicy {
  const defaults = getCapabilityRecommendedDefaults(capability);
  return {
    capability,
    primary_provider_id: "",
    primary_model: "",
    fallback_chain_json: "[]",
    timeout_ms: defaults.timeout_ms,
    retry_count: defaults.retry_count,
    enabled: true,
  };
}

export function useSettingsController() {
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [providers, setProviders] = useState<ProviderConfig[]>([]);

  const [selectedCapability, setSelectedCapability] = useState("chat");
  const [chatRoutingPolicy, setChatRoutingPolicy] = useState<CapabilityRoutingPolicy>(
    buildDefaultRoutingPolicy("chat"),
  );
  const [policySaveState, setPolicySaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [policyError, setPolicyError] = useState("");
  const [chatPrimaryModels, setChatPrimaryModels] = useState<string[]>([]);
  const [chatFallbackRows, setChatFallbackRows] = useState<RouteLogRow[]>([]);
  const [routeTemplates, setRouteTemplates] = useState<CapabilityRouteTemplateInfo[]>([]);
  const [selectedRouteTemplateId, setSelectedRouteTemplateId] = useState("china-first-p0");

  const [healthResult, setHealthResult] = useState<ProviderHealthInfo | null>(null);
  const [allHealthResults, setAllHealthResults] = useState<ProviderHealthInfo[]>([]);
  const [healthLoading, setHealthLoading] = useState(false);
  const [healthProviderId, setHealthProviderId] = useState("");
  const [routeLogs, setRouteLogs] = useState<RouteAttemptLog[]>([]);
  const [routeLogsLoading, setRouteLogsLoading] = useState(false);
  const [routeLogsOffset, setRouteLogsOffset] = useState(0);
  const [routeLogsHasMore, setRouteLogsHasMore] = useState(false);
  const [routeLogsSessionId, setRouteLogsSessionId] = useState("");
  const [routeLogsCapabilityFilter, setRouteLogsCapabilityFilter] = useState("all");
  const [routeLogsResultFilter, setRouteLogsResultFilter] = useState("all");
  const [routeLogsErrorKindFilter, setRouteLogsErrorKindFilter] = useState("all");
  const [routeLogsExporting, setRouteLogsExporting] = useState(false);
  const [routeStats, setRouteStats] = useState<RouteAttemptStat[]>([]);
  const [routeStatsLoading, setRouteStatsLoading] = useState(false);
  const [routeStatsCapability, setRouteStatsCapability] = useState("all");
  const [routeStatsHours, setRouteStatsHours] = useState(24);

  useEffect(() => {
    let cancelled = false;

    async function loadInitialData() {
      try {
        const [loadedModels, loadedProviders] = await Promise.all([listModelConfigs(), listProviderConfigs()]);
        if (cancelled) return;
        setModels(loadedModels);
        setProviders(loadedProviders);
      } catch (error) {
        if (!cancelled) {
          console.warn("加载设置初始数据失败:", error);
        }
      }
    }

    void loadInitialData();

    return () => {
      cancelled = true;
    };
  }, []);

  async function loadChatPrimaryModels(providerId: string, capability: string) {
    if (!providerId) {
      setChatPrimaryModels([]);
      return;
    }
    try {
      const models = await invoke<string[]>("list_provider_models", {
        providerId,
        capability,
      });
      setChatPrimaryModels(models);
    } catch {
      setChatPrimaryModels([]);
    }
  }

  async function loadCapabilityRoutingPolicy(capability: string) {
    try {
      const loaded = await invoke<CapabilityRoutingPolicy | null>("get_capability_routing_policy", {
        capability,
      });
      const nextPolicy = loaded ?? buildDefaultRoutingPolicy(capability);
      setChatRoutingPolicy(nextPolicy);
      setChatFallbackRows(parseFallbackRows(nextPolicy.fallback_chain_json));
      void loadChatPrimaryModels(nextPolicy.primary_provider_id, capability);
    } catch {
      setChatRoutingPolicy(buildDefaultRoutingPolicy(capability));
      setChatFallbackRows([]);
      setChatPrimaryModels([]);
    }
  }

  async function loadRouteTemplates(capability: string) {
    try {
      const templates = await invoke<CapabilityRouteTemplateInfo[]>("list_capability_route_templates", {
        capability,
      });
      setRouteTemplates(templates);
      setSelectedRouteTemplateId((current) => {
        if (templates.some((item) => item.template_id === current)) {
          return current;
        }
        return templates[0]?.template_id || "";
      });
    } catch {
      setRouteTemplates([]);
      setSelectedRouteTemplateId("");
    }
  }

  async function handleSaveChatPolicy() {
    setPolicySaveState("saving");
    setPolicyError("");
    try {
      const policyToSave = {
        ...chatRoutingPolicy,
        capability: selectedCapability,
        fallback_chain_json: JSON.stringify(chatFallbackRows),
      };
      await invoke("set_capability_routing_policy", { policy: policyToSave });
      setPolicySaveState("saved");
      setTimeout(() => setPolicySaveState("idle"), 1200);
    } catch (error) {
      setPolicySaveState("error");
      setPolicyError("保存聊天路由策略失败: " + String(error));
    }
  }

  async function handleCheckProviderHealth() {
    if (!healthProviderId) return;
    setHealthLoading(true);
    try {
      const result = await invoke<ProviderHealthInfo>("test_provider_health", {
        providerId: healthProviderId,
      });
      setHealthResult(result);
    } catch (error) {
      setHealthResult({
        provider_id: healthProviderId,
        ok: false,
        protocol_type: "",
        message: String(error),
      });
    } finally {
      setHealthLoading(false);
    }
  }

  async function handleCheckAllProviderHealth() {
    setHealthLoading(true);
    try {
      const results = await invoke<ProviderHealthInfo[]>("test_all_provider_health");
      setAllHealthResults(results);
      if (results.length > 0) {
        setHealthResult(results[0]);
      }
    } catch (error) {
      setAllHealthResults([
        {
          provider_id: "",
          ok: false,
          protocol_type: "",
          message: String(error),
        },
      ]);
    } finally {
      setHealthLoading(false);
    }
  }

  async function loadRecentRouteLogs(append: boolean) {
    setRouteLogsLoading(true);
    try {
      const logs = await invoke<RouteAttemptLog[]>("list_recent_route_attempt_logs", {
        sessionId: routeLogsSessionId.trim() || null,
        limit: 50,
        offset: append ? routeLogsOffset : 0,
      });
      setRouteLogs((prev) => (append ? [...prev, ...logs] : logs));
      setRouteLogsOffset((prev) => (append ? prev + logs.length : logs.length));
      setRouteLogsHasMore(logs.length === 50);
    } catch {
      if (!append) {
        setRouteLogs([]);
        setRouteLogsOffset(0);
        setRouteLogsHasMore(false);
      }
    } finally {
      setRouteLogsLoading(false);
    }
  }

  async function loadRouteStats() {
    setRouteStatsLoading(true);
    try {
      const stats = await invoke<RouteAttemptStat[]>("list_route_attempt_stats", {
        hours: routeStatsHours,
        capability: routeStatsCapability === "all" ? null : routeStatsCapability,
      });
      setRouteStats(stats);
    } catch {
      setRouteStats([]);
    } finally {
      setRouteStatsLoading(false);
    }
  }

  async function handleExportRouteLogsCsv() {
    setRouteLogsExporting(true);
    try {
      const csv = await invoke<string>("export_route_attempt_logs_csv", {
        sessionId: routeLogsSessionId.trim() || null,
        hours: routeStatsHours,
        capability: routeLogsCapabilityFilter === "all" ? null : routeLogsCapabilityFilter,
        resultFilter: routeLogsResultFilter === "all" ? null : routeLogsResultFilter,
        errorKind: routeLogsErrorKindFilter === "all" ? null : routeLogsErrorKindFilter,
      });
      const dir = await invoke<string | null>("select_directory", { defaultPath: "" });
      if (dir) {
        const stamp = new Date().toISOString().replace(/:/g, "-").replace(/\..+/, "");
        const path = `${dir}\\route-attempt-logs-${stamp}.csv`;
        await invoke("write_export_file", { path, content: csv });
      }
      if (navigator?.clipboard?.writeText) {
        await navigator.clipboard.writeText(csv);
      }
    } finally {
      setRouteLogsExporting(false);
    }
  }

  function addFallbackRow() {
    setChatFallbackRows((rows) => [...rows, { provider_id: "", model: "" }]);
  }

  function updateFallbackRow(index: number, patch: Partial<RouteLogRow>) {
    setChatFallbackRows((rows) => rows.map((row, i) => (i === index ? { ...row, ...patch } : row)));
  }

  function removeFallbackRow(index: number) {
    setChatFallbackRows((rows) => rows.filter((_, i) => i !== index));
  }

  async function handleApplyRouteTemplate() {
    try {
      const policy = await invoke<CapabilityRoutingPolicy>("apply_capability_route_template", {
        capability: selectedCapability,
        templateId: selectedRouteTemplateId,
      });
      setChatRoutingPolicy(policy);
      setChatFallbackRows(parseFallbackRows(policy.fallback_chain_json));
    } catch (error) {
      const raw = String(error);
      const enabledKeys = Array.from(new Set(providers.filter((p) => p.enabled).map((p) => p.provider_key)));
      const enabledText = enabledKeys.length > 0 ? enabledKeys.join(", ") : "无";
      let missingText = "";
      const match = raw.match(/需要其一）:\s*(\[[^\]]+\])/);
      if (match?.[1]) {
        missingText = `；缺少服务标识（任选其一）: ${match[1]}`;
      }
      setPolicyError(`应用路由模板失败: ${raw}${missingText}；当前已启用: ${enabledText}。请先到“模型连接”补齐并启用。`);
    }
  }

  const filteredRouteLogs = routeLogs.filter((log) => {
    if (routeLogsCapabilityFilter !== "all" && log.capability !== routeLogsCapabilityFilter) return false;
    if (routeLogsResultFilter === "success" && !log.success) return false;
    if (routeLogsResultFilter === "failed" && log.success) return false;
    if (routeLogsErrorKindFilter !== "all" && log.error_kind !== routeLogsErrorKindFilter) return false;
    return true;
  });

  return {
    models,
    setModels,
    providers,
    setProviders,
    selectedCapability,
    setSelectedCapability,
    chatRoutingPolicy,
    setChatRoutingPolicy,
    policySaveState,
    policyError,
    chatPrimaryModels,
    setChatPrimaryModels,
    chatFallbackRows,
    routeTemplates,
    selectedRouteTemplateId,
    setSelectedRouteTemplateId,
    healthResult,
    allHealthResults,
    healthLoading,
    healthProviderId,
    setHealthProviderId,
    routeLogs,
    routeLogsLoading,
    routeLogsOffset,
    setRouteLogsOffset,
    routeLogsHasMore,
    routeLogsSessionId,
    setRouteLogsSessionId,
    routeLogsCapabilityFilter,
    setRouteLogsCapabilityFilter,
    routeLogsResultFilter,
    setRouteLogsResultFilter,
    routeLogsErrorKindFilter,
    setRouteLogsErrorKindFilter,
    routeLogsExporting,
    routeStats,
    routeStatsLoading,
    routeStatsCapability,
    setRouteStatsCapability,
    routeStatsHours,
    setRouteStatsHours,
    filteredRouteLogs,
    getCapabilityRecommendedDefaults,
    loadChatPrimaryModels,
    loadCapabilityRoutingPolicy,
    loadRouteTemplates,
    handleSaveChatPolicy,
    handleCheckProviderHealth,
    handleCheckAllProviderHealth,
    loadRecentRouteLogs,
    loadRouteStats,
    handleExportRouteLogsCsv,
    addFallbackRow,
    updateFallbackRow,
    removeFallbackRow,
    handleApplyRouteTemplate,
  };
}
