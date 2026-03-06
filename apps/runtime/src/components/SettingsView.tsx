import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppUpdater } from "../hooks/useAppUpdater";
import {
  DEFAULT_MODEL_PROVIDER_ID,
  MODEL_PROVIDER_CATALOG,
  buildModelFormFromCatalogItem,
  getModelProviderCatalogItem,
  resolveCatalogItemForConfig,
} from "../model-provider-catalog";
import {
  CapabilityRouteTemplateInfo,
  CapabilityRoutingPolicy,
  ModelConfig,
  ProviderConfig,
  ProviderHealthInfo,
  RuntimePreferences,
  RouteAttemptLog,
  RouteAttemptStat,
} from "../types";

const MCP_PRESETS = [
  { label: "— 快速选择 —", value: "", name: "", command: "", args: "", env: "" },
  { label: "Filesystem", value: "filesystem", name: "filesystem", command: "npx", args: "-y @anthropic/mcp-server-filesystem /tmp", env: "" },
  { label: "Brave Search", value: "brave-search", name: "brave-search", command: "npx", args: "-y @anthropic/mcp-server-brave-search", env: '{"BRAVE_API_KEY": ""}' },
  { label: "Memory", value: "memory", name: "memory", command: "npx", args: "-y @anthropic/mcp-server-memory", env: "" },
  { label: "Puppeteer", value: "puppeteer", name: "puppeteer", command: "npx", args: "-y @anthropic/mcp-server-puppeteer", env: "" },
  { label: "Fetch", value: "fetch", name: "fetch", command: "npx", args: "-y @anthropic/mcp-server-fetch", env: "" },
];

const SEARCH_PRESETS = [
  { label: "— 快速选择 —", value: "", api_format: "", base_url: "", model_name: "" },
  { label: "Brave Search (国际首选)", value: "brave", api_format: "search_brave", base_url: "https://api.search.brave.com", model_name: "" },
  { label: "Tavily (AI 专用)", value: "tavily", api_format: "search_tavily", base_url: "https://api.tavily.com", model_name: "" },
  { label: "秘塔搜索 (中文首选)", value: "metaso", api_format: "search_metaso", base_url: "https://metaso.cn", model_name: "" },
  { label: "博查搜索 (中文 AI)", value: "bocha", api_format: "search_bocha", base_url: "https://api.bochaai.com", model_name: "" },
  { label: "SerpAPI (多引擎)", value: "serpapi", api_format: "search_serpapi", base_url: "https://serpapi.com", model_name: "google" },
];

function parseMcpEnvJson(text: string): { env: Record<string, string>; error: string | null } {
  if (!text.trim()) {
    return { env: {}, error: null };
  }
  try {
    const parsed = JSON.parse(text) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return { env: {}, error: "环境变量 JSON 必须是对象格式" };
    }
    const normalized: Record<string, string> = {};
    for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
      normalized[key] = typeof value === "string" ? value : String(value ?? "");
    }
    return { env: normalized, error: null };
  } catch {
    return { env: {}, error: "环境变量 JSON 格式错误" };
  }
}

interface Props {
  onClose: () => void;
  showDevModelSetupTools?: boolean;
  onDevResetFirstUseOnboarding?: () => void;
  onDevOpenQuickModelSetup?: () => void;
}

interface RoutingSettings {
  max_call_depth: number;
  node_timeout_seconds: number;
  retry_count: number;
}

interface DesktopLifecyclePaths {
  app_data_dir: string;
  cache_dir: string;
  log_dir: string;
  default_work_dir: string;
}

interface DesktopCleanupResult {
  removed_files: number;
  removed_dirs: number;
}

const ROUTING_CAPABILITIES = [
  { label: "对话 Chat", value: "chat" },
  { label: "视觉 Vision", value: "vision" },
  { label: "生图 Image", value: "image_gen" },
  { label: "语音转写 STT", value: "audio_stt" },
  { label: "语音合成 TTS", value: "audio_tts" },
];

// 普通用户模式：仅保留关键入口，其他能力后台自动处理
const SHOW_CAPABILITY_ROUTING_SETTINGS = false;
const SHOW_HEALTH_SETTINGS = false;
const SHOW_MCP_SETTINGS = true;
const SHOW_AUTO_ROUTING_SETTINGS = false;

const DEFAULT_RUNTIME_PREFERENCES: RuntimePreferences = {
  default_work_dir: "",
  default_language: "zh-CN",
  immersive_translation_enabled: true,
  immersive_translation_display: "translated_only",
  immersive_translation_trigger: "auto",
  translation_engine: "model_then_free",
  translation_model_id: "",
  auto_update_enabled: true,
  update_channel: "stable",
  dismissed_update_version: "",
  last_update_check_at: "",
};

const DEFAULT_MODEL_PROVIDER = getModelProviderCatalogItem(DEFAULT_MODEL_PROVIDER_ID);

function formatRuntimeTimestamp(value: string): string {
  if (!value.trim()) {
    return "尚未检查";
  }
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString("zh-CN", { hour12: false });
}

export function SettingsView({
  onClose,
  showDevModelSetupTools = false,
  onDevResetFirstUseOnboarding,
  onDevOpenQuickModelSetup,
}: Props) {
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [selectedModelProviderId, setSelectedModelProviderId] = useState(DEFAULT_MODEL_PROVIDER.id);
  const [form, setForm] = useState({
    ...buildModelFormFromCatalogItem(DEFAULT_MODEL_PROVIDER),
    api_key: "",
  });
  const [error, setError] = useState("");
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<boolean | null>(null);
  const [modelSuggestions, setModelSuggestions] = useState<string[]>(DEFAULT_MODEL_PROVIDER.models);

  // 编辑状态 + API Key 可见性
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [showApiKey, setShowApiKey] = useState(false);

  // MCP 服务器管理
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [mcpServers, setMcpServers] = useState<any[]>([]);
  const [mcpForm, setMcpForm] = useState({ name: "", command: "", args: "", env: "" });
  const [mcpError, setMcpError] = useState("");
  const [showMcpEnvJson, setShowMcpEnvJson] = useState(false);
  const [activeTab, setActiveTab] = useState<
    "models" | "capabilities" | "health" | "mcp" | "search" | "routing"
  >("models");

  // 搜索引擎配置
  const [searchConfigs, setSearchConfigs] = useState<ModelConfig[]>([]);
  const [searchForm, setSearchForm] = useState({ name: "", api_format: "", base_url: "", model_name: "", api_key: "" });
  const [searchError, setSearchError] = useState("");
  const [searchTesting, setSearchTesting] = useState(false);
  const [searchTestResult, setSearchTestResult] = useState<boolean | null>(null);

  // 搜索引擎编辑状态 + API Key 可见性
  const [editingSearchId, setEditingSearchId] = useState<string | null>(null);
  const [showSearchApiKey, setShowSearchApiKey] = useState(false);
  const [routeSettings, setRouteSettings] = useState<RoutingSettings>({
    max_call_depth: 4,
    node_timeout_seconds: 60,
    retry_count: 0,
  });
  const [routeSaveState, setRouteSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [routeError, setRouteError] = useState("");

  const [providers, setProviders] = useState<ProviderConfig[]>([]);

  const [selectedCapability, setSelectedCapability] = useState("chat");
  const [chatRoutingPolicy, setChatRoutingPolicy] = useState<CapabilityRoutingPolicy>({
    capability: "chat",
    primary_provider_id: "",
    primary_model: "",
    fallback_chain_json: "[]",
    timeout_ms: 60000,
    retry_count: 0,
    enabled: true,
  });
  const [policySaveState, setPolicySaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [policyError, setPolicyError] = useState("");
  const [chatPrimaryModels, setChatPrimaryModels] = useState<string[]>([]);
  const [chatFallbackRows, setChatFallbackRows] = useState<Array<{ provider_id: string; model: string }>>([]);
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
  const [runtimePreferences, setRuntimePreferences] = useState<RuntimePreferences>(
    DEFAULT_RUNTIME_PREFERENCES,
  );
  const [runtimePreferencesSaveState, setRuntimePreferencesSaveState] = useState<
    "idle" | "saving" | "saved" | "error"
  >("idle");
  const [runtimePreferencesError, setRuntimePreferencesError] = useState("");
  const [updaterPreferencesSaveState, setUpdaterPreferencesSaveState] = useState<
    "idle" | "saving" | "saved" | "error"
  >("idle");
  const [updaterPreferencesError, setUpdaterPreferencesError] = useState("");
  const [desktopLifecyclePaths, setDesktopLifecyclePaths] = useState<DesktopLifecyclePaths | null>(
    null,
  );
  const [desktopLifecycleLoading, setDesktopLifecycleLoading] = useState(false);
  const [desktopLifecycleActionState, setDesktopLifecycleActionState] = useState<
    "idle" | "opening" | "clearing" | "exporting"
  >("idle");
  const [desktopLifecycleError, setDesktopLifecycleError] = useState("");
  const [desktopLifecycleMessage, setDesktopLifecycleMessage] = useState("");
  const selectedModelProvider = getModelProviderCatalogItem(selectedModelProviderId);

  async function persistRuntimePreferencesInput(input: Record<string, unknown>) {
    const saved = await invoke<RuntimePreferences>("set_runtime_preferences", { input });
    const normalized = normalizeRuntimePreferences(saved);
    setRuntimePreferences(normalized);
    return normalized;
  }

  const appUpdater = useAppUpdater({
    autoCheck: runtimePreferences.auto_update_enabled,
    dismissedVersion: runtimePreferences.dismissed_update_version,
    lastCheckedAt: runtimePreferences.last_update_check_at,
    onPreferencesChange: async (patch) => {
      try {
        await persistRuntimePreferencesInput(patch);
      } catch (error) {
        console.warn("保存更新状态失败:", error);
      }
    },
  });

  function resetModelForm(providerId = DEFAULT_MODEL_PROVIDER_ID) {
    const provider = getModelProviderCatalogItem(providerId);
    setSelectedModelProviderId(provider.id);
    setForm({
      ...buildModelFormFromCatalogItem(provider),
      api_key: "",
    });
    setModelSuggestions(provider.models);
    setEditingModelId(null);
    setShowApiKey(false);
    setError("");
    setTestResult(null);
  }

  function validateModelForm() {
    if (!form.name.trim()) {
      return "请输入名称";
    }
    if (!form.base_url.trim()) {
      return "请输入 Base URL";
    }
    if (!form.model_name.trim()) {
      return "请输入模型名称";
    }
    if (!form.api_key.trim()) {
      return "请输入 API Key";
    }
    return null;
  }

  function inferConnectionKey(baseUrl: string, apiFormat: string): string {
    const normalized = (baseUrl || "").toLowerCase();
    if (normalized.includes("deepseek")) return "deepseek";
    if (normalized.includes("dashscope")) return "qwen";
    if (normalized.includes("moonshot") || normalized.includes("kimi")) return "moonshot";
    if (normalized.includes("bigmodel") || normalized.includes("open.bigmodel")) return "zhipu";
    if (normalized.includes("anthropic")) return "anthropic";
    if (normalized.includes("minimax")) return "minimax";
    if (normalized.includes("lingyiwanwu")) return "yi";
    if (normalized.includes("openai")) return "openai";
    if (apiFormat === "anthropic") return "anthropic";
    return "openai";
  }

  async function syncConnectionToRouting(model: ModelConfig, apiKey: string) {
    await invoke("save_provider_config", {
      config: {
        id: model.id,
        provider_key: inferConnectionKey(model.base_url, model.api_format),
        display_name: model.name || model.model_name || model.id,
        protocol_type: model.api_format === "anthropic" ? "anthropic" : "openai",
        base_url: model.base_url,
        auth_type: "api_key",
        api_key_encrypted: apiKey,
        org_id: "",
        extra_json: "{}",
        enabled: true,
      },
    });
  }

  async function syncModelConnections(modelList: ModelConfig[]) {
    await Promise.all(
      modelList.map(async (model) => {
        try {
          const apiKey = await invoke<string>("get_model_api_key", { modelId: model.id });
          await syncConnectionToRouting(model, apiKey);
        } catch (e) {
          console.warn("同步连接配置失败:", model.id, e);
        }
      }),
    );
  }

  useEffect(() => {
    loadModels();
    loadSearchConfigs();
    loadRuntimePreferences();
    loadDesktopLifecyclePaths();
    if (SHOW_MCP_SETTINGS) {
      loadMcpServers();
    }
    if (SHOW_AUTO_ROUTING_SETTINGS) {
      loadRoutingSettings();
    }
    if (SHOW_CAPABILITY_ROUTING_SETTINGS) {
      loadCapabilityRoutingPolicy("chat");
      loadRouteTemplates("chat");
    }
  }, []);

  useEffect(() => {
    if (chatRoutingPolicy.primary_provider_id) {
      loadChatPrimaryModels(chatRoutingPolicy.primary_provider_id, selectedCapability);
    }
  }, [chatRoutingPolicy.primary_provider_id, selectedCapability]);

  useEffect(() => {
    if (SHOW_HEALTH_SETTINGS && activeTab === "health") {
      loadRecentRouteLogs(false);
      loadRouteStats();
    }
  }, [activeTab]);

  async function loadModels() {
    try {
      const list = await invoke<ModelConfig[]>("list_model_configs");
      setModels(list);
      await syncModelConnections(list);
      await loadProviderConfigs(list);
    } catch (e) {
      setError("加载模型连接失败: " + String(e));
    }
  }

  async function loadSearchConfigs() {
    try {
      const list = await invoke<ModelConfig[]>("list_search_configs");
      setSearchConfigs(list);
    } catch (e) {
      console.error("加载搜索配置失败:", e);
    }
  }

  function normalizeRuntimePreferences(raw: unknown): RuntimePreferences {
    const parsed = (raw ?? {}) as Partial<RuntimePreferences>;
    const immersiveDisplay =
      parsed.immersive_translation_display === "bilingual_inline"
        ? "bilingual_inline"
        : "translated_only";
    const triggerMode = parsed.immersive_translation_trigger === "manual" ? "manual" : "auto";
    const translationEngine =
      parsed.translation_engine === "model_only" || parsed.translation_engine === "free_only"
        ? parsed.translation_engine
        : "model_then_free";
    const translationModelId =
      typeof parsed.translation_model_id === "string" ? parsed.translation_model_id : "";
    const updateChannel =
      typeof parsed.update_channel === "string" && parsed.update_channel === "stable"
        ? parsed.update_channel
        : "stable";
    return {
      default_work_dir: typeof parsed.default_work_dir === "string" ? parsed.default_work_dir : "",
      default_language:
        typeof parsed.default_language === "string" && parsed.default_language
          ? parsed.default_language
          : "zh-CN",
      immersive_translation_enabled:
        typeof parsed.immersive_translation_enabled === "boolean"
          ? parsed.immersive_translation_enabled
          : true,
      immersive_translation_display: immersiveDisplay,
      immersive_translation_trigger: triggerMode,
      translation_engine: translationEngine,
      translation_model_id: translationModelId,
      auto_update_enabled:
        typeof parsed.auto_update_enabled === "boolean" ? parsed.auto_update_enabled : true,
      update_channel: updateChannel,
      dismissed_update_version:
        typeof parsed.dismissed_update_version === "string"
          ? parsed.dismissed_update_version
          : "",
      last_update_check_at:
        typeof parsed.last_update_check_at === "string" ? parsed.last_update_check_at : "",
    };
  }

  async function loadRuntimePreferences() {
    try {
      const prefs = await invoke<RuntimePreferences>("get_runtime_preferences");
      setRuntimePreferences(normalizeRuntimePreferences(prefs));
    } catch (e) {
      console.warn("加载运行时偏好失败:", e);
      setRuntimePreferences(DEFAULT_RUNTIME_PREFERENCES);
    }
  }

  async function handleSaveRuntimePreferences() {
    setRuntimePreferencesSaveState("saving");
    setRuntimePreferencesError("");
    try {
      const input: {
        default_work_dir?: string;
        default_language: string;
        immersive_translation_enabled: boolean;
        immersive_translation_display: string;
        immersive_translation_trigger: string;
        translation_engine: string;
        translation_model_id: string;
      } = {
        default_language: runtimePreferences.default_language,
        immersive_translation_enabled: runtimePreferences.immersive_translation_enabled,
        immersive_translation_display: runtimePreferences.immersive_translation_display,
        immersive_translation_trigger: runtimePreferences.immersive_translation_trigger,
        translation_engine: runtimePreferences.translation_engine,
        translation_model_id: runtimePreferences.translation_model_id,
      };
      if (runtimePreferences.default_work_dir.trim()) {
        input.default_work_dir = runtimePreferences.default_work_dir;
      }
      await persistRuntimePreferencesInput(input);
      setRuntimePreferencesSaveState("saved");
      setTimeout(() => setRuntimePreferencesSaveState("idle"), 1200);
    } catch (e) {
      setRuntimePreferencesSaveState("error");
      setRuntimePreferencesError("保存语言与翻译设置失败: " + String(e));
    }
  }

  async function handleSaveUpdaterPreferences() {
    setUpdaterPreferencesSaveState("saving");
    setUpdaterPreferencesError("");
    try {
      await persistRuntimePreferencesInput({
        auto_update_enabled: runtimePreferences.auto_update_enabled,
        update_channel: runtimePreferences.update_channel,
        dismissed_update_version: runtimePreferences.dismissed_update_version,
        last_update_check_at: runtimePreferences.last_update_check_at,
      });
      setUpdaterPreferencesSaveState("saved");
      setTimeout(() => setUpdaterPreferencesSaveState("idle"), 1200);
    } catch (e) {
      setUpdaterPreferencesSaveState("error");
      setUpdaterPreferencesError("保存更新设置失败: " + String(e));
    }
  }

  async function loadDesktopLifecyclePaths() {
    setDesktopLifecycleLoading(true);
    setDesktopLifecycleError("");
    try {
      const paths = await invoke<DesktopLifecyclePaths>("get_desktop_lifecycle_paths");
      setDesktopLifecyclePaths(paths);
    } catch (e) {
      setDesktopLifecycleError("加载数据目录失败: " + String(e));
    } finally {
      setDesktopLifecycleLoading(false);
    }
  }

  async function handleOpenDesktopPath(path: string) {
    if (!path.trim()) return;
    setDesktopLifecycleActionState("opening");
    setDesktopLifecycleError("");
    setDesktopLifecycleMessage("");
    try {
      await invoke("open_desktop_path", { path });
    } catch (e) {
      setDesktopLifecycleError("打开目录失败: " + String(e));
    } finally {
      setDesktopLifecycleActionState("idle");
    }
  }

  async function handleClearDesktopCacheAndLogs() {
    setDesktopLifecycleActionState("clearing");
    setDesktopLifecycleError("");
    setDesktopLifecycleMessage("");
    try {
      const result = await invoke<DesktopCleanupResult>("clear_desktop_cache_and_logs");
      setDesktopLifecycleMessage(
        `已清理 ${result.removed_files} 个文件，删除 ${result.removed_dirs} 个目录`,
      );
      await loadDesktopLifecyclePaths();
    } catch (e) {
      setDesktopLifecycleError("清理缓存与日志失败: " + String(e));
    } finally {
      setDesktopLifecycleActionState("idle");
    }
  }

  async function handleExportDesktopEnvironmentSummary() {
    setDesktopLifecycleActionState("exporting");
    setDesktopLifecycleError("");
    setDesktopLifecycleMessage("");
    try {
      const summary = await invoke<string>("export_desktop_environment_summary");
      await navigator?.clipboard?.writeText?.(summary);
      setDesktopLifecycleMessage("环境摘要已复制到剪贴板");
    } catch (e) {
      setDesktopLifecycleError("导出环境摘要失败: " + String(e));
    } finally {
      setDesktopLifecycleActionState("idle");
    }
  }

  async function loadRoutingSettings() {
    try {
      const settings = await invoke<RoutingSettings>("get_routing_settings");
      setRouteSettings(settings);
    } catch (e) {
      setRouteError("加载自动路由设置失败: " + String(e));
      setRouteSaveState("error");
    }
  }

  async function loadProviderConfigs(modelList: ModelConfig[] = models) {
    try {
      const list = await invoke<ProviderConfig[]>("list_provider_configs");
      const ids = new Set(modelList.map((m) => m.id));
      const aligned = list.filter((p) => ids.has(p.id));
      setProviders(aligned);
      if (aligned.length === 0) {
        setHealthProviderId("");
      } else if (!healthProviderId || !aligned.some((p) => p.id === healthProviderId)) {
        setHealthProviderId(aligned[0].id);
      }
    } catch (e) {
      console.warn("加载连接路由配置失败:", e);
    }
  }

  async function loadCapabilityRoutingPolicy(capability: string) {
    try {
      const policy = await invoke<CapabilityRoutingPolicy | null>("get_capability_routing_policy", {
        capability,
      });
      if (policy) {
        setChatRoutingPolicy(policy);
        try {
          const parsed = JSON.parse(policy.fallback_chain_json || "[]");
          if (Array.isArray(parsed)) {
            setChatFallbackRows(
              parsed.map((item) => ({
                provider_id: String(item?.provider_id || ""),
                model: String(item?.model || ""),
              })),
            );
          }
        } catch {
          setChatFallbackRows([]);
        }
      } else {
        setChatRoutingPolicy({
          capability,
          primary_provider_id: "",
          primary_model: "",
          fallback_chain_json: "[]",
          timeout_ms: 60000,
          retry_count: 0,
          enabled: true,
        });
        setChatFallbackRows([]);
      }
    } catch (e) {
      setPolicyError("加载聊天路由策略失败: " + String(e));
    }
  }

  async function loadRouteTemplates(capability: string) {
    try {
      const list = await invoke<CapabilityRouteTemplateInfo[]>("list_capability_route_templates", {
        capability,
      });
      setRouteTemplates(list);
      if (list.length > 0 && !list.some((x) => x.template_id === selectedRouteTemplateId)) {
        setSelectedRouteTemplateId(list[0].template_id);
      }
    } catch {
      setRouteTemplates([]);
    }
  }

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
    } catch (e) {
      setPolicySaveState("error");
      setPolicyError("保存聊天路由策略失败: " + String(e));
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
    } catch (e) {
      setHealthResult({
        provider_id: healthProviderId,
        ok: false,
        protocol_type: "",
        message: String(e),
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
    } catch (e) {
      setAllHealthResults([
        {
          provider_id: "",
          ok: false,
          protocol_type: "",
          message: String(e),
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

  const filteredRouteLogs = routeLogs.filter((log) => {
    if (routeLogsCapabilityFilter !== "all" && log.capability !== routeLogsCapabilityFilter) return false;
    if (routeLogsResultFilter === "success" && !log.success) return false;
    if (routeLogsResultFilter === "failed" && log.success) return false;
    if (routeLogsErrorKindFilter !== "all" && log.error_kind !== routeLogsErrorKindFilter) return false;
    return true;
  });

  function addFallbackRow() {
    setChatFallbackRows((rows) => [...rows, { provider_id: "", model: "" }]);
  }

  function updateFallbackRow(index: number, patch: Partial<{ provider_id: string; model: string }>) {
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
      const parsed = JSON.parse(policy.fallback_chain_json || "[]");
      if (Array.isArray(parsed)) {
        setChatFallbackRows(
          parsed.map((item) => ({
            provider_id: String(item?.provider_id || ""),
            model: String(item?.model || ""),
          })),
        );
      } else {
        setChatFallbackRows([]);
      }
    } catch (e) {
      const raw = String(e);
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

  async function handleSaveRoutingSettings() {
    setRouteSaveState("saving");
    setRouteError("");
    try {
      await invoke("set_routing_settings", {
        settings: {
          max_call_depth: Math.max(2, Math.min(8, routeSettings.max_call_depth)),
          node_timeout_seconds: Math.max(5, Math.min(600, routeSettings.node_timeout_seconds)),
          retry_count: Math.max(0, Math.min(2, routeSettings.retry_count)),
        },
      });
      setRouteSaveState("saved");
      setTimeout(() => setRouteSaveState("idle"), 1200);
    } catch (e) {
      setRouteError("保存自动路由设置失败: " + String(e));
      setRouteSaveState("error");
    }
  }

  // 加载已保存的模型配置到表单（用于编辑）
  async function handleEditModel(m: ModelConfig) {
    try {
      const apiKey = await invoke<string>("get_model_api_key", { modelId: m.id });
      const provider = resolveCatalogItemForConfig({
        api_format: m.api_format === "anthropic" ? "anthropic" : "openai",
        base_url: m.base_url,
      });
      setForm({
        name: m.name,
        api_format: m.api_format === "anthropic" ? "anthropic" : "openai",
        base_url: m.base_url,
        model_name: m.model_name,
        api_key: apiKey,
      });
      setSelectedModelProviderId(provider.id);
      setEditingModelId(m.id);
      setShowApiKey(false);
      setError("");
      setTestResult(null);
      setModelSuggestions(provider.models);
    } catch (e) {
      setError("加载配置失败: " + String(e));
    }
  }

  // 加载已保存的搜索配置到表单（用于编辑）
  async function handleEditSearch(s: ModelConfig) {
    try {
      const apiKey = await invoke<string>("get_model_api_key", { modelId: s.id });
      setSearchForm({
        name: s.name,
        api_format: s.api_format,
        base_url: s.base_url,
        model_name: s.model_name,
        api_key: apiKey,
      });
      setEditingSearchId(s.id);
      setShowSearchApiKey(false);
      setSearchError("");
      setSearchTestResult(null);
    } catch (e) {
      setSearchError("加载配置失败: " + String(e));
    }
  }

  async function handleSave() {
    const validationError = validateModelForm();
    if (validationError) {
      setError(validationError);
      setTestResult(null);
      return;
    }
    setError("");
    try {
      await invoke("save_model_config", {
        config: {
          id: editingModelId || "",
          name: form.name.trim(),
          api_format: form.api_format,
          base_url: form.base_url.trim(),
          model_name: form.model_name.trim(),
          is_default: editingModelId
            ? models.find((m) => m.id === editingModelId)?.is_default ?? false
            : models.length === 0,
        },
        apiKey: form.api_key.trim(),
      });
      resetModelForm();
      await loadModels();
    } catch (e: unknown) {
      setError(String(e));
    }
  }

  async function handleTest() {
    const validationError = validateModelForm();
    if (validationError) {
      setError(validationError);
      setTestResult(null);
      return;
    }
    setError("");
    setTesting(true);
    setTestResult(null);
    try {
      const ok = await invoke<boolean>("test_connection_cmd", {
        config: {
          id: "",
          name: form.name.trim(),
          api_format: form.api_format,
          base_url: form.base_url.trim(),
          model_name: form.model_name.trim(),
          is_default: false,
        },
        apiKey: form.api_key.trim(),
      });
      setTestResult(ok);
    } catch (e: unknown) {
      setError(String(e));
      setTestResult(false);
    } finally {
      setTesting(false);
    }
  }

  function applyPreset(value: string) {
    const preset = getModelProviderCatalogItem(value);
    setForm((f) => ({
      ...f,
      ...buildModelFormFromCatalogItem(preset),
      api_key: f.api_key,
    }));
    setSelectedModelProviderId(preset.id);
    setModelSuggestions(preset.models);
    setError("");
    setTestResult(null);
  }

  function applyMcpPreset(value: string) {
    const preset = MCP_PRESETS.find((p) => p.value === value);
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
    setMcpForm((s) => ({ ...s, env: JSON.stringify(next) }));
  }

  function applySearchPreset(value: string) {
    const preset = SEARCH_PRESETS.find((p) => p.value === value);
    if (!preset || !preset.value) return;
    setSearchForm((f) => ({
      ...f,
      name: preset.label.replace(/ \(.*\)/, ""),
      api_format: preset.api_format,
      base_url: preset.base_url,
      model_name: preset.model_name,
    }));
  }

  async function handleDelete(id: string) {
    await invoke("delete_model_config", { modelId: id });
    await invoke("delete_provider_config", { providerId: id }).catch(() => null);
    // 若删除的是当前编辑项，重置表单
    if (editingModelId === id) {
      resetModelForm();
    }
    await loadModels();
  }

  async function loadMcpServers() {
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const list = await invoke<any[]>("list_mcp_servers");
      setMcpServers(list);
    } catch (e) {
      console.error("加载 MCP 服务器失败:", e);
    }
  }

  async function handleAddMcp() {
    setMcpError("");
    try {
      const args = mcpForm.args.split(/\s+/).filter(Boolean);
      const parsedEnv = parseMcpEnvJson(mcpForm.env);
      if (parsedEnv.error) {
        setMcpError(parsedEnv.error);
        return;
      }
      await invoke("add_mcp_server", {
        name: mcpForm.name,
        command: mcpForm.command,
        args,
        env: parsedEnv.env,
      });
      setMcpForm({ name: "", command: "", args: "", env: "" });
      setShowMcpEnvJson(false);
      loadMcpServers();
    } catch (e) {
      setMcpError(String(e));
    }
  }

  async function handleRemoveMcp(id: string) {
    await invoke("remove_mcp_server", { id });
    loadMcpServers();
  }

  async function handleSaveSearch() {
    setSearchError("");
    try {
      await invoke("save_model_config", {
        config: {
          id: editingSearchId || "",
          name: searchForm.name,
          api_format: searchForm.api_format,
          base_url: searchForm.base_url,
          model_name: searchForm.model_name,
          is_default: editingSearchId
            ? searchConfigs.find((s) => s.id === editingSearchId)?.is_default ?? false
            : searchConfigs.length === 0,
        },
        apiKey: searchForm.api_key,
      });
      setSearchForm({ name: "", api_format: "", base_url: "", model_name: "", api_key: "" });
      setEditingSearchId(null);
      setShowSearchApiKey(false);
      loadSearchConfigs();
    } catch (e) {
      setSearchError(String(e));
    }
  }

  async function handleTestSearch() {
    setSearchTesting(true);
    setSearchTestResult(null);
    try {
      const ok = await invoke<boolean>("test_search_connection", {
        config: {
          id: "",
          name: searchForm.name,
          api_format: searchForm.api_format,
          base_url: searchForm.base_url,
          model_name: searchForm.model_name,
          is_default: false,
        },
        apiKey: searchForm.api_key,
      });
      setSearchTestResult(ok);
    } catch (e) {
      setSearchError(String(e));
      setSearchTestResult(false);
    } finally {
      setSearchTesting(false);
    }
  }

  async function handleSetDefaultSearch(id: string) {
    await invoke("set_default_search", { configId: id });
    loadSearchConfigs();
  }

  async function handleDeleteSearch(id: string) {
    await invoke("delete_model_config", { modelId: id });
    // 若删除的是当前编辑项，重置表单
    if (editingSearchId === id) {
      setEditingSearchId(null);
      setShowSearchApiKey(false);
      setSearchForm({ name: "", api_format: "", base_url: "", model_name: "", api_key: "" });
      setSearchError("");
      setSearchTestResult(null);
    }
    loadSearchConfigs();
  }

  const inputCls = "sm-input w-full text-sm py-1.5";
  const labelCls = "sm-field-label";
  const parsedMcpEnv = parseMcpEnvJson(mcpForm.env);
  const mcpApiKeyEnvKeys = Object.keys(parsedMcpEnv.env).filter((key) => key.toUpperCase().includes("API_KEY"));

  // 眼睛图标：显示状态（可见）
  function EyeOpenIcon() {
    return (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963 7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
      </svg>
    );
  }

  // 眼睛图标：隐藏状态（划线）
  function EyeSlashIcon() {
    return (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M3.98 8.223A10.477 10.477 0 001.934 12C3.226 16.338 7.244 19.5 12 19.5c.993 0 1.953-.138 2.863-.395M6.228 6.228A10.45 10.45 0 0112 4.5c4.756 0 8.773 3.162 10.065 7.498a10.523 10.523 0 01-4.293 5.774M6.228 6.228L3 3m3.228 3.228l3.65 3.65m7.894 7.894L21 21m-3.228-3.228l-3.65-3.65m0 0a3 3 0 10-4.243-4.243m4.242 4.242L9.88 9.88" />
      </svg>
    );
  }

  return (
    <div className="sm-surface flex flex-col h-full p-6 overflow-y-auto">
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-4">
          <button
            onClick={() => setActiveTab("models")}
              className={"sm-btn h-8 px-2 rounded-none border-b-2 text-sm font-medium transition-colors " +
              (activeTab === "models" ? "text-[var(--sm-primary-strong)] border-[var(--sm-primary)]" : "sm-text-muted border-transparent hover:text-[var(--sm-text)]")}
          >
            模型连接
          </button>
          {SHOW_CAPABILITY_ROUTING_SETTINGS && (
            <button
              onClick={() => setActiveTab("capabilities")}
                className={"sm-btn h-8 px-2 rounded-none border-b-2 text-sm font-medium transition-colors " +
                (activeTab === "capabilities" ? "text-[var(--sm-primary-strong)] border-[var(--sm-primary)]" : "sm-text-muted border-transparent hover:text-[var(--sm-text)]")}
            >
              能力路由
            </button>
          )}
          {SHOW_HEALTH_SETTINGS && (
            <button
              onClick={() => setActiveTab("health")}
                className={"sm-btn h-8 px-2 rounded-none border-b-2 text-sm font-medium transition-colors " +
                (activeTab === "health" ? "text-[var(--sm-primary-strong)] border-[var(--sm-primary)]" : "sm-text-muted border-transparent hover:text-[var(--sm-text)]")}
            >
              健康检查
            </button>
          )}
          {SHOW_MCP_SETTINGS && (
            <button
              onClick={() => setActiveTab("mcp")}
                className={"sm-btn h-8 px-2 rounded-none border-b-2 text-sm font-medium transition-colors " +
                (activeTab === "mcp" ? "text-[var(--sm-primary-strong)] border-[var(--sm-primary)]" : "sm-text-muted border-transparent hover:text-[var(--sm-text)]")}
            >
              MCP 服务器
            </button>
          )}
          <button
            onClick={() => setActiveTab("search")}
              className={"sm-btn h-8 px-2 rounded-none border-b-2 text-sm font-medium transition-colors " +
              (activeTab === "search" ? "text-[var(--sm-primary-strong)] border-[var(--sm-primary)]" : "sm-text-muted border-transparent hover:text-[var(--sm-text)]")}
          >
            搜索引擎
          </button>
          {SHOW_AUTO_ROUTING_SETTINGS && (
            <button
              onClick={() => setActiveTab("routing")}
                className={"sm-btn h-8 px-2 rounded-none border-b-2 text-sm font-medium transition-colors " +
                (activeTab === "routing" ? "text-[var(--sm-primary-strong)] border-[var(--sm-primary)]" : "sm-text-muted border-transparent hover:text-[var(--sm-text)]")}
            >
              自动路由
            </button>
          )}
        </div>
        <button onClick={onClose} className="text-gray-500 hover:text-gray-800 text-sm">
          返回
        </button>
      </div>

      {activeTab === "models" && (<>
      {models.length > 0 && (
        <div className="mb-6 space-y-2">
          <div className="text-xs text-gray-500 mb-2">已配置模型</div>
          {models.map((m) => (
            <div
              key={m.id}
              className={
                "flex items-center justify-between bg-white rounded-lg px-4 py-2.5 text-sm border transition-colors " +
                (editingModelId === m.id ? "border-blue-400 ring-1 ring-blue-400" : "border-transparent hover:border-gray-200")
              }
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-gray-800">{m.name}</span>
                  {m.is_default && (
                    <span className="text-[10px] bg-blue-500 text-white px-1.5 py-0.5 rounded">默认</span>
                  )}
                </div>
                <div className="text-xs text-gray-400 mt-0.5 truncate">
                  {m.model_name} · {m.api_format === "anthropic" ? "Anthropic" : "OpenAI 兼容"} · {m.base_url}
                </div>
              </div>
              <div className="flex items-center gap-2 flex-shrink-0 ml-3">
                <button
                  onClick={() => handleEditModel(m)}
                  className="text-blue-500 hover:text-blue-600 text-xs"
                >
                  编辑
                </button>
                <button
                  onClick={() => handleDelete(m.id)}
                  className="text-red-400 hover:text-red-500 text-xs"
                >
                  删除
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="bg-white rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between mb-2">
          <div className="text-xs font-medium text-gray-500">
            {editingModelId ? "编辑模型" : "添加模型"}
          </div>
          {editingModelId && (
            <button
              onClick={() => resetModelForm()}
              className="text-xs text-gray-400 hover:text-gray-600"
            >
              取消编辑
            </button>
          )}
        </div>
        <div>
          <label className={labelCls}>快速选择模型服务</label>
          <select
            data-testid="settings-model-provider-preset"
            className={inputCls}
            value={selectedModelProviderId}
            onChange={(e) => applyPreset(e.target.value)}
          >
            {MODEL_PROVIDER_CATALOG.map((p) => (
              <option key={p.id} value={p.id}>{p.label}</option>
            ))}
          </select>
        </div>
        <div>
          <label className={labelCls}>名称</label>
          <input
            data-testid="settings-model-provider-name"
            className={inputCls}
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
          />
        </div>
        <div>
          <label className={labelCls}>API 格式</label>
          <select
            className={inputCls}
            value={form.api_format}
            disabled
          >
            <option value="openai">OpenAI 兼容</option>
            <option value="anthropic">Anthropic (Claude)</option>
          </select>
        </div>
        <div>
          <label className={labelCls}>Base URL</label>
          <input
            data-testid="settings-model-provider-base-url"
            className={inputCls}
            value={form.base_url}
            placeholder={selectedModelProvider.baseUrlPlaceholder}
            onChange={(e) => setForm({ ...form, base_url: e.target.value })}
          />
        </div>
        <div>
          <label className={labelCls}>模型名称</label>
          <input
            data-testid="settings-model-provider-model-name"
            className={inputCls}
            list="model-suggestions"
            value={form.model_name}
            placeholder={selectedModelProvider.modelNamePlaceholder}
            onChange={(e) => setForm({ ...form, model_name: e.target.value })}
          />
          {modelSuggestions.length > 0 && (
            <datalist id="model-suggestions">
              {modelSuggestions.map((m) => (
                <option key={m} value={m} />
              ))}
            </datalist>
          )}
        </div>
        <div className="rounded-2xl border border-gray-200 bg-gray-50 px-4 py-4">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <div className="text-sm font-medium text-gray-800">{selectedModelProvider.label}</div>
                <span className="inline-flex items-center rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-blue-700">
                  {selectedModelProvider.protocolLabel}
                </span>
              </div>
              <div className="mt-2 text-xs leading-5 text-gray-500">{selectedModelProvider.helper}</div>
            </div>
            {selectedModelProvider.officialConsoleUrl ? (
              <div className="flex flex-wrap gap-2">
                <a
                  href={selectedModelProvider.officialConsoleUrl}
                  target="_blank"
                  rel="noreferrer"
                  className="sm-btn rounded-xl border border-gray-200 bg-white px-4 py-2 text-sm text-gray-700 hover:bg-gray-100"
                >
                  {selectedModelProvider.officialConsoleLabel ?? "获取 API Key"}
                </a>
                {selectedModelProvider.officialDocsUrl ? (
                  <a
                    href={selectedModelProvider.officialDocsUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="sm-btn rounded-xl border border-transparent px-4 py-2 text-sm text-gray-500 hover:bg-white hover:text-gray-700"
                  >
                    {selectedModelProvider.officialDocsLabel ?? "查看文档"}
                  </a>
                ) : null}
              </div>
            ) : null}
          </div>
          {selectedModelProvider.isCustom ? (
            <div
              data-testid="settings-model-provider-custom-guidance"
              className="mt-3 rounded-2xl border border-dashed border-gray-200 bg-white px-3 py-3"
            >
              <div className="text-xs font-semibold text-gray-800">
                {selectedModelProvider.customGuidanceTitle}
              </div>
              <div className="mt-2 space-y-1.5 text-[12px] leading-5 text-gray-500">
                {selectedModelProvider.customGuidanceLines?.map((line) => (
                  <div key={line}>{line}</div>
                ))}
              </div>
            </div>
          ) : null}
        </div>
        <div>
          <label className={labelCls}>API Key</label>
          <div className="relative">
            <input
              data-testid="settings-model-provider-api-key"
              className={inputCls + " pr-10"}
              type={showApiKey ? "text" : "password"}
              value={form.api_key}
              onChange={(e) => setForm({ ...form, api_key: e.target.value })}
            />
            <button
              type="button"
              onClick={() => setShowApiKey(!showApiKey)}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 p-1"
              title={showApiKey ? "隐藏" : "显示"}
            >
              {showApiKey ? <EyeSlashIcon /> : <EyeOpenIcon />}
            </button>
          </div>
        </div>
        {error && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{error}</div>}
        {testResult !== null && (
          <div className={"text-xs " + (testResult ? "bg-green-50 text-green-600 px-2 py-1 rounded" : "bg-red-50 text-red-600 px-2 py-1 rounded")}>
            {testResult ? "连接成功" : "连接失败，请检查配置"}
          </div>
        )}
        <div className="flex gap-2 pt-1">
          <button
            onClick={handleTest}
            disabled={testing}
            className="flex-1 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {testing ? "测试中..." : "测试连接"}
          </button>
          <button
            data-testid="settings-model-provider-save"
            onClick={handleSave}
            className="flex-1 bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {editingModelId ? "保存修改" : "保存"}
          </button>
        </div>
        <div className="text-xs text-gray-400">
          保存后会自动同步到默认路由和健康检查，无需重复配置。
        </div>
      </div>
      <div className="bg-white rounded-lg p-4 space-y-3 mt-4">
        <div className="text-xs font-medium text-gray-500">语言与沉浸式翻译</div>
        <div>
          <label className={labelCls}>默认语言</label>
          <select
            aria-label="默认语言"
            className={inputCls}
            value={runtimePreferences.default_language}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({ ...prev, default_language: e.target.value }))
            }
          >
            <option value="zh-CN">简体中文 (zh-CN)</option>
            <option value="en-US">English (en-US)</option>
          </select>
        </div>
        <label className="flex items-center gap-2 text-xs text-gray-600">
          <input
            aria-label="启用沉浸式翻译"
            type="checkbox"
            checked={runtimePreferences.immersive_translation_enabled}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                immersive_translation_enabled: e.target.checked,
              }))
            }
          />
          启用沉浸式翻译
        </label>
        <div>
          <label className={labelCls}>显示模式</label>
          <select
            aria-label="翻译显示模式"
            className={inputCls}
            value={runtimePreferences.immersive_translation_display}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                immersive_translation_display:
                  e.target.value === "bilingual_inline" ? "bilingual_inline" : "translated_only",
              }))
            }
          >
            <option value="translated_only">仅译文</option>
            <option value="bilingual_inline">双语对照</option>
          </select>
        </div>
        <div>
          <label className={labelCls}>翻译触发方式</label>
          <select
            aria-label="翻译触发方式"
            className={inputCls}
            value={runtimePreferences.immersive_translation_trigger}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                immersive_translation_trigger: e.target.value === "manual" ? "manual" : "auto",
              }))
            }
          >
            <option value="auto">自动翻译（默认）</option>
            <option value="manual">手动触发</option>
          </select>
        </div>
        <div>
          <label className={labelCls}>翻译引擎策略</label>
          <select
            aria-label="翻译引擎策略"
            className={inputCls}
            value={runtimePreferences.translation_engine}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                translation_engine:
                  e.target.value === "model_only" || e.target.value === "free_only"
                    ? e.target.value
                    : "model_then_free",
                translation_model_id: e.target.value === "free_only" ? "" : prev.translation_model_id,
              }))
            }
          >
            <option value="model_then_free">优先模型，失败回退免费翻译（推荐）</option>
            <option value="model_only">仅使用翻译模型</option>
            <option value="free_only">仅使用免费翻译</option>
          </select>
        </div>
        <div>
          <label className={labelCls}>翻译模型</label>
          <select
            aria-label="翻译模型"
            className={inputCls}
            disabled={runtimePreferences.translation_engine === "free_only"}
            value={runtimePreferences.translation_model_id}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                translation_model_id: e.target.value,
              }))
            }
          >
            <option value="">跟随默认模型</option>
            {models.map((model) => (
              <option key={model.id} value={model.id}>
                {model.name || model.model_name || model.id}
              </option>
            ))}
          </select>
        </div>
        {runtimePreferences.translation_engine !== "free_only" && models.length === 0 && (
          <div className="bg-amber-50 text-amber-700 text-xs px-2 py-1 rounded">
            当前未配置可用模型。翻译会尝试免费翻译接口；若策略为“仅使用翻译模型”则可能失败。
          </div>
        )}
        {runtimePreferences.translation_engine === "model_only" && models.length === 0 && (
          <div className="bg-red-50 text-red-700 text-xs px-2 py-1 rounded">
            已选择仅模型翻译，但当前无可用模型配置。建议切换到“优先模型，失败回退免费翻译”。
          </div>
        )}
        {runtimePreferences.translation_model_id &&
          !models.some((model) => model.id === runtimePreferences.translation_model_id) && (
            <div className="bg-amber-50 text-amber-700 text-xs px-2 py-1 rounded">
              选中的翻译模型不存在，将自动跟随默认模型或回退免费翻译。
            </div>
          )}
        {runtimePreferencesError && (
          <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">
            {runtimePreferencesError}
          </div>
        )}
        {runtimePreferencesSaveState === "saved" && (
          <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">已保存</div>
        )}
        <button
          onClick={handleSaveRuntimePreferences}
          disabled={runtimePreferencesSaveState === "saving"}
          className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
        >
          {runtimePreferencesSaveState === "saving" ? "保存中..." : "保存语言与翻译设置"}
        </button>
      </div>

      <div className="bg-white rounded-lg p-4 space-y-3 mt-4">
        <div className="text-xs font-medium text-gray-500">软件更新</div>
        <label className="flex items-center gap-2 text-xs text-gray-600">
          <input
            aria-label="自动检查更新"
            type="checkbox"
            checked={runtimePreferences.auto_update_enabled}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                auto_update_enabled: e.target.checked,
              }))
            }
          />
          自动检查更新
        </label>
        <div className="text-[11px] text-gray-400">
          当前仅支持 stable 渠道，推荐通过 Windows `.exe` 安装包使用应用内更新。
        </div>
        <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3 text-xs text-gray-600 space-y-1">
          <div>更新渠道：{runtimePreferences.update_channel}</div>
          <div>最近检查：{formatRuntimeTimestamp(appUpdater.lastCheckedAt)}</div>
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={handleSaveUpdaterPreferences}
            disabled={updaterPreferencesSaveState === "saving"}
            className="flex-1 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {updaterPreferencesSaveState === "saving" ? "保存中..." : "保存更新设置"}
          </button>
          <button
            type="button"
            onClick={() => void appUpdater.checkForUpdates({ manual: true })}
            disabled={appUpdater.isWorking}
            className="flex-1 bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {appUpdater.status === "checking" ? "检查中..." : "检查更新"}
          </button>
        </div>
        {updaterPreferencesError && (
          <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">
            {updaterPreferencesError}
          </div>
        )}
        {updaterPreferencesSaveState === "saved" && (
          <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">
            更新设置已保存
          </div>
        )}
        {appUpdater.status === "up_to_date" && (
          <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">
            当前已是最新版本
          </div>
        )}
        {appUpdater.status === "checking" && (
          <div className="bg-blue-50 text-blue-600 text-xs px-2 py-1 rounded">正在检查更新</div>
        )}
        {appUpdater.status === "update_available" && appUpdater.availableUpdate && (
          <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-3 space-y-2">
            <div className="text-sm font-medium text-blue-700">
              {`发现新版本 v${appUpdater.availableUpdate.version}`}
            </div>
            {appUpdater.availableUpdate.body && (
              <div className="text-xs text-blue-700 whitespace-pre-wrap">
                {appUpdater.availableUpdate.body}
              </div>
            )}
            <div className="flex gap-2">
              {appUpdater.canDismiss && (
                <button
                  type="button"
                  onClick={appUpdater.dismissUpdate}
                  className="flex-1 bg-white hover:bg-blue-100 text-blue-700 text-sm py-1.5 rounded-lg border border-blue-200 transition-all active:scale-[0.97]"
                >
                  稍后提醒我
                </button>
              )}
              {appUpdater.canDownload && (
                <button
                  type="button"
                  onClick={() => void appUpdater.downloadUpdate()}
                  className="flex-1 bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
                >
                  下载更新
                </button>
              )}
            </div>
          </div>
        )}
        {appUpdater.status === "deferred" && appUpdater.availableUpdate && (
          <div className="bg-amber-50 text-amber-700 text-xs px-2 py-1 rounded">
            {`已忽略 v${appUpdater.availableUpdate.version}，稍后会再次提醒。`}
          </div>
        )}
        {appUpdater.status === "downloading" && (
          <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-3 space-y-1">
            <div className="text-sm font-medium text-blue-700">正在下载更新</div>
            <div className="text-xs text-blue-700">
              {`已下载 ${appUpdater.downloadProgress.percent ?? 0}%`}
            </div>
          </div>
        )}
        {appUpdater.status === "ready_to_install" && appUpdater.availableUpdate && (
          <div className="rounded-lg border border-emerald-100 bg-emerald-50 px-3 py-3 space-y-2">
            <div className="text-sm font-medium text-emerald-700">下载完成，准备安装</div>
            <button
              type="button"
              onClick={() => void appUpdater.installUpdate()}
              className="w-full bg-emerald-500 hover:bg-emerald-600 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
            >
              立即安装更新
            </button>
          </div>
        )}
        {appUpdater.status === "installing" && (
          <div className="bg-blue-50 text-blue-600 text-xs px-2 py-1 rounded">正在安装更新</div>
        )}
        {appUpdater.status === "restart_required" && (
          <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">
            更新安装已完成，重启应用后生效。
          </div>
        )}
        {appUpdater.status === "failed" && (
          <div className="rounded-lg border border-red-100 bg-red-50 px-3 py-3 space-y-2">
            <div className="text-sm font-medium text-red-700">更新失败</div>
            <div className="text-xs text-red-700">{appUpdater.error || "请稍后重试"}</div>
            <button
              type="button"
              onClick={appUpdater.resetFailure}
              className="w-full bg-white hover:bg-red-100 text-red-700 text-sm py-1.5 rounded-lg border border-red-200 transition-all active:scale-[0.97]"
            >
              清除错误状态
            </button>
          </div>
        )}
      </div>

      <div className="bg-white rounded-lg p-4 space-y-3 mt-4">
        <div className="text-xs font-medium text-gray-500">数据与卸载</div>
        {desktopLifecycleLoading && (
          <div className="bg-gray-50 text-gray-500 text-xs px-2 py-1 rounded">正在读取本地目录</div>
        )}
        {desktopLifecyclePaths && (
          <div className="space-y-3">
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-xs font-medium text-gray-500">应用数据目录</div>
              <div className="mt-1 break-all text-xs text-gray-700">
                {desktopLifecyclePaths.app_data_dir}
              </div>
              <button
                type="button"
                onClick={() => void handleOpenDesktopPath(desktopLifecyclePaths.app_data_dir)}
                disabled={desktopLifecycleActionState === "opening"}
                className="mt-2 bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97]"
              >
                打开应用数据目录
              </button>
            </div>
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-xs font-medium text-gray-500">缓存目录</div>
              <div className="mt-1 break-all text-xs text-gray-700">
                {desktopLifecyclePaths.cache_dir}
              </div>
              <button
                type="button"
                onClick={() => void handleOpenDesktopPath(desktopLifecyclePaths.cache_dir)}
                disabled={desktopLifecycleActionState === "opening"}
                className="mt-2 bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97]"
              >
                打开缓存目录
              </button>
            </div>
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-xs font-medium text-gray-500">日志目录</div>
              <div className="mt-1 break-all text-xs text-gray-700">
                {desktopLifecyclePaths.log_dir}
              </div>
              <button
                type="button"
                onClick={() => void handleOpenDesktopPath(desktopLifecyclePaths.log_dir)}
                disabled={desktopLifecycleActionState === "opening"}
                className="mt-2 bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97]"
              >
                打开日志目录
              </button>
            </div>
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-xs font-medium text-gray-500">默认工作目录</div>
              <div className="mt-1 break-all text-xs text-gray-700">
                {desktopLifecyclePaths.default_work_dir || runtimePreferences.default_work_dir || "未设置"}
              </div>
              <button
                type="button"
                onClick={() =>
                  void handleOpenDesktopPath(
                    desktopLifecyclePaths.default_work_dir || runtimePreferences.default_work_dir,
                  )
                }
                disabled={
                  desktopLifecycleActionState === "opening" ||
                  !(
                    desktopLifecyclePaths.default_work_dir || runtimePreferences.default_work_dir
                  ).trim()
                }
                className="mt-2 bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97] disabled:opacity-50"
              >
                打开工作目录
              </button>
            </div>
          </div>
        )}
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => void handleClearDesktopCacheAndLogs()}
            disabled={desktopLifecycleActionState === "clearing"}
            className="flex-1 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {desktopLifecycleActionState === "clearing" ? "清理中..." : "清理缓存与日志"}
          </button>
          <button
            type="button"
            onClick={() => void handleExportDesktopEnvironmentSummary()}
            disabled={desktopLifecycleActionState === "exporting"}
            className="flex-1 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {desktopLifecycleActionState === "exporting" ? "导出中..." : "导出环境摘要"}
          </button>
        </div>
        <div className="rounded-lg border border-amber-100 bg-amber-50 px-3 py-3 text-xs text-amber-700 space-y-1">
          <div>卸载程序不会删除你的工作目录</div>
          <div>如需彻底清理，请先清理缓存与日志，再手动删除应用数据目录。</div>
        </div>
        {desktopLifecycleError && (
          <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">
            {desktopLifecycleError}
          </div>
        )}
        {desktopLifecycleMessage && (
          <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">
            {desktopLifecycleMessage}
          </div>
        )}
      </div>

      {showDevModelSetupTools && (
        <div
          data-testid="model-setup-dev-tools"
          className="mt-4 rounded-2xl border border-amber-200 bg-amber-50/80 p-4"
        >
          <div className="text-xs font-semibold uppercase tracking-[0.14em] text-amber-700">
            Dev Only
          </div>
          <div className="mt-1 text-sm font-medium text-amber-950">首次引导调试入口</div>
          <div className="mt-1 text-xs leading-5 text-amber-800/80">
            用于在开发阶段反复测试首次连接引导，不会在正式环境展示。
          </div>
          <div className="mt-3 grid gap-2 sm:grid-cols-2">
            <button
              type="button"
              onClick={onDevResetFirstUseOnboarding}
              className="sm-btn rounded-xl border border-amber-300 bg-white px-4 py-2 text-sm text-amber-900 hover:bg-amber-100"
            >
              重置首次引导状态
            </button>
            <button
              type="button"
              onClick={onDevOpenQuickModelSetup}
              className="sm-btn rounded-xl border border-amber-300 bg-white px-4 py-2 text-sm text-amber-900 hover:bg-amber-100"
            >
              打开首次配置弹层
            </button>
          </div>
        </div>
      )}
      </>)}

      {SHOW_CAPABILITY_ROUTING_SETTINGS && activeTab === "capabilities" && (
        <div className="bg-white rounded-lg p-4 space-y-3">
          <div className="text-xs font-medium text-gray-500 mb-2">能力路由</div>
          <div>
            <label className={labelCls}>能力类型</label>
            <select
              className={inputCls}
              value={selectedCapability}
              onChange={(e) => {
                const capability = e.target.value;
                setSelectedCapability(capability);
                loadCapabilityRoutingPolicy(capability);
                loadRouteTemplates(capability);
              }}
            >
              {ROUTING_CAPABILITIES.map((c) => (
                <option key={c.value} value={c.value}>{c.label}</option>
              ))}
            </select>
          </div>
          <div>
            <label className={labelCls}>主连接</label>
            <select
              className={inputCls}
              value={chatRoutingPolicy.primary_provider_id}
              onChange={(e) => {
                const providerId = e.target.value;
                setChatRoutingPolicy((s) => ({ ...s, primary_provider_id: providerId }));
                loadChatPrimaryModels(providerId, selectedCapability);
              }}
            >
              <option value="">请选择</option>
              {providers.map((p) => (
                <option key={p.id} value={p.id}>{p.display_name}</option>
              ))}
            </select>
          </div>
          <div>
            <label className={labelCls}>主模型</label>
            <input
              className={inputCls}
              list="chat-primary-models"
              value={chatRoutingPolicy.primary_model}
              onChange={(e) => setChatRoutingPolicy((s) => ({ ...s, primary_model: e.target.value }))}
              placeholder="例如: deepseek-chat / qwen-max / kimi-k2"
            />
            {chatPrimaryModels.length > 0 && (
              <datalist id="chat-primary-models">
                {chatPrimaryModels.map((model) => (
                  <option key={model} value={model} />
                ))}
              </datalist>
            )}
          </div>
          <div>
            <label className={labelCls}>Fallback 链</label>
            <div className="space-y-2">
              {chatFallbackRows.map((row, index) => (
                <div key={index} className="grid grid-cols-[1fr_1fr_auto] gap-2">
                  <select
                    className={inputCls}
                    value={row.provider_id}
                    onChange={(e) => updateFallbackRow(index, { provider_id: e.target.value })}
                  >
                    <option value="">选择连接</option>
                    {providers.map((p) => (
                      <option key={p.id} value={p.id}>{p.display_name}</option>
                    ))}
                  </select>
                  <input
                    className={inputCls}
                    value={row.model}
                    onChange={(e) => updateFallbackRow(index, { model: e.target.value })}
                    placeholder="模型名"
                  />
                  <button
                    onClick={() => removeFallbackRow(index)}
                    className="px-2 text-xs text-red-500 hover:text-red-600"
                  >
                    删除
                  </button>
                </div>
              ))}
              <button
                onClick={addFallbackRow}
                className="text-xs text-blue-500 hover:text-blue-600"
              >
                + 添加回退节点
              </button>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className={labelCls}>超时(ms)</label>
              <input
                className={inputCls}
                type="number"
                value={chatRoutingPolicy.timeout_ms}
                onChange={(e) => setChatRoutingPolicy((s) => ({ ...s, timeout_ms: Number(e.target.value || 60000) }))}
              />
            </div>
            <div>
              <label className={labelCls}>重试次数</label>
              <input
                className={inputCls}
                type="number"
                value={chatRoutingPolicy.retry_count}
                onChange={(e) => setChatRoutingPolicy((s) => ({ ...s, retry_count: Number(e.target.value || 0) }))}
              />
            </div>
          </div>
          <button
            onClick={() => {
              const defaults = getCapabilityRecommendedDefaults(selectedCapability);
              setChatRoutingPolicy((s) => ({
                ...s,
                timeout_ms: defaults.timeout_ms,
                retry_count: defaults.retry_count,
              }));
            }}
            className="w-full bg-gray-100 hover:bg-gray-200 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            应用推荐超时/重试配置
          </button>
          <div className="grid grid-cols-[1fr_auto] gap-2">
            <select
              className={inputCls}
              value={selectedRouteTemplateId}
              onChange={(e) => setSelectedRouteTemplateId(e.target.value)}
            >
              {routeTemplates.length === 0 && <option value="">暂无模板</option>}
              {routeTemplates.map((tpl) => (
                <option key={`${tpl.template_id}-${tpl.capability}`} value={tpl.template_id}>
                  {tpl.name}
                </option>
              ))}
            </select>
            <button
              onClick={handleApplyRouteTemplate}
              disabled={!selectedRouteTemplateId}
              className="bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm px-3 py-1.5 rounded-lg transition-all active:scale-[0.97]"
            >
              应用模板
            </button>
          </div>
          <label className="flex items-center gap-2 text-xs text-gray-600">
            <input
              type="checkbox"
              checked={chatRoutingPolicy.enabled}
              onChange={(e) => setChatRoutingPolicy((s) => ({ ...s, enabled: e.target.checked }))}
            />
            启用当前能力路由
          </label>
          {policyError && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{policyError}</div>}
          {policySaveState === "saved" && <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">已保存</div>}
          <button
            onClick={handleSaveChatPolicy}
            disabled={policySaveState === "saving"}
            className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {policySaveState === "saving" ? "保存中..." : "保存能力路由策略"}
          </button>
        </div>
      )}

      {SHOW_HEALTH_SETTINGS && activeTab === "health" && (
        <div className="bg-white rounded-lg p-4 space-y-3">
          <div className="text-xs font-medium text-gray-500 mb-2">连接健康检查</div>
          <div>
            <label className={labelCls}>选择连接</label>
            <select
              className={inputCls}
              value={healthProviderId}
              onChange={(e) => setHealthProviderId(e.target.value)}
            >
              <option value="">请选择</option>
              {providers.map((p) => (
                <option key={p.id} value={p.id}>{p.display_name}</option>
              ))}
            </select>
          </div>
          <button
            onClick={handleCheckProviderHealth}
            disabled={!healthProviderId || healthLoading}
            className="w-full bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {healthLoading ? "检测中..." : "执行健康检查"}
          </button>
          <button
            onClick={handleCheckAllProviderHealth}
            disabled={healthLoading}
            className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {healthLoading ? "检测中..." : "一键巡检全部连接"}
          </button>
          {healthResult && (
            <div className={"text-xs px-2 py-2 rounded " + (healthResult.ok ? "bg-green-50 text-green-700" : "bg-red-50 text-red-700")}>
              <div>状态: {healthResult.ok ? "正常" : "异常"}</div>
              <div>协议: {healthResult.protocol_type || "-"}</div>
              <div className="break-all">详情: {healthResult.message}</div>
            </div>
          )}
          {allHealthResults.length > 0 && (
            <div className="space-y-2">
              {allHealthResults.map((r, idx) => (
                <div
                  key={`${r.provider_id}-${idx}`}
                  className={"text-xs px-2 py-2 rounded " + (r.ok ? "bg-green-50 text-green-700" : "bg-red-50 text-red-700")}
                >
                  <div>连接ID: {r.provider_id || "-"}</div>
                  <div>状态: {r.ok ? "正常" : "异常"}</div>
                  <div>协议: {r.protocol_type || "-"}</div>
                  <div className="break-all">详情: {r.message}</div>
                </div>
              ))}
            </div>
          )}
          <div className="pt-2 border-t border-gray-100">
            <div className="mb-3">
              <div className="flex items-center justify-between mb-2">
                <div className="text-xs font-medium text-gray-500">路由统计</div>
                <button
                  onClick={loadRouteStats}
                  disabled={routeStatsLoading}
                  className="text-xs text-blue-500 hover:text-blue-600 disabled:opacity-50"
                >
                  {routeStatsLoading ? "刷新中..." : "刷新"}
                </button>
              </div>
              <div className="flex gap-2 mb-2">
                <select
                  className={inputCls}
                  value={String(routeStatsHours)}
                  onChange={(e) => setRouteStatsHours(Number(e.target.value || 24))}
                >
                  <option value="1">最近 1h</option>
                  <option value="24">最近 24h</option>
                  <option value="168">最近 7d</option>
                </select>
                <select
                  className={inputCls}
                  value={routeStatsCapability}
                  onChange={(e) => setRouteStatsCapability(e.target.value)}
                >
                  <option value="all">全部能力</option>
                  <option value="chat">chat</option>
                  <option value="vision">vision</option>
                  <option value="image_gen">image_gen</option>
                  <option value="audio_stt">audio_stt</option>
                  <option value="audio_tts">audio_tts</option>
                </select>
                <button
                  onClick={loadRouteStats}
                  disabled={routeStatsLoading}
                  className="bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs px-3 rounded"
                >
                  应用
                </button>
              </div>
              {routeStats.length === 0 ? (
                <div className="text-xs text-gray-400">暂无统计数据</div>
              ) : (
                <div className="space-y-1">
                  {routeStats.slice(0, 8).map((stat, idx) => (
                    <div key={`${stat.capability}-${stat.error_kind}-${idx}`} className="text-xs bg-gray-50 border border-gray-100 rounded px-2 py-1 text-gray-700">
                      {stat.capability} · {stat.success ? "success" : stat.error_kind || "unknown"} · {stat.count}
                    </div>
                  ))}
                </div>
              )}
            </div>
            <div className="flex items-center justify-between mb-2">
              <div className="text-xs font-medium text-gray-500">最近路由日志</div>
              <button
                onClick={() => {
                  setRouteLogsOffset(0);
                  loadRecentRouteLogs(false);
                }}
                disabled={routeLogsLoading}
                className="text-xs text-blue-500 hover:text-blue-600 disabled:opacity-50"
              >
                {routeLogsLoading ? "刷新中..." : "刷新"}
              </button>
            </div>
            <button
              onClick={handleExportRouteLogsCsv}
              disabled={routeLogsExporting}
              className="w-full mb-2 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs py-1.5 rounded"
            >
              {routeLogsExporting ? "导出中..." : "导出日志 CSV（保存文件并复制到剪贴板）"}
            </button>
            <div className="grid grid-cols-2 gap-2 mb-2">
              <input
                className={inputCls}
                placeholder="按 Session ID 过滤（可选）"
                value={routeLogsSessionId}
                onChange={(e) => setRouteLogsSessionId(e.target.value)}
              />
              <button
                onClick={() => {
                  setRouteLogsOffset(0);
                  loadRecentRouteLogs(false);
                }}
                disabled={routeLogsLoading}
                className="bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs py-1.5 rounded"
              >
                应用过滤
              </button>
              <select
                className={inputCls}
                value={routeLogsCapabilityFilter}
                onChange={(e) => setRouteLogsCapabilityFilter(e.target.value)}
              >
                <option value="all">能力: 全部</option>
                <option value="chat">chat</option>
                <option value="vision">vision</option>
                <option value="image_gen">image_gen</option>
                <option value="audio_stt">audio_stt</option>
                <option value="audio_tts">audio_tts</option>
              </select>
              <select
                className={inputCls}
                value={routeLogsResultFilter}
                onChange={(e) => setRouteLogsResultFilter(e.target.value)}
              >
                <option value="all">结果: 全部</option>
                <option value="success">成功</option>
                <option value="failed">失败</option>
              </select>
              <select
                className={inputCls}
                value={routeLogsErrorKindFilter}
                onChange={(e) => setRouteLogsErrorKindFilter(e.target.value)}
              >
                <option value="all">错误类型: 全部</option>
                <option value="auth">auth</option>
                <option value="rate_limit">rate_limit</option>
                <option value="timeout">timeout</option>
                <option value="network">network</option>
                <option value="unknown">unknown</option>
              </select>
            </div>
            {filteredRouteLogs.length === 0 ? (
              <div className="text-xs text-gray-400">暂无路由日志</div>
            ) : (
              <div className="space-y-2 max-h-72 overflow-y-auto pr-1">
                {filteredRouteLogs.map((log, idx) => (
                  <div
                    key={`${log.created_at}-${idx}`}
                    className={"text-xs rounded px-2 py-2 border " + (log.success ? "bg-green-50 border-green-100 text-green-700" : "bg-red-50 border-red-100 text-red-700")}
                  >
                    <div>{log.created_at}</div>
                    <div>能力: {log.capability} · 协议: {log.api_format}</div>
                    <div>模型: {log.model_name}</div>
                    <div>尝试: #{log.attempt_index} / 重试: {log.retry_index}</div>
                    <div className="flex gap-2 mt-1">
                      <button
                        onClick={() => setRouteLogsSessionId(log.session_id)}
                        className="text-[11px] text-blue-600 hover:text-blue-700"
                      >
                        按此 Session 过滤
                      </button>
                      <button
                        onClick={() => navigator?.clipboard?.writeText?.(log.session_id)}
                        className="text-[11px] text-blue-600 hover:text-blue-700"
                      >
                        复制 Session ID
                      </button>
                      {!log.success && log.error_message && (
                        <button
                          onClick={() => navigator?.clipboard?.writeText?.(log.error_message)}
                          className="text-[11px] text-blue-600 hover:text-blue-700"
                        >
                          复制错误详情
                        </button>
                      )}
                    </div>
                    <div>结果: {log.success ? "成功" : `失败 (${log.error_kind || "unknown"})`}</div>
                    {!log.success && log.error_message && (
                      <div className="break-all">错误: {log.error_message}</div>
                    )}
                  </div>
                ))}
              </div>
            )}
            {routeLogsHasMore && (
              <button
                onClick={() => loadRecentRouteLogs(true)}
                disabled={routeLogsLoading}
                className="w-full mt-2 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs py-1.5 rounded"
              >
                {routeLogsLoading ? "加载中..." : "加载更多"}
              </button>
            )}
          </div>
        </div>
      )}

      {SHOW_MCP_SETTINGS && activeTab === "mcp" && (<>
      {/* MCP 服务器管理 */}
      <div className="bg-white rounded-lg p-4 space-y-3">
        <div className="text-xs font-medium text-gray-500 mb-2">MCP 服务器</div>

        {mcpServers.length > 0 && (
          <div className="space-y-2 mb-3">
            {mcpServers.map((s) => (
              <div key={s.id} className="flex items-center justify-between bg-gray-100 rounded px-3 py-2 text-sm">
                <div>
                  <span className="font-medium">{s.name}</span>
                  <span className="text-gray-500 ml-2 text-xs">{s.command} {s.args?.join(" ")}</span>
                </div>
                <button onClick={() => handleRemoveMcp(s.id)} className="text-red-400 hover:text-red-300 text-xs">
                  删除
                </button>
              </div>
            ))}
          </div>
        )}

        <div>
          <label className={labelCls}>快速选择 MCP 服务器</label>
          <select
            className={inputCls}
            defaultValue=""
            onChange={(e) => applyMcpPreset(e.target.value)}
          >
            {MCP_PRESETS.map((p) => (
              <option key={p.value} value={p.value}>{p.label}</option>
            ))}
          </select>
        </div>
        <div>
          <label className={labelCls}>名称</label>
          <input className={inputCls} placeholder="例: filesystem" value={mcpForm.name} onChange={(e) => setMcpForm({ ...mcpForm, name: e.target.value })} />
        </div>
        <div>
          <label className={labelCls}>命令</label>
          <input className={inputCls} placeholder="例: npx" value={mcpForm.command} onChange={(e) => setMcpForm({ ...mcpForm, command: e.target.value })} />
        </div>
        <div>
          <label className={labelCls}>参数（空格分隔）</label>
          <input className={inputCls} placeholder="例: @anthropic/mcp-server-filesystem /tmp" value={mcpForm.args} onChange={(e) => setMcpForm({ ...mcpForm, args: e.target.value })} />
        </div>
        {mcpApiKeyEnvKeys.map((envKey) => (
          <div key={envKey}>
            <label className={labelCls}>API Key（可选）</label>
            <input
              className={inputCls}
              type="password"
              placeholder={`请输入 ${envKey}`}
              value={parsedMcpEnv.env[envKey] || ""}
              onChange={(e) => updateMcpEnvField(envKey, e.target.value)}
            />
            <div className="text-[11px] text-gray-400 mt-1">变量名：{envKey}</div>
          </div>
        ))}
        <div className="space-y-2">
          <button
            type="button"
            onClick={() => setShowMcpEnvJson((v) => !v)}
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
                onChange={(e) => setMcpForm({ ...mcpForm, env: e.target.value })}
              />
              {parsedMcpEnv.error && (
                <div className="text-[11px] text-red-500 mt-1">{parsedMcpEnv.error}</div>
              )}
            </div>
          )}
        </div>
        {mcpError && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{mcpError}</div>}
        <button
          onClick={handleAddMcp}
          disabled={!mcpForm.name || !mcpForm.command}
          className="w-full bg-blue-500 hover:bg-blue-600 disabled:bg-gray-200 disabled:text-gray-400 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
        >
          添加 MCP 服务器
        </button>
      </div>
      </>)}

      {activeTab === "search" && (<>
        {searchConfigs.length > 0 && (
          <div className="mb-6 space-y-2">
            <div className="text-xs text-gray-500 mb-2">已配置搜索引擎</div>
            {searchConfigs.map((s) => (
              <div
                key={s.id}
                className={
                  "flex items-center justify-between bg-white rounded-lg px-4 py-2.5 text-sm border transition-colors " +
                  (editingSearchId === s.id ? "border-blue-400 ring-1 ring-blue-400" : "border-transparent hover:border-gray-200")
                }
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-gray-800">{s.name}</span>
                    {s.is_default && (
                      <span className="text-[10px] bg-blue-500 text-white px-1.5 py-0.5 rounded">默认</span>
                    )}
                  </div>
                  <div className="text-xs text-gray-400 mt-0.5 truncate">
                    {s.api_format.replace("search_", "")} · {s.base_url}
                  </div>
                </div>
                <div className="flex items-center gap-2 flex-shrink-0 ml-3">
                  {!s.is_default && (
                    <button onClick={() => handleSetDefaultSearch(s.id)} className="text-blue-400 hover:text-blue-500 text-xs">
                      设为默认
                    </button>
                  )}
                  <button onClick={() => handleEditSearch(s)} className="text-blue-500 hover:text-blue-600 text-xs">
                    编辑
                  </button>
                  <button onClick={() => handleDeleteSearch(s.id)} className="text-red-400 hover:text-red-500 text-xs">
                    删除
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        <div className="bg-white rounded-lg p-4 space-y-3">
          <div className="flex items-center justify-between mb-2">
            <div className="text-xs font-medium text-gray-500">
              {editingSearchId ? "编辑搜索引擎" : "添加搜索引擎"}
            </div>
            {editingSearchId && (
              <button
                onClick={() => {
                  setEditingSearchId(null);
                  setShowSearchApiKey(false);
                  setSearchForm({ name: "", api_format: "", base_url: "", model_name: "", api_key: "" });
                  setSearchError("");
                  setSearchTestResult(null);
                }}
                className="text-xs text-gray-400 hover:text-gray-600"
              >
                取消编辑
              </button>
            )}
          </div>
          <div>
            <label className={labelCls}>快速选择搜索引擎</label>
            <select className={inputCls} defaultValue="" onChange={(e) => applySearchPreset(e.target.value)}>
              {SEARCH_PRESETS.map((p) => (
                <option key={p.value} value={p.value}>{p.label}</option>
              ))}
            </select>
          </div>
          <div>
            <label className={labelCls}>名称</label>
            <input className={inputCls} value={searchForm.name} onChange={(e) => setSearchForm({ ...searchForm, name: e.target.value })} />
          </div>
          <div>
            <label className={labelCls}>API Key</label>
            <div className="relative">
              <input
                className={inputCls + " pr-10"}
                type={showSearchApiKey ? "text" : "password"}
                value={searchForm.api_key}
                onChange={(e) => setSearchForm({ ...searchForm, api_key: e.target.value })}
              />
              <button
                type="button"
                onClick={() => setShowSearchApiKey(!showSearchApiKey)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 p-1"
                title={showSearchApiKey ? "隐藏" : "显示"}
              >
                {showSearchApiKey ? <EyeSlashIcon /> : <EyeOpenIcon />}
              </button>
            </div>
          </div>
          <div>
            <label className={labelCls}>Base URL（可选自定义）</label>
            <input className={inputCls} value={searchForm.base_url} onChange={(e) => setSearchForm({ ...searchForm, base_url: e.target.value })} />
          </div>
          {searchForm.api_format === "search_serpapi" && (
            <div>
              <label className={labelCls}>搜索引擎 (google/baidu/bing)</label>
              <input className={inputCls} value={searchForm.model_name} onChange={(e) => setSearchForm({ ...searchForm, model_name: e.target.value })} />
            </div>
          )}
          {searchError && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{searchError}</div>}
          {searchTestResult !== null && (
            <div className={"text-xs px-2 py-1 rounded " + (searchTestResult ? "bg-green-50 text-green-600" : "bg-red-50 text-red-600")}>
              {searchTestResult ? "连接成功" : "连接失败，请检查配置"}
            </div>
          )}
          <div className="flex gap-2 pt-1">
            <button
              onClick={handleTestSearch}
              disabled={searchTesting || !searchForm.api_format}
              className="flex-1 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
            >
              {searchTesting ? "测试中..." : "测试连接"}
            </button>
            <button
              onClick={handleSaveSearch}
              disabled={!searchForm.name || !searchForm.api_format || !searchForm.api_key}
              className="flex-1 bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
            >
              {editingSearchId ? "保存修改" : "保存"}
            </button>
          </div>
        </div>
      </>)}

      {SHOW_AUTO_ROUTING_SETTINGS && activeTab === "routing" && (
        <div className="bg-white rounded-lg p-4 space-y-3">
          <div className="text-xs font-medium text-gray-500 mb-2">子 Skill 自动路由</div>
          <div>
            <label className={labelCls}>最大调用深度 (2-8)</label>
            <input
              className={inputCls}
              type="number"
              min={2}
              max={8}
              value={routeSettings.max_call_depth}
              onChange={(e) => setRouteSettings((s) => ({ ...s, max_call_depth: Number(e.target.value || 4) }))}
            />
          </div>
          <div>
            <label className={labelCls}>节点超时秒数 (5-600)</label>
            <input
              className={inputCls}
              type="number"
              min={5}
              max={600}
              value={routeSettings.node_timeout_seconds}
              onChange={(e) => setRouteSettings((s) => ({ ...s, node_timeout_seconds: Number(e.target.value || 60) }))}
            />
          </div>
          <div>
            <label className={labelCls}>失败重试次数 (0-2)</label>
            <input
              className={inputCls}
              type="number"
              min={0}
              max={2}
              value={routeSettings.retry_count}
              onChange={(e) => setRouteSettings((s) => ({ ...s, retry_count: Number(e.target.value || 0) }))}
            />
          </div>
          {routeError && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{routeError}</div>}
          {routeSaveState === "saved" && (
            <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">已保存</div>
          )}
          <button
            onClick={handleSaveRoutingSettings}
            disabled={routeSaveState === "saving"}
            className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {routeSaveState === "saving" ? "保存中..." : "保存自动路由设置"}
          </button>
        </div>
      )}

    </div>
  );
}
