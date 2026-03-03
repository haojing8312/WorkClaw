import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  CapabilityRouteTemplateInfo,
  CapabilityRoutingPolicy,
  AgentEmployee,
  FeishuChatInfo,
  FeishuEventRelayStatus,
  FeishuGatewaySettings,
  FeishuWsStatus,
  ModelConfig,
  ProviderConfig,
  ProviderHealthInfo,
  ProviderPluginInfo,
  RecentImThread,
  RouteAttemptLog,
  RouteAttemptStat,
  ThreadEmployeeBinding,
  ThreadRoleConfig,
  RuntimePreferences,
  SkillManifest,
  UpsertAgentEmployeeInput,
} from "../types";

const MCP_PRESETS = [
  { label: "— 快速选择 —", value: "", name: "", command: "", args: "", env: "" },
  { label: "Filesystem", value: "filesystem", name: "filesystem", command: "npx", args: "-y @anthropic/mcp-server-filesystem /tmp", env: "" },
  { label: "Brave Search", value: "brave-search", name: "brave-search", command: "npx", args: "-y @anthropic/mcp-server-brave-search", env: '{"BRAVE_API_KEY": ""}' },
  { label: "Memory", value: "memory", name: "memory", command: "npx", args: "-y @anthropic/mcp-server-memory", env: "" },
  { label: "Puppeteer", value: "puppeteer", name: "puppeteer", command: "npx", args: "-y @anthropic/mcp-server-puppeteer", env: "" },
  { label: "Fetch", value: "fetch", name: "fetch", command: "npx", args: "-y @anthropic/mcp-server-fetch", env: "" },
];

const PROVIDER_PRESETS = [
  { label: "— 快速选择 —", value: "", models: [] as string[] },
  { label: "OpenAI", value: "openai", api_format: "openai", base_url: "https://api.openai.com/v1", model_name: "gpt-4o-mini", models: ["gpt-4o", "gpt-4o-mini", "gpt-4.1", "gpt-4.1-mini", "gpt-4.1-nano", "o3-mini"] },
  { label: "Claude (Anthropic)", value: "anthropic", api_format: "anthropic", base_url: "https://api.anthropic.com/v1", model_name: "claude-3-5-haiku-20241022", models: ["claude-sonnet-4-5-20250929", "claude-3-5-haiku-20241022", "claude-3-5-sonnet-20241022"] },
  { label: "MiniMax (OpenAI 兼容)", value: "minimax-oai", api_format: "openai", base_url: "https://api.minimax.io/v1", model_name: "MiniMax-M2.5", models: ["MiniMax-M2.5", "MiniMax-M1", "MiniMax-Text-01"] },
  { label: "MiniMax (Anthropic 兼容)", value: "minimax-ant", api_format: "anthropic", base_url: "https://api.minimax.io/anthropic/v1", model_name: "MiniMax-M2.5", models: ["MiniMax-M2.5", "MiniMax-M1", "MiniMax-Text-01"] },
  { label: "DeepSeek", value: "deepseek", api_format: "openai", base_url: "https://api.deepseek.com/v1", model_name: "deepseek-chat", models: ["deepseek-chat", "deepseek-reasoner"] },
  { label: "Qwen (国际)", value: "qwen-intl", api_format: "openai", base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1", model_name: "qwen-max", models: ["qwen-max", "qwen-plus", "qwen-turbo", "qwen-long", "qwen-vl-max", "qwen-vl-plus"] },
  { label: "Qwen (国内)", value: "qwen-cn", api_format: "openai", base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1", model_name: "qwen-max", models: ["qwen-max", "qwen-plus", "qwen-turbo", "qwen-long", "qwen-vl-max", "qwen-vl-plus"] },
  { label: "Moonshot / Kimi", value: "moonshot", api_format: "openai", base_url: "https://api.moonshot.ai/v1", model_name: "kimi-k2", models: ["kimi-k2", "moonshot-v1-8k", "moonshot-v1-32k", "moonshot-v1-128k"] },
  { label: "Yi", value: "yi", api_format: "openai", base_url: "https://api.lingyiwanwu.com/v1", model_name: "yi-large", models: ["yi-large", "yi-medium", "yi-spark"] },
  { label: "自定义", value: "custom", models: [] as string[] },
];

const SEARCH_PRESETS = [
  { label: "— 快速选择 —", value: "", api_format: "", base_url: "", model_name: "" },
  { label: "Brave Search (国际首选)", value: "brave", api_format: "search_brave", base_url: "https://api.search.brave.com", model_name: "" },
  { label: "Tavily (AI 专用)", value: "tavily", api_format: "search_tavily", base_url: "https://api.tavily.com", model_name: "" },
  { label: "秘塔搜索 (中文首选)", value: "metaso", api_format: "search_metaso", base_url: "https://metaso.cn", model_name: "" },
  { label: "博查搜索 (中文 AI)", value: "bocha", api_format: "search_bocha", base_url: "https://api.bochaai.com", model_name: "" },
  { label: "SerpAPI (多引擎)", value: "serpapi", api_format: "search_serpapi", base_url: "https://serpapi.com", model_name: "google" },
];

interface Props {
  onClose: () => void;
}

interface RoutingSettings {
  max_call_depth: number;
  node_timeout_seconds: number;
  retry_count: number;
}

const PROVIDER_PROTOCOL_OPTIONS = [
  { label: "OpenAI 兼容", value: "openai" },
  { label: "Anthropic 兼容", value: "anthropic" },
];

const ROUTING_CAPABILITIES = [
  { label: "对话 Chat", value: "chat" },
  { label: "视觉 Vision", value: "vision" },
  { label: "生图 Image", value: "image_gen" },
  { label: "语音转写 STT", value: "audio_stt" },
  { label: "语音合成 TTS", value: "audio_tts" },
];

const EMPLOYEE_ROLE_TEMPLATES: Array<{ name: string; role_id: string; persona: string }> = [
  {
    name: "项目经理",
    role_id: "project_manager",
    persona: "负责需求澄清、任务拆解、里程碑推进与风险管理，优先输出可执行计划与验收标准。",
  },
  {
    name: "技术负责人",
    role_id: "tech_lead",
    persona: "负责技术方案评审、架构决策和质量把关，强调可维护性、测试覆盖和交付稳定性。",
  },
  {
    name: "运营专员",
    role_id: "operations",
    persona: "负责运营数据分析、活动复盘与流程优化，输出可落地行动项和指标跟踪方案。",
  },
  {
    name: "客服专员",
    role_id: "customer_success",
    persona: "负责用户问题分级、解决路径设计与满意度提升，提供清晰且可执行的处理建议。",
  },
];

export function SettingsView({ onClose }: Props) {
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [form, setForm] = useState({
    name: "",
    api_format: "openai",
    base_url: "https://api.openai.com/v1",
    model_name: "gpt-4o-mini",
    api_key: "",
  });
  const [error, setError] = useState("");
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<boolean | null>(null);
  const [modelSuggestions, setModelSuggestions] = useState<string[]>([]);

  // 编辑状态 + API Key 可见性
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [showApiKey, setShowApiKey] = useState(false);

  // MCP 服务器管理
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [mcpServers, setMcpServers] = useState<any[]>([]);
  const [mcpForm, setMcpForm] = useState({ name: "", command: "", args: "", env: "" });
  const [mcpError, setMcpError] = useState("");
  const [activeTab, setActiveTab] = useState<
    "models" | "providers" | "capabilities" | "health" | "mcp" | "search" | "routing" | "feishu"
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

  const [providerPlugins, setProviderPlugins] = useState<ProviderPluginInfo[]>([]);
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [providerForm, setProviderForm] = useState<ProviderConfig>({
    id: "",
    provider_key: "deepseek",
    display_name: "DeepSeek",
    protocol_type: "openai",
    base_url: "https://api.deepseek.com/v1",
    auth_type: "api_key",
    api_key_encrypted: "",
    org_id: "",
    extra_json: "{}",
    enabled: true,
  });
  const [providerError, setProviderError] = useState("");
  const [editingProviderId, setEditingProviderId] = useState<string | null>(null);

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

  const [feishuSettings, setFeishuSettings] = useState<FeishuGatewaySettings>({
    app_id: "",
    app_secret: "",
    ingress_token: "",
    encrypt_key: "",
    sidecar_base_url: "http://localhost:8765",
  });
  const [feishuWsStatus, setFeishuWsStatus] = useState<FeishuWsStatus | null>(null);
  const [feishuRelayStatus, setFeishuRelayStatus] = useState<FeishuEventRelayStatus | null>(null);
  const [feishuChats, setFeishuChats] = useState<FeishuChatInfo[]>([]);
  const [recentThreads, setRecentThreads] = useState<RecentImThread[]>([]);
  const [selectedThreadId, setSelectedThreadId] = useState("");
  const [threadRoleConfig, setThreadRoleConfig] = useState<ThreadRoleConfig | null>(null);
  const [threadEmployeeBinding, setThreadEmployeeBinding] = useState<ThreadEmployeeBinding | null>(null);
  const [employees, setEmployees] = useState<AgentEmployee[]>([]);
  const [employeeSkillOptions, setEmployeeSkillOptions] = useState<SkillManifest[]>([]);
  const [selectedEmployeeId, setSelectedEmployeeId] = useState("");
  const [threadEmployeeIdsInput, setThreadEmployeeIdsInput] = useState("");
  const [employeeForm, setEmployeeForm] = useState<UpsertAgentEmployeeInput>({
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
  });
  const [globalDefaultWorkDir, setGlobalDefaultWorkDir] = useState("");
  const [roleTenantId, setRoleTenantId] = useState("default");
  const [roleScenarioTemplate, setRoleScenarioTemplate] = useState("opportunity_review");
  const [roleIdsInput, setRoleIdsInput] = useState(
    "presales,project_manager,business_consultant,architect"
  );
  const [feishuOpMessage, setFeishuOpMessage] = useState("");

  useEffect(() => {
    loadModels();
    loadMcpServers();
    loadSearchConfigs();
    loadRoutingSettings();
    loadProviderPlugins();
    loadProviderConfigs();
    loadCapabilityRoutingPolicy("chat");
    loadRouteTemplates("chat");
  }, []);

  useEffect(() => {
    if (chatRoutingPolicy.primary_provider_id) {
      loadChatPrimaryModels(chatRoutingPolicy.primary_provider_id, selectedCapability);
    }
  }, [chatRoutingPolicy.primary_provider_id, selectedCapability]);

  useEffect(() => {
    if (activeTab === "health") {
      loadRecentRouteLogs(false);
      loadRouteStats();
    }
  }, [activeTab]);

  useEffect(() => {
    if (activeTab === "feishu") {
      refreshFeishuConsole();
    }
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "feishu" || !selectedThreadId) return;
    loadThreadRoleConfig(selectedThreadId);
    loadThreadEmployeeBinding(selectedThreadId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTab, selectedThreadId]);

  async function loadModels() {
    const list = await invoke<ModelConfig[]>("list_model_configs");
    setModels(list);
  }

  async function loadFeishuSettings() {
    const settings = await invoke<FeishuGatewaySettings>("get_feishu_gateway_settings");
    setFeishuSettings(settings);
  }

  async function loadFeishuStatuses() {
    const [ws, relay] = await Promise.all([
      invoke<FeishuWsStatus>("get_feishu_long_connection_status", { sidecarBaseUrl: null }),
      invoke<FeishuEventRelayStatus>("get_feishu_event_relay_status"),
    ]);
    setFeishuWsStatus(ws);
    setFeishuRelayStatus(relay);
  }

  async function loadFeishuChats() {
    const result = await invoke<{ items: FeishuChatInfo[] }>("list_feishu_chats", {
      pageSize: 50,
      pageToken: null,
      userIdType: "open_id",
      appId: null,
      appSecret: null,
      sidecarBaseUrl: null,
    });
    const items = Array.isArray(result?.items) ? result.items : [];
    setFeishuChats(items);
    if (!selectedThreadId && items.length > 0) {
      setSelectedThreadId(items[0].chat_id);
    }
  }

  async function loadRecentThreads() {
    const items = await invoke<RecentImThread[]>("list_recent_im_threads", { limit: 30 });
    setRecentThreads(items || []);
    if (!selectedThreadId && items && items.length > 0) {
      setSelectedThreadId(items[0].thread_id);
    }
  }

  async function loadAgentEmployees() {
    const list = await invoke<AgentEmployee[]>("list_agent_employees");
    setEmployees(list || []);
    if (!selectedEmployeeId && list.length > 0) {
      setSelectedEmployeeId(list[0].id);
    }
  }

  async function loadEmployeeSkillOptions() {
    const list = await invoke<SkillManifest[]>("list_skills");
    setEmployeeSkillOptions((list || []).filter((x) => x.id !== "builtin-general"));
  }

  async function loadRuntimePreferences() {
    const prefs = await invoke<RuntimePreferences>("get_runtime_preferences");
    setGlobalDefaultWorkDir(prefs?.default_work_dir || "");
  }

  async function loadThreadEmployeeBinding(threadId: string) {
    if (!threadId.trim()) {
      setThreadEmployeeBinding(null);
      setThreadEmployeeIdsInput("");
      return;
    }
    try {
      const binding = await invoke<ThreadEmployeeBinding>("get_thread_employee_bindings", { threadId });
      setThreadEmployeeBinding(binding);
      setThreadEmployeeIdsInput((binding.employee_ids || []).join(","));
    } catch {
      setThreadEmployeeBinding(null);
      setThreadEmployeeIdsInput("");
    }
  }

  async function loadThreadRoleConfig(threadId: string) {
    if (!threadId.trim()) {
      setThreadRoleConfig(null);
      return;
    }
    try {
      const cfg = await invoke<ThreadRoleConfig>("get_thread_role_config", { threadId });
      setThreadRoleConfig(cfg);
      setRoleTenantId(cfg.tenant_id || "default");
      setRoleScenarioTemplate(cfg.scenario_template || "opportunity_review");
      if (cfg.roles && cfg.roles.length > 0) {
        setRoleIdsInput(cfg.roles.join(","));
      }
    } catch {
      setThreadRoleConfig(null);
    }
  }

  async function refreshFeishuConsole() {
    setFeishuOpMessage("");
    try {
      await Promise.all([
        loadFeishuSettings(),
        loadFeishuStatuses(),
        loadFeishuChats(),
        loadRecentThreads(),
        loadAgentEmployees(),
        loadEmployeeSkillOptions(),
        loadRuntimePreferences(),
      ]);
      if (selectedThreadId) {
        await Promise.all([loadThreadRoleConfig(selectedThreadId), loadThreadEmployeeBinding(selectedThreadId)]);
      }
    } catch (e) {
      setFeishuOpMessage("飞书控制台加载失败: " + String(e));
    }
  }

  async function handleSaveFeishuSettings() {
    setFeishuOpMessage("");
    try {
      await invoke("set_feishu_gateway_settings", { settings: feishuSettings });
      setFeishuOpMessage("飞书配置已保存");
    } catch (e) {
      setFeishuOpMessage("保存飞书配置失败: " + String(e));
    }
  }

  async function handleVerifyFeishuConnection() {
    setFeishuOpMessage("");
    try {
      await invoke("set_feishu_gateway_settings", { settings: feishuSettings });
      await invoke<FeishuWsStatus>("start_feishu_long_connection", {
        sidecarBaseUrl: null,
        appId: null,
        appSecret: null,
      });
      await invoke<FeishuEventRelayStatus>("start_feishu_event_relay", {
        sidecarBaseUrl: null,
        intervalMs: 1500,
        limit: 50,
      });
      await loadFeishuStatuses();
      await loadFeishuChats();
      await loadRecentThreads();
      setFeishuOpMessage("配置已保存，连接正常");
    } catch (e) {
      setFeishuOpMessage("连接校验失败: " + String(e));
    }
  }

  async function handleStartFeishuLongConnection() {
    setFeishuOpMessage("");
    try {
      const ws = await invoke<FeishuWsStatus>("start_feishu_long_connection", {
        sidecarBaseUrl: null,
        appId: null,
        appSecret: null,
      });
      setFeishuWsStatus(ws);
      setFeishuOpMessage("飞书长连接已启动");
    } catch (e) {
      setFeishuOpMessage("启动长连接失败: " + String(e));
    }
  }

  async function handleBindThreadRoles() {
    if (!selectedThreadId.trim()) {
      setFeishuOpMessage("请先选择线程");
      return;
    }
    const roles = roleIdsInput
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);
    if (roles.length === 0) {
      setFeishuOpMessage("角色列表不能为空");
      return;
    }
    try {
      await invoke("bind_thread_roles", {
        threadId: selectedThreadId,
        tenantId: roleTenantId.trim() || "default",
        scenarioTemplate: roleScenarioTemplate.trim() || "opportunity_review",
        roles,
      });
      setFeishuOpMessage("线程角色绑定已保存");
      await loadThreadRoleConfig(selectedThreadId);
    } catch (e) {
      setFeishuOpMessage("保存线程角色绑定失败: " + String(e));
    }
  }

  async function handleSaveEmployee() {
    try {
      const payload: UpsertAgentEmployeeInput = {
        ...employeeForm,
        skill_ids: employeeForm.skill_ids.filter((x) => x.trim().length > 0),
      };
      const id = await invoke<string>("upsert_agent_employee", { input: payload });
      setSelectedEmployeeId(id);
      setFeishuOpMessage("员工配置已保存");
      await loadAgentEmployees();
    } catch (e) {
      setFeishuOpMessage("保存员工失败: " + String(e));
    }
  }

  async function handleSaveGlobalDefaultWorkDir() {
    try {
      if (!globalDefaultWorkDir.trim()) {
        setFeishuOpMessage("默认工作目录不能为空");
        return;
      }
      await invoke("set_runtime_preferences", {
        input: { default_work_dir: globalDefaultWorkDir.trim() },
      });
      const resolved = await invoke<string>("resolve_default_work_dir");
      setGlobalDefaultWorkDir(resolved);
      setFeishuOpMessage("默认工作目录已保存");
    } catch (e) {
      setFeishuOpMessage("保存默认工作目录失败: " + String(e));
    }
  }

  function applyEmployeeRoleTemplate(roleId: string) {
    const tpl = EMPLOYEE_ROLE_TEMPLATES.find((x) => x.role_id === roleId);
    if (!tpl) return;
    setEmployeeForm((s) => ({
      ...s,
      role_id: tpl.role_id,
      persona: tpl.persona,
    }));
  }

  async function handleDeleteEmployee() {
    if (!selectedEmployeeId) return;
    try {
      await invoke("delete_agent_employee", { employeeId: selectedEmployeeId });
      setSelectedEmployeeId("");
      setEmployeeForm({
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
      });
      setFeishuOpMessage("员工已删除");
      await loadAgentEmployees();
    } catch (e) {
      setFeishuOpMessage("删除员工失败: " + String(e));
    }
  }

  function handlePickEmployee(employeeId: string) {
    setSelectedEmployeeId(employeeId);
    const employee = employees.find((x) => x.id === employeeId);
    if (!employee) return;
    setEmployeeForm({
      id: employee.id,
      name: employee.name,
      role_id: employee.role_id,
      persona: employee.persona,
      feishu_open_id: employee.feishu_open_id,
      feishu_app_id: employee.feishu_app_id,
      feishu_app_secret: employee.feishu_app_secret,
      primary_skill_id: employee.primary_skill_id || "",
      default_work_dir: employee.default_work_dir,
      enabled: employee.enabled,
      is_default: employee.is_default,
      skill_ids: employee.skill_ids.length > 0 ? employee.skill_ids : [],
    });
  }

  async function handleBindThreadEmployees() {
    if (!selectedThreadId.trim()) {
      setFeishuOpMessage("请先选择线程");
      return;
    }
    const employeeIds = threadEmployeeIdsInput
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);
    try {
      await invoke("bind_thread_employees", {
        threadId: selectedThreadId,
        employeeIds,
      });
      setFeishuOpMessage("线程员工绑定已保存");
      await loadThreadEmployeeBinding(selectedThreadId);
    } catch (e) {
      setFeishuOpMessage("保存线程员工绑定失败: " + String(e));
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

  async function loadRoutingSettings() {
    try {
      const settings = await invoke<RoutingSettings>("get_routing_settings");
      setRouteSettings(settings);
    } catch (e) {
      setRouteError("加载自动路由设置失败: " + String(e));
      setRouteSaveState("error");
    }
  }

  async function loadProviderPlugins() {
    try {
      const list = await invoke<ProviderPluginInfo[]>("list_builtin_provider_plugins");
      setProviderPlugins(list);
    } catch (e) {
      setProviderError("加载 Provider 插件失败: " + String(e));
    }
  }

  async function loadProviderConfigs() {
    try {
      const list = await invoke<ProviderConfig[]>("list_provider_configs");
      setProviders(list);
      if (!healthProviderId && list.length > 0) {
        setHealthProviderId(list[0].id);
      }
    } catch (e) {
      setProviderError("加载 Provider 配置失败: " + String(e));
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

  async function handleSaveProvider() {
    setProviderError("");
    try {
      const id = await invoke<string>("save_provider_config", {
        config: {
          ...providerForm,
          id: editingProviderId || providerForm.id,
        },
      });
      setEditingProviderId(null);
      setProviderForm((prev) => ({ ...prev, id, api_key_encrypted: "" }));
      await loadProviderConfigs();
    } catch (e) {
      setProviderError("保存 Provider 配置失败: " + String(e));
    }
  }

  function handleEditProvider(p: ProviderConfig) {
    setEditingProviderId(p.id);
    setProviderForm(p);
    setProviderError("");
  }

  async function handleDeleteProvider(id: string) {
    try {
      await invoke("delete_provider_config", { providerId: id });
      if (editingProviderId === id) {
        setEditingProviderId(null);
      }
      await loadProviderConfigs();
    } catch (e) {
      setProviderError("删除 Provider 配置失败: " + String(e));
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
        missingText = `；缺少 Provider Key（任选其一）: ${match[1]}`;
      }
      setPolicyError(`应用路由模板失败: ${raw}${missingText}；当前已启用: ${enabledText}。请先到 Providers 页补齐并启用。`);
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
      setForm({
        name: m.name,
        api_format: m.api_format,
        base_url: m.base_url,
        model_name: m.model_name,
        api_key: apiKey,
      });
      setEditingModelId(m.id);
      setShowApiKey(false);
      setError("");
      setTestResult(null);
      // 更新模型建议列表
      const preset = PROVIDER_PRESETS.find((p) => p.api_format === m.api_format && p.base_url === m.base_url);
      setModelSuggestions(preset?.models || []);
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
    setError("");
    try {
      await invoke("save_model_config", {
        config: {
          id: editingModelId || "",
          name: form.name,
          api_format: form.api_format,
          base_url: form.base_url,
          model_name: form.model_name,
          is_default: editingModelId
            ? models.find((m) => m.id === editingModelId)?.is_default ?? false
            : models.length === 0,
        },
        apiKey: form.api_key,
      });
      setForm({ name: "", api_format: "openai", base_url: "https://api.openai.com/v1", model_name: "gpt-4o-mini", api_key: "" });
      setEditingModelId(null);
      setShowApiKey(false);
      loadModels();
    } catch (e: unknown) {
      setError(String(e));
    }
  }

  async function handleTest() {
    setTesting(true);
    setTestResult(null);
    try {
      const ok = await invoke<boolean>("test_connection_cmd", {
        config: {
          id: "",
          name: form.name,
          api_format: form.api_format,
          base_url: form.base_url,
          model_name: form.model_name,
          is_default: false,
        },
        apiKey: form.api_key,
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
    const preset = PROVIDER_PRESETS.find((p) => p.value === value);
    if (!preset || !preset.api_format) {
      setModelSuggestions([]);
      return;
    }
    setForm((f) => ({
      ...f,
      api_format: preset.api_format!,
      base_url: preset.base_url!,
      model_name: preset.model_name!,
    }));
    setModelSuggestions(preset.models);
  }

  function applyMcpPreset(value: string) {
    const preset = MCP_PRESETS.find((p) => p.value === value);
    if (!preset || !preset.value) return;
    setMcpForm({
      name: preset.name,
      command: preset.command,
      args: preset.args,
      env: preset.env,
    });
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
    // 若删除的是当前编辑项，重置表单
    if (editingModelId === id) {
      setEditingModelId(null);
      setShowApiKey(false);
      setForm({ name: "", api_format: "openai", base_url: "https://api.openai.com/v1", model_name: "gpt-4o-mini", api_key: "" });
      setError("");
      setTestResult(null);
    }
    loadModels();
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
      let env: Record<string, string> = {};
      if (mcpForm.env.trim()) {
        try {
          env = JSON.parse(mcpForm.env.trim());
        } catch {
          setMcpError("环境变量 JSON 格式错误");
          return;
        }
      }
      await invoke("add_mcp_server", {
        name: mcpForm.name,
        command: mcpForm.command,
        args,
        env,
      });
      setMcpForm({ name: "", command: "", args: "", env: "" });
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

  const inputCls = "w-full bg-gray-50 border border-gray-200 rounded px-3 py-1.5 text-sm focus:outline-none focus:border-blue-400 focus:ring-1 focus:ring-blue-400";
  const labelCls = "block text-xs text-gray-500 mb-1";

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
    <div className="flex flex-col h-full p-6 overflow-y-auto">
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-4">
          <button
            onClick={() => setActiveTab("models")}
            className={"text-sm font-medium pb-1 border-b-2 transition-colors " +
              (activeTab === "models" ? "text-gray-800 border-blue-500" : "text-gray-500 border-transparent hover:text-gray-700")}
          >
            模型配置
          </button>
          <button
            onClick={() => setActiveTab("providers")}
            className={"text-sm font-medium pb-1 border-b-2 transition-colors " +
              (activeTab === "providers" ? "text-gray-800 border-blue-500" : "text-gray-500 border-transparent hover:text-gray-700")}
          >
            Providers
          </button>
          <button
            onClick={() => setActiveTab("capabilities")}
            className={"text-sm font-medium pb-1 border-b-2 transition-colors " +
              (activeTab === "capabilities" ? "text-gray-800 border-blue-500" : "text-gray-500 border-transparent hover:text-gray-700")}
          >
            能力路由
          </button>
          <button
            onClick={() => setActiveTab("health")}
            className={"text-sm font-medium pb-1 border-b-2 transition-colors " +
              (activeTab === "health" ? "text-gray-800 border-blue-500" : "text-gray-500 border-transparent hover:text-gray-700")}
          >
            健康检查
          </button>
          <button
            onClick={() => setActiveTab("mcp")}
            className={"text-sm font-medium pb-1 border-b-2 transition-colors " +
              (activeTab === "mcp" ? "text-gray-800 border-blue-500" : "text-gray-500 border-transparent hover:text-gray-700")}
          >
            MCP 服务器
          </button>
          <button
            onClick={() => setActiveTab("search")}
            className={"text-sm font-medium pb-1 border-b-2 transition-colors " +
              (activeTab === "search" ? "text-gray-800 border-blue-500" : "text-gray-500 border-transparent hover:text-gray-700")}
          >
            搜索引擎
          </button>
          <button
            onClick={() => setActiveTab("routing")}
            className={"text-sm font-medium pb-1 border-b-2 transition-colors " +
              (activeTab === "routing" ? "text-gray-800 border-blue-500" : "text-gray-500 border-transparent hover:text-gray-700")}
          >
            自动路由
          </button>
          <button
            onClick={() => setActiveTab("feishu")}
            className={"text-sm font-medium pb-1 border-b-2 transition-colors " +
              (activeTab === "feishu" ? "text-gray-800 border-blue-500" : "text-gray-500 border-transparent hover:text-gray-700")}
          >
            飞书协作
          </button>
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
              onClick={() => {
                setEditingModelId(null);
                setShowApiKey(false);
                setForm({ name: "", api_format: "openai", base_url: "https://api.openai.com/v1", model_name: "gpt-4o-mini", api_key: "" });
                setError("");
                setTestResult(null);
              }}
              className="text-xs text-gray-400 hover:text-gray-600"
            >
              取消编辑
            </button>
          )}
        </div>
        <div>
          <label className={labelCls}>快速选择 Provider</label>
          <select
            className={inputCls}
            defaultValue=""
            onChange={(e) => applyPreset(e.target.value)}
          >
            {PROVIDER_PRESETS.map((p) => (
              <option key={p.value} value={p.value}>{p.label}</option>
            ))}
          </select>
        </div>
        <div>
          <label className={labelCls}>名称</label>
          <input className={inputCls} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
        </div>
        <div>
          <label className={labelCls}>API 格式</label>
          <select className={inputCls} value={form.api_format} onChange={(e) => setForm({ ...form, api_format: e.target.value })}>
            <option value="openai">OpenAI 兼容</option>
            <option value="anthropic">Anthropic (Claude)</option>
          </select>
        </div>
        <div>
          <label className={labelCls}>Base URL</label>
          <input className={inputCls} value={form.base_url} onChange={(e) => setForm({ ...form, base_url: e.target.value })} />
        </div>
        <div>
          <label className={labelCls}>模型名称</label>
          <input className={inputCls} list="model-suggestions" value={form.model_name} onChange={(e) => setForm({ ...form, model_name: e.target.value })} />
          {modelSuggestions.length > 0 && (
            <datalist id="model-suggestions">
              {modelSuggestions.map((m) => (
                <option key={m} value={m} />
              ))}
            </datalist>
          )}
        </div>
        <div>
          <label className={labelCls}>API Key</label>
          <div className="relative">
            <input
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
            onClick={handleSave}
            className="flex-1 bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {editingModelId ? "保存修改" : "保存"}
          </button>
        </div>
      </div>
      </>)}

      {activeTab === "providers" && (
        <>
          {providers.length > 0 && (
            <div className="mb-6 space-y-2">
              <div className="text-xs text-gray-500 mb-2">已配置 Providers</div>
              {providers.map((p) => (
                <div key={p.id} className="flex items-center justify-between bg-white rounded-lg px-4 py-2.5 text-sm border border-transparent hover:border-gray-200">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-gray-800">{p.display_name}</span>
                      <span className="text-[10px] bg-gray-100 text-gray-600 px-1.5 py-0.5 rounded">{p.provider_key}</span>
                      {!p.enabled && <span className="text-[10px] bg-red-100 text-red-600 px-1.5 py-0.5 rounded">禁用</span>}
                    </div>
                    <div className="text-xs text-gray-400 mt-0.5 truncate">{p.protocol_type} · {p.base_url}</div>
                  </div>
                  <div className="flex items-center gap-2 flex-shrink-0 ml-3">
                    <button onClick={() => handleEditProvider(p)} className="text-blue-500 hover:text-blue-600 text-xs">编辑</button>
                    <button onClick={() => handleDeleteProvider(p.id)} className="text-red-400 hover:text-red-500 text-xs">删除</button>
                  </div>
                </div>
              ))}
            </div>
          )}

          <div className="bg-white rounded-lg p-4 space-y-3">
            <div className="text-xs font-medium text-gray-500 mb-2">
              {editingProviderId ? "编辑 Provider" : "添加 Provider"}
            </div>
            <div>
              <label className={labelCls}>Provider Key</label>
              <select
                className={inputCls}
                value={providerForm.provider_key}
                onChange={(e) => {
                  const key = e.target.value;
                  const preset = providerPlugins.find((x) => x.key === key);
                  setProviderForm((s) => ({
                    ...s,
                    provider_key: key,
                    display_name: preset?.display_name || key,
                  }));
                }}
              >
                {providerPlugins.map((p) => (
                  <option key={p.key} value={p.key}>{p.display_name} ({p.key})</option>
                ))}
              </select>
            </div>
            <div>
              <label className={labelCls}>显示名称</label>
              <input className={inputCls} value={providerForm.display_name} onChange={(e) => setProviderForm({ ...providerForm, display_name: e.target.value })} />
            </div>
            <div>
              <label className={labelCls}>协议</label>
              <select className={inputCls} value={providerForm.protocol_type} onChange={(e) => setProviderForm({ ...providerForm, protocol_type: e.target.value })}>
                {PROVIDER_PROTOCOL_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>{o.label}</option>
                ))}
              </select>
            </div>
            <div>
              <label className={labelCls}>Base URL</label>
              <input className={inputCls} value={providerForm.base_url} onChange={(e) => setProviderForm({ ...providerForm, base_url: e.target.value })} />
            </div>
            <div>
              <label className={labelCls}>API Key</label>
              <input className={inputCls} type="password" value={providerForm.api_key_encrypted} onChange={(e) => setProviderForm({ ...providerForm, api_key_encrypted: e.target.value })} />
            </div>
            <label className="flex items-center gap-2 text-xs text-gray-600">
              <input type="checkbox" checked={providerForm.enabled} onChange={(e) => setProviderForm({ ...providerForm, enabled: e.target.checked })} />
              启用该 Provider
            </label>
            {providerError && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{providerError}</div>}
            <button
              onClick={handleSaveProvider}
              className="w-full bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
            >
              {editingProviderId ? "保存修改" : "保存 Provider"}
            </button>
          </div>
        </>
      )}

      {activeTab === "capabilities" && (
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
            <label className={labelCls}>主 Provider</label>
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
                <option key={p.id} value={p.id}>{p.display_name} ({p.provider_key})</option>
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
                    <option value="">选择 Provider</option>
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

      {activeTab === "health" && (
        <div className="bg-white rounded-lg p-4 space-y-3">
          <div className="text-xs font-medium text-gray-500 mb-2">Provider 健康检查</div>
          <div>
            <label className={labelCls}>选择 Provider</label>
            <select
              className={inputCls}
              value={healthProviderId}
              onChange={(e) => setHealthProviderId(e.target.value)}
            >
              <option value="">请选择</option>
              {providers.map((p) => (
                <option key={p.id} value={p.id}>{p.display_name} ({p.provider_key})</option>
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
            {healthLoading ? "检测中..." : "一键巡检全部 Provider"}
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
                  <div>ID: {r.provider_id || "-"}</div>
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

      {activeTab === "mcp" && (<>
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
        <div>
          <label className={labelCls}>环境变量（JSON 格式，可选）</label>
          <input className={inputCls} placeholder='例: {"API_KEY": "xxx"}' value={mcpForm.env} onChange={(e) => setMcpForm({ ...mcpForm, env: e.target.value })} />
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

      {activeTab === "routing" && (
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

      {activeTab === "feishu" && (
        <div className="space-y-4">
          <div className="bg-white rounded-lg p-4 space-y-3">
            <div className="text-xs font-medium text-gray-500">飞书网关配置</div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div>
                <label className={labelCls}>Ingress Token（可空，长连接可不填）</label>
                <input
                  className={inputCls}
                  value={feishuSettings.ingress_token}
                  onChange={(e) => setFeishuSettings((s) => ({ ...s, ingress_token: e.target.value }))}
                />
              </div>
              <div>
                <label className={labelCls}>Encrypt Key（可空，长连接可不填）</label>
                <input
                  className={inputCls}
                  value={feishuSettings.encrypt_key}
                  onChange={(e) => setFeishuSettings((s) => ({ ...s, encrypt_key: e.target.value }))}
                />
              </div>
            </div>
            <div className="text-xs text-gray-500">
              飞书 AppID/AppSecret 不再使用全局配置，请到“智能体员工”页面为具体员工绑定。
            </div>
            <div className="text-xs text-gray-400">
              普通用户无需配置 Sidecar 地址，系统默认 `http://localhost:8765` 自动管理。
            </div>
            <div className="flex gap-2">
              <button
                onClick={handleSaveFeishuSettings}
                className="bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 px-3 rounded-lg"
              >
                保存配置
              </button>
              <button
                onClick={handleVerifyFeishuConnection}
                className="bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 px-3 rounded-lg"
              >
                保存并校验连接
              </button>
              <button
                onClick={refreshFeishuConsole}
                className="bg-gray-100 hover:bg-gray-200 text-sm py-1.5 px-3 rounded-lg"
              >
                刷新状态
              </button>
            </div>
          </div>

          <div className="bg-white rounded-lg p-4 space-y-3">
            <div className="text-xs font-medium text-gray-500">连接状态</div>
            <div className="text-xs text-gray-600">
              WS: {feishuWsStatus?.running ? "running" : "stopped"} · queued={feishuWsStatus?.queued_events ?? 0}
            </div>
            <div className="text-xs text-gray-600">
              Relay: {feishuRelayStatus?.running ? "running" : "stopped"} · accepted_total={feishuRelayStatus?.total_accepted ?? 0}
              {feishuRelayStatus?.last_error ? ` · error=${feishuRelayStatus.last_error}` : ""}
            </div>
            <div className="text-xs text-gray-400">
              连接由系统自动维护，无需手动启动/停止长连接。
            </div>
            {feishuOpMessage && (
              <div className="text-xs bg-blue-50 text-blue-700 border border-blue-100 rounded px-2 py-1">{feishuOpMessage}</div>
            )}
          </div>

          <div className="bg-white rounded-lg p-4 space-y-3">
            <div className="text-xs font-medium text-gray-500">智能体员工</div>
            <div className="text-xs text-gray-400">
              每个员工可配置角色、技能集合、飞书标识与默认工作目录。技能只允许在桌面端由管理员维护。
            </div>
            <div className="space-y-2 border border-gray-100 rounded p-2">
              <div className="text-xs text-gray-500">全局默认工作目录（新建会话默认使用）</div>
              <input
                className={inputCls}
                placeholder="例如 D:\\workspace\\skillmint"
                value={globalDefaultWorkDir}
                onChange={(e) => setGlobalDefaultWorkDir(e.target.value)}
              />
              <div className="text-[11px] text-gray-400">
                默认：C:\Users\&lt;用户名&gt;\SkillMint\workspace。支持 C/D/E 盘，目录不存在自动创建。
              </div>
              <button
                onClick={handleSaveGlobalDefaultWorkDir}
                className="bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 px-3 rounded-lg"
              >
                保存默认目录
              </button>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div className="max-h-56 overflow-y-auto border border-gray-100 rounded">
                {employees.length === 0 ? (
                  <div className="text-xs text-gray-400 px-2 py-2">暂无员工</div>
                ) : (
                  employees.map((e) => (
                    <button
                      key={e.id}
                      onClick={() => handlePickEmployee(e.id)}
                      className={
                        "w-full text-left px-2 py-1.5 text-xs border-b border-gray-50 hover:bg-gray-50 " +
                        (selectedEmployeeId === e.id ? "bg-blue-50 text-blue-700" : "text-gray-700")
                      }
                    >
                      <div className="font-medium truncate">
                        {e.name} · {e.role_id}
                      </div>
                      <div className="text-[11px] text-gray-400 truncate">
                        skill={e.primary_skill_id || "通用助手（系统默认）"} · dir={e.default_work_dir || "(默认)"}
                      </div>
                    </button>
                  ))
                )}
              </div>
              <div className="space-y-2">
                <input
                  className={inputCls}
                  placeholder="员工名称"
                  value={employeeForm.name}
                  onChange={(e) => setEmployeeForm((s) => ({ ...s, name: e.target.value }))}
                />
                <input
                  className={inputCls}
                  placeholder="角色 ID（如 project_manager）"
                  value={employeeForm.role_id}
                  onChange={(e) => setEmployeeForm((s) => ({ ...s, role_id: e.target.value }))}
                />
                <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
                  {EMPLOYEE_ROLE_TEMPLATES.map((tpl) => (
                    <button
                      key={tpl.role_id}
                      type="button"
                      onClick={() => applyEmployeeRoleTemplate(tpl.role_id)}
                      className="h-8 rounded border border-gray-200 hover:border-blue-300 hover:bg-blue-50 text-xs text-gray-700"
                    >
                      填充{tpl.name}
                    </button>
                  ))}
                </div>
                <input
                  className={inputCls}
                  placeholder="飞书 open_id（可空，仅用于飞书@精准路由）"
                  value={employeeForm.feishu_open_id}
                  onChange={(e) => setEmployeeForm((s) => ({ ...s, feishu_open_id: e.target.value }))}
                />
                <select
                  className={inputCls + " bg-white"}
                  value={employeeForm.primary_skill_id}
                  onChange={(e) => setEmployeeForm((s) => ({ ...s, primary_skill_id: e.target.value }))}
                >
                  <option value="">通用助手（系统默认）</option>
                  {employeeSkillOptions.map((skill) => (
                    <option key={skill.id} value={skill.id}>
                      {skill.name}
                    </option>
                  ))}
                </select>
                <input
                  className={inputCls}
                  placeholder="默认工作目录（可空）"
                  value={employeeForm.default_work_dir}
                  onChange={(e) => setEmployeeForm((s) => ({ ...s, default_work_dir: e.target.value }))}
                />
                <div className="text-xs text-gray-500">技能集合（主员工可留空）</div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-2 max-h-36 overflow-y-auto border border-gray-100 rounded p-2">
                  {employeeSkillOptions.map((skill) => {
                    const checked = employeeForm.skill_ids.includes(skill.id);
                    return (
                      <label key={skill.id} className="inline-flex items-center gap-2 text-xs text-gray-700">
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={(e) => {
                            setEmployeeForm((s) => {
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
                <div className="flex items-center gap-4 text-xs text-gray-600">
                  <label className="inline-flex items-center gap-1">
                    <input
                      type="checkbox"
                      checked={employeeForm.enabled}
                      onChange={(e) => setEmployeeForm((s) => ({ ...s, enabled: e.target.checked }))}
                    />
                    启用
                  </label>
                  <label className="inline-flex items-center gap-1">
                    <input
                      type="checkbox"
                      checked={employeeForm.is_default}
                      onChange={(e) => setEmployeeForm((s) => ({ ...s, is_default: e.target.checked }))}
                    />
                    默认员工
                  </label>
                </div>
                <textarea
                  className={inputCls}
                  rows={2}
                  placeholder="员工人格/职责（可空）"
                  value={employeeForm.persona}
                  onChange={(e) => setEmployeeForm((s) => ({ ...s, persona: e.target.value }))}
                />
                <div className="flex gap-2">
                  <button
                    onClick={handleSaveEmployee}
                    className="bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 px-3 rounded-lg"
                  >
                    保存员工
                  </button>
                  <button
                    onClick={handleDeleteEmployee}
                    className="bg-red-50 hover:bg-red-100 text-red-600 text-sm py-1.5 px-3 rounded-lg"
                  >
                    删除员工
                  </button>
                </div>
              </div>
            </div>
          </div>

          <div className="bg-white rounded-lg p-4 space-y-3">
            <div className="text-xs font-medium text-gray-500">最近会话（用于绑定角色）</div>
            <div className="text-xs text-gray-400">
              这里展示机器人最近收到消息的会话。你只需要选中会话并配置角色，后续在飞书群 `@机器人` 即可自动协作。
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div>
                <div className="text-xs text-gray-500 mb-1">飞书群聊（API）</div>
                <div className="max-h-44 overflow-y-auto border border-gray-100 rounded">
                  {feishuChats.length === 0 ? (
                    <div className="text-xs text-gray-400 px-2 py-2">暂无</div>
                  ) : (
                    feishuChats.map((c) => (
                      <button
                        key={c.chat_id}
                        onClick={() => setSelectedThreadId(c.chat_id)}
                        className={
                          "w-full text-left px-2 py-1.5 text-xs border-b border-gray-50 hover:bg-gray-50 " +
                          (selectedThreadId === c.chat_id ? "bg-blue-50 text-blue-700" : "text-gray-700")
                        }
                      >
                        <div className="font-medium truncate">{c.name || c.chat_id}</div>
                        <div className="text-[11px] text-gray-400 truncate">{c.chat_id}</div>
                      </button>
                    ))
                  )}
                </div>
              </div>
              <div>
                <div className="text-xs text-gray-500 mb-1">最近线程（收件箱）</div>
                <div className="max-h-44 overflow-y-auto border border-gray-100 rounded">
                  {recentThreads.length === 0 ? (
                    <div className="text-xs text-gray-400 px-2 py-2">暂无</div>
                  ) : (
                    recentThreads.map((t) => (
                      <button
                        key={t.thread_id + t.last_seen_at}
                        onClick={() => setSelectedThreadId(t.thread_id)}
                        className={
                          "w-full text-left px-2 py-1.5 text-xs border-b border-gray-50 hover:bg-gray-50 " +
                          (selectedThreadId === t.thread_id ? "bg-blue-50 text-blue-700" : "text-gray-700")
                        }
                      >
                        <div className="font-medium truncate">{t.thread_id}</div>
                        <div className="text-[11px] text-gray-400 truncate">{t.last_text_preview}</div>
                      </button>
                    ))
                  )}
                </div>
              </div>
            </div>

            <div className="pt-2 border-t border-gray-100 space-y-2">
              <div className="text-xs text-gray-600">当前线程：{selectedThreadId || "(未选择)"}</div>
              <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
                <input
                  className={inputCls}
                  placeholder="tenant_id"
                  value={roleTenantId}
                  onChange={(e) => setRoleTenantId(e.target.value)}
                />
                <input
                  className={inputCls}
                  placeholder="scenario_template"
                  value={roleScenarioTemplate}
                  onChange={(e) => setRoleScenarioTemplate(e.target.value)}
                />
                <button
                  className="bg-gray-100 hover:bg-gray-200 text-sm rounded-lg"
                  onClick={() => loadThreadRoleConfig(selectedThreadId)}
                >
                  读取线程角色配置
                </button>
              </div>
              <textarea
                className={inputCls}
                rows={3}
                placeholder="角色 ID，逗号分隔"
                value={roleIdsInput}
                onChange={(e) => setRoleIdsInput(e.target.value)}
              />
              <button
                onClick={handleBindThreadRoles}
                className="bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 px-3 rounded-lg"
              >
                保存线程角色绑定
              </button>
              <textarea
                className={inputCls}
                rows={2}
                placeholder="线程员工ID（逗号分隔）"
                value={threadEmployeeIdsInput}
                onChange={(e) => setThreadEmployeeIdsInput(e.target.value)}
              />
              <button
                onClick={handleBindThreadEmployees}
                className="bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 px-3 rounded-lg"
              >
                保存线程员工绑定
              </button>
              {threadRoleConfig && (
                <div className="text-xs text-gray-600 bg-gray-50 border border-gray-100 rounded px-2 py-1">
                  已绑定：{threadRoleConfig.roles.join(", ")} · 模板：{threadRoleConfig.scenario_template}
                </div>
              )}
              {threadEmployeeBinding && (
                <div className="text-xs text-gray-600 bg-gray-50 border border-gray-100 rounded px-2 py-1">
                  员工绑定：{threadEmployeeBinding.employee_ids.join(", ") || "(空)"}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
