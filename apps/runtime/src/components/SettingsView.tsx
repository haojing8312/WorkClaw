import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openExternalUrl } from "../utils/openExternalUrl";
import { SearchConfigForm } from "./SearchConfigForm";
import { SettingsShell } from "./settings/SettingsShell";
import { ModelsSettingsSection } from "./settings/models/ModelsSettingsSection";
import { DesktopSettingsSection } from "./settings/desktop/DesktopSettingsSection";
import { SettingsTabNav, type SettingsTabName } from "./settings/SettingsTabNav";
import {
  applySearchPresetToForm,
  EMPTY_SEARCH_CONFIG_FORM,
  validateSearchConfigForm,
} from "../lib/search-config";
import {
  listModelConfigs,
  listProviderConfigs,
  syncModelConnections,
} from "./settings/models/modelSettingsService";
import {
  CapabilityRouteTemplateInfo,
  CapabilityRoutingPolicy,
  FeishuPairingRequestRecord,
  FeishuPluginEnvironmentStatus,
  FeishuSetupProgress,
  FeishuGatewaySettings,
  OpenClawPluginFeishuAdvancedSettings,
  OpenClawPluginChannelHost,
  OpenClawPluginInstallRecord,
  OpenClawPluginChannelSnapshotResult,
  OpenClawPluginFeishuCredentialProbeResult,
  OpenClawPluginFeishuRuntimeStatus,
  OpenClawLarkInstallerMode,
  OpenClawLarkInstallerSessionStatus,
  ModelConfig,
  ProviderConfig,
  ProviderHealthInfo,
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

function getErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string") {
    return error || fallback;
  }
  if (error instanceof Error) {
    return error.message || fallback;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message || fallback;
  }
  return fallback;
}

interface Props {
  onClose: () => void;
  onOpenEmployees?: () => void;
  initialTab?: SettingsTabName;
  showDevModelSetupTools?: boolean;
  onDevResetFirstUseOnboarding?: () => void;
  onDevOpenQuickModelSetup?: () => void;
}

interface RoutingSettings {
  max_call_depth: number;
  node_timeout_seconds: number;
  retry_count: number;
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

const FEISHU_OFFICIAL_PLUGIN_DOC_URL =
  "https://bytedance.larkoffice.com/docx/MFK7dDFLFoVlOGxWCv5cTXKmnMh#M0usd9GLwoiBxtx1UyjcpeMhnRe";

type FeishuAdvancedFieldConfig = {
  key: keyof OpenClawPluginFeishuAdvancedSettings;
  label: string;
  description: string;
  kind: "input" | "textarea";
  rows?: number;
};

const FEISHU_ADVANCED_MESSAGE_FIELDS: FeishuAdvancedFieldConfig[] = [
  { key: "footer_json", label: "回复页脚 JSON", description: "定义回复尾部展示的状态、耗时等附加信息。", kind: "textarea", rows: 5 },
  { key: "account_overrides_json", label: "账号覆盖 JSON", description: "按账号覆盖消息展示行为，适合多账号接入时做细分调整。", kind: "textarea", rows: 5 },
  { key: "render_mode", label: "渲染模式", description: "控制回复内容的主要渲染方式。", kind: "input" },
  { key: "streaming", label: "流式输出", description: "决定回复是否边生成边发送。", kind: "input" },
  { key: "text_chunk_limit", label: "文本分块上限", description: "单次消息的最大文本块长度。", kind: "input" },
  { key: "chunk_mode", label: "分块模式", description: "控制长消息按什么策略拆分。", kind: "input" },
  { key: "markdown_mode", label: "Markdown 模式", description: "控制 Markdown 内容如何转换给飞书。", kind: "input" },
  { key: "markdown_table_mode", label: "Markdown 表格模式", description: "控制表格内容的展示方式。", kind: "input" },
];

const FEISHU_ADVANCED_ROUTING_FIELDS: FeishuAdvancedFieldConfig[] = [
  { key: "groups_json", label: "群聊高级规则 JSON", description: "按群聊配置启用、提及规则等进阶行为。", kind: "textarea", rows: 8 },
  { key: "dms_json", label: "私聊高级规则 JSON", description: "按私聊对象配置启用状态和系统提示。", kind: "textarea", rows: 8 },
  { key: "reply_in_thread", label: "线程内回复", description: "控制消息是否优先在线程中回复。", kind: "input" },
  { key: "group_session_scope", label: "群聊会话范围", description: "决定群聊里如何划分会话上下文。", kind: "input" },
  { key: "topic_session_mode", label: "话题会话模式", description: "决定是否把话题回复视为独立会话。", kind: "input" },
];

const FEISHU_ADVANCED_RUNTIME_FIELDS: FeishuAdvancedFieldConfig[] = [
  { key: "heartbeat_visibility", label: "心跳可见性", description: "控制连接保活提示是否对外可见。", kind: "input" },
  { key: "heartbeat_interval_ms", label: "心跳间隔毫秒", description: "设置连接保活检测频率。", kind: "input" },
  { key: "media_max_mb", label: "媒体大小上限 MB", description: "限制可处理媒体消息的大小。", kind: "input" },
  { key: "http_timeout_ms", label: "HTTP 超时毫秒", description: "设置外部请求的最大等待时间。", kind: "input" },
  { key: "config_writes", label: "允许插件写回配置", description: "决定插件运行时是否允许自动写回部分配置。", kind: "input" },
];

const FEISHU_ADVANCED_DYNAMIC_AGENT_FIELDS: FeishuAdvancedFieldConfig[] = [
  { key: "dynamic_agent_creation_enabled", label: "动态 Agent 创建", description: "决定是否允许根据飞书会话动态创建 Agent。", kind: "input" },
  { key: "dynamic_agent_creation_workspace_template", label: "动态工作区模板", description: "定义动态创建工作区时使用的路径模板。", kind: "input" },
  { key: "dynamic_agent_creation_agent_dir_template", label: "动态 Agent 目录模板", description: "定义动态 Agent 目录的生成规则。", kind: "input" },
  { key: "dynamic_agent_creation_max_agents", label: "动态 Agent 数量上限", description: "限制动态创建 Agent 的最大数量。", kind: "input" },
];

export type FeishuOnboardingStep =
  | "environment"
  | "plugin"
  | "existing_robot"
  | "create_robot"
  | "authorize"
  | "approve_pairing"
  | "routing"
  | "skipped";

export interface FeishuOnboardingInput {
  summaryState?: string | null;
  setupProgress?: Partial<FeishuSetupProgress> | null;
  installerMode?: OpenClawLarkInstallerMode | null;
  skipped?: boolean;
}

export interface FeishuOnboardingState {
  currentStep: FeishuOnboardingStep;
  stepOrder: FeishuOnboardingStep[];
  canContinue: boolean;
  skipped: boolean;
  mode: "existing_robot" | "create_robot";
}

interface NormalizedFeishuOnboardingInput {
  summaryState: string | null;
  installerMode: OpenClawLarkInstallerMode | null;
  skipped: boolean;
  runtimeRunning: boolean;
  authApproved: boolean;
  pendingPairings: number;
}

function normalizeFeishuOnboardingInput(input: FeishuOnboardingInput): NormalizedFeishuOnboardingInput {
  const summaryState = input.summaryState ?? input.setupProgress?.summary_state ?? null;
  return {
    summaryState,
    installerMode: input.installerMode ?? null,
    skipped: input.skipped === true || summaryState === "skipped",
    runtimeRunning: input.setupProgress?.runtime_running === true,
    authApproved: input.setupProgress?.auth_status === "approved",
    pendingPairings: input.setupProgress?.pending_pairings ?? 0,
  };
}

function resolveFeishuOnboardingMode(installerMode: OpenClawLarkInstallerMode | null) {
  return installerMode === "create" ? "create_robot" : "existing_robot";
}

function resolveFeishuOnboardingCurrentStep(input: NormalizedFeishuOnboardingInput): FeishuOnboardingStep {
  if (input.skipped) {
    return "skipped";
  }

  const mode = resolveFeishuOnboardingMode(input.installerMode);

  if (input.summaryState === "env_missing") {
    return "environment";
  }
  if (input.summaryState === "ready_to_bind") {
    return "create_robot";
  }
  if (input.summaryState === "plugin_not_installed") {
    return "plugin";
  }
  if (input.summaryState === "plugin_starting") {
    return "authorize";
  }
  if (input.summaryState === "awaiting_auth") {
    return "authorize";
  }
  if (input.summaryState === "awaiting_pairing_approval") {
    return "approve_pairing";
  }
  if (input.summaryState === "ready_for_routing") {
    return "routing";
  }
  if (input.summaryState === "runtime_error") {
    return "environment";
  }

  if (input.runtimeRunning) {
    if (input.pendingPairings > 0) {
      return "approve_pairing";
    }
    return input.authApproved ? "routing" : "authorize";
  }

  return "environment";
}

function canContinueFeishuOnboarding(input: NormalizedFeishuOnboardingInput, currentStep: FeishuOnboardingStep) {
  return input.skipped || currentStep === "routing";
}

function resolveFeishuOnboardingStepOrder(input: NormalizedFeishuOnboardingInput, mode: "existing_robot" | "create_robot") {
  if (input.skipped) {
    return ["skipped"] as FeishuOnboardingStep[];
  }

  return ["environment", "plugin", mode, "authorize", "approve_pairing", "routing"] as FeishuOnboardingStep[];
}

export function buildFeishuOnboardingState(input: FeishuOnboardingInput): FeishuOnboardingState {
  const normalized = normalizeFeishuOnboardingInput(input);
  const mode =
    normalized.summaryState === "ready_to_bind"
      ? "create_robot"
      : resolveFeishuOnboardingMode(normalized.installerMode);
  const currentStep = resolveFeishuOnboardingCurrentStep(normalized);
  return {
    currentStep,
    stepOrder: resolveFeishuOnboardingStepOrder(normalized, mode),
    canContinue: canContinueFeishuOnboarding(normalized, currentStep),
    skipped: normalized.skipped,
    mode,
  };
}

function formatFeishuOnboardingStepLabel(step: FeishuOnboardingStep) {
  switch (step) {
    case "environment":
      return "检查运行环境";
    case "existing_robot":
      return "绑定已有机器人";
    case "plugin":
      return "安装官方插件";
    case "create_robot":
      return "新建机器人";
    case "authorize":
      return "完成授权";
    case "approve_pairing":
      return "批准接入";
    case "routing":
      return "设置接待";
    case "skipped":
      return "已跳过引导";
    default:
      return step;
  }
}

interface FeishuOnboardingStepDisplay {
  title: string;
  body: string;
}

interface FeishuOnboardingPanelDisplay extends FeishuOnboardingStepDisplay {
  badgeLabel: string;
  badgeClassName: string;
  primaryActionLabel: string;
}

function resolveFeishuOnboardingStepDisplay(step: FeishuOnboardingStep): FeishuOnboardingStepDisplay {
  switch (step) {
    case "environment":
      return {
        title: "检查运行环境",
        body: "先确认 Node.js 和 npm 可用，再继续安装和授权。",
      };
    case "existing_robot":
      return {
        title: "绑定已有机器人",
        body: "先在这里确认你已有机器人的信息，再到高级控制台保存完整配置。",
      };
    case "plugin":
      return {
        title: "安装官方插件",
        body: "先安装飞书官方插件。安装完成后，再继续新建机器人或绑定已有机器人。",
      };
    case "create_robot":
      return {
        title: "新建机器人",
        body: "点击“新建机器人向导”后会打开飞书官方安装向导。完成创建后，WorkClaw 会自动回填机器人信息，再继续安装与授权。",
      };
    case "authorize":
      return {
        title: "完成授权",
        body: "完成官方插件启动后，回到飞书会话里走完授权；如果机器人提示 access not configured，下一步还需要批准这次接入。",
      };
    case "approve_pairing":
      return {
        title: "批准接入请求",
        body: "飞书里的会话已经发起接入请求。请在这里批准这次接入，机器人才能真正开始收发消息。",
      };
    case "routing":
      return {
        title: "设置接待",
        body: "授权完成后，把默认接待员工和群聊范围补齐，消息才会稳定落到正确员工。",
      };
    case "skipped":
      return {
        title: "已跳过引导",
        body: "你已经暂时跳过引导，随时可以重新打开。",
      };
    default:
      return {
        title: step,
        body: step,
      };
  }
}

function resolveFeishuOnboardingPanelDisplay(
  state: FeishuOnboardingState,
  isSkipped: boolean,
  selectedPath: "existing_robot" | "create_robot" | null,
): FeishuOnboardingPanelDisplay {
  if (isSkipped) {
    return {
      title: "已跳过引导",
      body: "已跳过引导。需要时随时点击“重新打开引导”。",
      badgeLabel: "已跳过引导",
      badgeClassName: "border-gray-200 bg-gray-100 text-gray-700",
      primaryActionLabel: "重新打开引导",
    };
  }

  const branchStep =
    state.currentStep === "existing_robot" || state.currentStep === "create_robot"
      ? selectedPath ?? state.currentStep
      : state.currentStep;
  const stepDisplay = resolveFeishuOnboardingStepDisplay(branchStep);
  if (branchStep === "create_robot") {
    return {
      ...stepDisplay,
      badgeLabel: state.canContinue ? "可继续使用" : "仍需完成当前步骤",
      badgeClassName: "border-blue-200 bg-blue-50 text-blue-700",
      primaryActionLabel: "新建机器人向导（高级）",
    };
  }
  if (branchStep === "existing_robot") {
    return {
      ...stepDisplay,
      badgeLabel: state.canContinue ? "可继续使用" : "仍需完成当前步骤",
      badgeClassName: "border-blue-200 bg-blue-50 text-blue-700",
      primaryActionLabel: "验证机器人信息",
    };
  }
  if (branchStep === "plugin") {
    return {
      ...stepDisplay,
      badgeLabel: state.canContinue ? "可继续使用" : "仍需完成当前步骤",
      badgeClassName: "border-blue-200 bg-blue-50 text-blue-700",
      primaryActionLabel: "安装官方插件",
    };
  }
  if (branchStep === "approve_pairing") {
    return {
      ...stepDisplay,
      badgeLabel: state.canContinue ? "可继续使用" : "等待批准接入",
      badgeClassName: "border-amber-200 bg-amber-50 text-amber-700",
      primaryActionLabel: "批准这次接入",
    };
  }
  return {
    ...stepDisplay,
    badgeLabel: state.canContinue ? "可继续使用" : "仍需完成当前步骤",
    badgeClassName: "border-blue-200 bg-blue-50 text-blue-700",
    primaryActionLabel: "继续引导设置",
  };
}

function resolveFeishuGuidedInlineError(
  errorMessage: string,
  step: FeishuOnboardingStep,
  branch: "existing_robot" | "create_robot" | null,
): string | null {
  if (!errorMessage.trim()) {
    return null;
  }
  if (
    errorMessage.startsWith("请先填写已有机器人的 App ID 和 App Secret") ||
    errorMessage.startsWith("已有机器人校验失败:") ||
    errorMessage.startsWith("验证机器人信息失败:") ||
    errorMessage.startsWith("启动飞书官方创建机器人向导失败:") ||
    errorMessage.startsWith("启动飞书官方绑定机器人向导失败:")
  ) {
    return errorMessage;
  }
  if (branch === "existing_robot" && errorMessage.startsWith("请先填写并保存已有机器人的 App ID 和 App Secret")) {
    return errorMessage;
  }
  if (
    step === "plugin" &&
    errorMessage.startsWith("安装飞书官方插件失败:")
  ) {
    return errorMessage;
  }
  if (
    step === "authorize" &&
    (errorMessage.startsWith("安装并启动飞书连接失败:") ||
      errorMessage.startsWith("刷新飞书官方插件状态失败:") ||
      errorMessage.startsWith("官方插件启动失败:") ||
      errorMessage.startsWith("启动飞书官方插件失败:"))
  ) {
    return errorMessage;
  }
  if (
    step === "approve_pairing" &&
    (errorMessage.startsWith("批准飞书接入请求失败:") ||
      errorMessage.startsWith("拒绝飞书接入请求失败:"))
  ) {
    return errorMessage;
  }
  if (step === "environment" && errorMessage.startsWith("刷新飞书接入状态失败:")) {
    return errorMessage;
  }
  return null;
}

function resolveFeishuAuthorizationInlineError(errorMessage: string): string | null {
  if (!errorMessage.trim()) {
    return null;
  }
  if (
    errorMessage.startsWith("安装并启动飞书连接失败:") ||
    errorMessage.startsWith("刷新飞书官方插件状态失败:") ||
    errorMessage.startsWith("官方插件启动失败:") ||
    errorMessage.startsWith("启动飞书官方插件失败:")
  ) {
    return errorMessage;
  }
  return null;
}

function resolveFeishuGuidedInlineNotice(
  noticeMessage: string,
  step: FeishuOnboardingStep,
  branch: "existing_robot" | "create_robot" | null,
): string | null {
  if (!noticeMessage.trim()) {
    return null;
  }
  if (
    (branch === "existing_robot" && noticeMessage.startsWith("机器人信息验证成功")) ||
    (branch === "create_robot" && noticeMessage.startsWith("已启动飞书官方创建机器人向导")) ||
    (branch === "existing_robot" && noticeMessage.startsWith("已启动飞书官方绑定机器人向导"))
  ) {
    return noticeMessage;
  }
  if (
    step === "plugin" &&
    noticeMessage.startsWith("飞书官方插件已安装")
  ) {
    return noticeMessage;
  }
  if (
    step === "authorize" &&
    (noticeMessage.startsWith("飞书连接组件已启动") || noticeMessage.startsWith("已尝试启动飞书连接组件"))
  ) {
    return noticeMessage;
  }
  if (
    step === "approve_pairing" &&
    (noticeMessage.startsWith("已批准飞书接入请求") || noticeMessage.startsWith("已拒绝飞书接入请求"))
  ) {
    return noticeMessage;
  }
  return null;
}

function getLatestInstallerOutputLine(session: OpenClawLarkInstallerSessionStatus) {
  return session.recent_output.length > 0
    ? session.recent_output[session.recent_output.length - 1] ?? ""
    : "";
}

function isFeishuInstallerFinished(session: OpenClawLarkInstallerSessionStatus) {
  return (
    !session.running &&
    !!session.mode &&
    session.recent_output.some((line) => line.includes("[system] official installer finished"))
  );
}

function resolveFeishuInstallerCompletionNotice(session: OpenClawLarkInstallerSessionStatus) {
  if (!isFeishuInstallerFinished(session)) {
    return "";
  }

  const output = session.recent_output.join("\n");
  if (
    session.mode === "create" &&
    (output.includes("Success! Bot configured.") || output.includes("机器人配置成功"))
  ) {
    return "机器人创建已完成，请点击“启动连接”继续完成授权。";
  }

  if (session.mode === "link") {
    return "机器人关联已完成，请点击“启动连接”继续完成授权。";
  }

  return "安装向导已完成，请继续启动连接并完成授权。";
}

function shouldShowFeishuInstallerGuidedPanel(
  branch: "existing_robot" | "create_robot" | null,
  session: OpenClawLarkInstallerSessionStatus,
) {
  return branch === "create_robot" && (session.running || session.recent_output.length > 0);
}

function resolveFeishuInstallerFlowLabel(mode: OpenClawLarkInstallerMode | null) {
  if (mode === "create") {
    return "飞书官方创建机器人向导";
  }
  if (mode === "link") {
    return "飞书官方绑定机器人向导";
  }
  return "飞书官方向导";
}

function looksLikeInstallerQrLine(line: string) {
  return /[█▀▄▌▐]/.test(line);
}

function extractFeishuInstallerQrBlock(lines: string[]) {
  let bestStart = -1;
  let bestLength = 0;
  let currentStart = -1;
  let currentLength = 0;

  for (let index = 0; index < lines.length; index += 1) {
    if (looksLikeInstallerQrLine(lines[index] || "")) {
      if (currentStart === -1) {
        currentStart = index;
        currentLength = 0;
      }
      currentLength += 1;
      if (currentLength > bestLength) {
        bestStart = currentStart;
        bestLength = currentLength;
      }
    } else {
      currentStart = -1;
      currentLength = 0;
    }
  }

  if (bestStart === -1 || bestLength < 3) {
    return [];
  }
  return lines.slice(bestStart, bestStart + bestLength);
}

function sanitizeFeishuInstallerDisplayLines(lines: string[]) {
  const qrBlock = extractFeishuInstallerQrBlock(lines);
  const qrSet = new Set(qrBlock);
  const filtered: string[] = [];
  let skipDebugObject = false;

  for (const rawLine of lines) {
    const line = rawLine ?? "";
    if (qrSet.has(line)) {
      continue;
    }
    if (line.startsWith("[DEBUG]") && line.includes("{")) {
      skipDebugObject = true;
      continue;
    }
    if (skipDebugObject) {
      if (line.trim() === "}") {
        skipDebugObject = false;
      }
      continue;
    }
    if (line.startsWith("[DEBUG]")) {
      continue;
    }
    filtered.push(line);
  }

  return filtered;
}

export function SettingsView({
  onClose,
  onOpenEmployees,
  initialTab = "models",
  showDevModelSetupTools = false,
  onDevResetFirstUseOnboarding,
  onDevOpenQuickModelSetup,
}: Props) {
  const [models, setModels] = useState<ModelConfig[]>([]);

  // MCP 服务器管理
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [mcpServers, setMcpServers] = useState<any[]>([]);
  const [mcpForm, setMcpForm] = useState({ name: "", command: "", args: "", env: "" });
  const [mcpError, setMcpError] = useState("");
  const [showMcpEnvJson, setShowMcpEnvJson] = useState(false);
  const [activeTab, setActiveTab] = useState<SettingsTabName>(initialTab);

  // 搜索引擎配置
  const [searchConfigs, setSearchConfigs] = useState<ModelConfig[]>([]);
  const [searchForm, setSearchForm] = useState(EMPTY_SEARCH_CONFIG_FORM);
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
  const [feishuConnectorSettings, setFeishuConnectorSettings] = useState<FeishuGatewaySettings>({
    app_id: "",
    app_secret: "",
    ingress_token: "",
    encrypt_key: "",
    sidecar_base_url: "",
  });
  const [feishuAdvancedSettings, setFeishuAdvancedSettings] = useState<OpenClawPluginFeishuAdvancedSettings>({
    groups_json: "",
    dms_json: "",
    footer_json: "",
    account_overrides_json: "",
    render_mode: "",
    streaming: "",
    text_chunk_limit: "",
    chunk_mode: "",
    reply_in_thread: "",
    group_session_scope: "",
    topic_session_mode: "",
    markdown_mode: "",
    markdown_table_mode: "",
    heartbeat_visibility: "",
    heartbeat_interval_ms: "",
    media_max_mb: "",
    http_timeout_ms: "",
    config_writes: "",
    webhook_host: "",
    webhook_port: "",
    dynamic_agent_creation_enabled: "",
    dynamic_agent_creation_workspace_template: "",
    dynamic_agent_creation_agent_dir_template: "",
    dynamic_agent_creation_max_agents: "",
  });
  const [officialFeishuRuntimeStatus, setOfficialFeishuRuntimeStatus] =
    useState<OpenClawPluginFeishuRuntimeStatus | null>(null);
  const [pluginChannelHosts, setPluginChannelHosts] = useState<OpenClawPluginChannelHost[]>([]);
  const [pluginChannelSnapshots, setPluginChannelSnapshots] = useState<Record<string, OpenClawPluginChannelSnapshotResult>>({});
  const [pluginChannelHostsError, setPluginChannelHostsError] = useState("");
  const [pluginChannelSnapshotsError, setPluginChannelSnapshotsError] = useState("");
  const [feishuEnvironmentStatus, setFeishuEnvironmentStatus] = useState<FeishuPluginEnvironmentStatus | null>(null);
  const [feishuSetupProgress, setFeishuSetupProgress] = useState<FeishuSetupProgress | null>(null);
  const [validatingFeishuCredentials, setValidatingFeishuCredentials] = useState(false);
  const [feishuCredentialProbe, setFeishuCredentialProbe] =
    useState<OpenClawPluginFeishuCredentialProbeResult | null>(null);
  const [feishuInstallerSession, setFeishuInstallerSession] = useState<OpenClawLarkInstallerSessionStatus>({
    running: false,
    mode: null,
    started_at: null,
    last_output_at: null,
    last_error: null,
    prompt_hint: null,
    recent_output: [],
  });
  const [feishuInstallerInput, setFeishuInstallerInput] = useState("");
  const [feishuInstallerBusy, setFeishuInstallerBusy] = useState(false);
  const [feishuInstallerStartingMode, setFeishuInstallerStartingMode] = useState<OpenClawLarkInstallerMode | null>(null);
  const handledFeishuInstallerCompletionRef = useRef("");
  const [feishuPairingRequests, setFeishuPairingRequests] = useState<FeishuPairingRequestRecord[]>([]);
  const [feishuPairingRequestsError, setFeishuPairingRequestsError] = useState("");
  const [feishuPairingActionLoading, setFeishuPairingActionLoading] = useState<"approve" | "deny" | null>(null);
  const [savingFeishuConnector, setSavingFeishuConnector] = useState(false);
  const [savingFeishuAdvancedSettings, setSavingFeishuAdvancedSettings] = useState(false);
  const [retryingFeishuConnector, setRetryingFeishuConnector] = useState(false);
  const [installingOfficialFeishuPlugin, setInstallingOfficialFeishuPlugin] = useState(false);
  const [feishuConnectorNotice, setFeishuConnectorNotice] = useState("");
  const [feishuConnectorError, setFeishuConnectorError] = useState("");

  useEffect(() => {
    setActiveTab(initialTab);
  }, [initialTab]);

  useEffect(() => {
    void loadSharedModelData();
    loadSearchConfigs();
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

  async function loadSharedModelData() {
    try {
      const list = await listModelConfigs();
      setModels(list);
      await syncModelConnections(list);
      const providerList = await listProviderConfigs();
      const ids = new Set(list.map((model) => model.id));
      const aligned = providerList.filter((provider) => ids.has(provider.id));
      setProviders(aligned);
      if (aligned.length === 0) {
        setHealthProviderId("");
      } else if (!healthProviderId || !aligned.some((provider) => provider.id === healthProviderId)) {
        setHealthProviderId(aligned[0].id);
      }
    } catch (error) {
      console.warn("加载模型共享数据失败:", error);
    }
  }

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    void Promise.all([
      loadConnectorSettings(),
      loadConnectorStatuses(),
      loadConnectorPlatformData(),
      loadFeishuSetupProgress(),
      loadFeishuInstallerSessionStatus(),
    ]);
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    const timer = window.setInterval(() => {
      void Promise.all([
        loadConnectorStatuses(),
        loadConnectorPlatformData(),
        loadFeishuSetupProgress(),
      ]);
    }, 5000);

    return () => window.clearInterval(timer);
  }, [activeTab]);

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

  async function loadConnectorSettings() {
    try {
      const [feishuSettings, feishuAdvanced] = await Promise.all([
        invoke<FeishuGatewaySettings>("get_feishu_gateway_settings"),
        invoke<OpenClawPluginFeishuAdvancedSettings>("get_openclaw_plugin_feishu_advanced_settings"),
      ]);
      setFeishuConnectorSettings({
        app_id: feishuSettings?.app_id || "",
        app_secret: feishuSettings?.app_secret || "",
        ingress_token: feishuSettings?.ingress_token || "",
        encrypt_key: feishuSettings?.encrypt_key || "",
        sidecar_base_url: feishuSettings?.sidecar_base_url || "",
      });
      setFeishuAdvancedSettings({
        groups_json: feishuAdvanced?.groups_json || "",
        dms_json: feishuAdvanced?.dms_json || "",
        footer_json: feishuAdvanced?.footer_json || "",
        account_overrides_json: feishuAdvanced?.account_overrides_json || "",
        render_mode: feishuAdvanced?.render_mode || "",
        streaming: feishuAdvanced?.streaming || "",
        text_chunk_limit: feishuAdvanced?.text_chunk_limit || "",
        chunk_mode: feishuAdvanced?.chunk_mode || "",
        reply_in_thread: feishuAdvanced?.reply_in_thread || "",
        group_session_scope: feishuAdvanced?.group_session_scope || "",
        topic_session_mode: feishuAdvanced?.topic_session_mode || "",
        markdown_mode: feishuAdvanced?.markdown_mode || "",
        markdown_table_mode: feishuAdvanced?.markdown_table_mode || "",
        heartbeat_visibility: feishuAdvanced?.heartbeat_visibility || "",
        heartbeat_interval_ms: feishuAdvanced?.heartbeat_interval_ms || "",
        media_max_mb: feishuAdvanced?.media_max_mb || "",
        http_timeout_ms: feishuAdvanced?.http_timeout_ms || "",
        config_writes: feishuAdvanced?.config_writes || "",
        webhook_host: feishuAdvanced?.webhook_host || "",
        webhook_port: feishuAdvanced?.webhook_port || "",
        dynamic_agent_creation_enabled: feishuAdvanced?.dynamic_agent_creation_enabled || "",
        dynamic_agent_creation_workspace_template:
          feishuAdvanced?.dynamic_agent_creation_workspace_template || "",
        dynamic_agent_creation_agent_dir_template:
          feishuAdvanced?.dynamic_agent_creation_agent_dir_template || "",
        dynamic_agent_creation_max_agents:
          feishuAdvanced?.dynamic_agent_creation_max_agents || "",
      });
    } catch (e) {
      console.warn("加载渠道连接器配置失败:", e);
    }
  }

  async function loadFeishuSetupProgress() {
    try {
      const progress = await invoke<FeishuSetupProgress>("get_feishu_setup_progress");
      setFeishuEnvironmentStatus(progress.environment ?? null);
      setFeishuSetupProgress(progress);
    } catch (e) {
      console.warn("加载飞书接入进度失败:", e);
      setFeishuEnvironmentStatus(null);
      setFeishuSetupProgress(null);
    }
  }

  async function loadConnectorStatuses() {
    try {
      const runtimeStatus = await invoke<OpenClawPluginFeishuRuntimeStatus>(
        "get_openclaw_plugin_feishu_runtime_status",
      );
      setOfficialFeishuRuntimeStatus(runtimeStatus);
    } catch (e) {
      console.warn("加载渠道连接器状态失败:", e);
      setOfficialFeishuRuntimeStatus(null);
    }
  }

  async function loadFeishuInstallerSessionStatus() {
    try {
      const status = await invoke<OpenClawLarkInstallerSessionStatus>(
        "get_openclaw_lark_installer_session_status",
      );
      setFeishuInstallerSession(status);
    } catch (e) {
      console.warn("加载飞书官方安装向导状态失败:", e);
    }
  }

  async function loadConnectorPlatformData() {
    const [hostsResult, pairingResult] = await Promise.allSettled([
      invoke<OpenClawPluginChannelHost[]>("list_openclaw_plugin_channel_hosts"),
      invoke<FeishuPairingRequestRecord[]>("list_feishu_pairing_requests", {
        status: null,
      }),
    ]);

    const normalizedHosts =
      hostsResult.status === "fulfilled"
        ? (Array.isArray(hostsResult.value) ? hostsResult.value : []).filter(
            (host) =>
              host.channel === "feishu" ||
              host.plugin_id === "openclaw-lark" ||
              host.npm_spec === "@larksuite/openclaw-lark" ||
              host.display_name.toLowerCase().includes("feishu") ||
              host.display_name.toLowerCase().includes("lark"),
          )
        : [];
    if (hostsResult.status !== "fulfilled") {
      console.warn("加载官方插件宿主失败:", hostsResult.reason);
    }
    setPluginChannelHosts(normalizedHosts);
    setPluginChannelHostsError(hostsResult.status === "fulfilled" ? "" : "官方插件状态暂时不可用");

    if (pairingResult.status !== "fulfilled") {
      console.warn("加载飞书配对请求失败:", pairingResult.reason);
    }
    setFeishuPairingRequests(pairingResult.status === "fulfilled" && Array.isArray(pairingResult.value) ? pairingResult.value : []);
    setFeishuPairingRequestsError(pairingResult.status === "fulfilled" ? "" : "配对记录加载失败");

    if (normalizedHosts.length === 0) {
      setPluginChannelSnapshots({});
      setPluginChannelSnapshotsError("");
      return;
    }

    const snapshotResults = await Promise.allSettled(
      normalizedHosts.map((host) =>
        invoke<OpenClawPluginChannelSnapshotResult>("get_openclaw_plugin_feishu_channel_snapshot", {
          pluginId: host.plugin_id,
        }),
      ),
    );
    const nextSnapshots: Record<string, OpenClawPluginChannelSnapshotResult> = {};
    for (const result of snapshotResults) {
      if (result.status !== "fulfilled") {
        continue;
      }
      nextSnapshots[result.value.snapshot.channelId || result.value.entryPath] = result.value;
    }
    setPluginChannelSnapshots(nextSnapshots);
    setPluginChannelSnapshotsError(
      snapshotResults.some((result) => result.status !== "fulfilled") ? "部分账号快照暂时不可用" : "",
    );
  }

  function formatCompactDateTime(value: string | null | undefined) {
    const normalized = String(value || "").trim();
    if (!normalized) return "未知时间";
    const date = new Date(normalized);
    if (Number.isNaN(date.getTime())) {
      return normalized;
    }
    const year = date.getUTCFullYear();
    const month = String(date.getUTCMonth() + 1).padStart(2, "0");
    const day = String(date.getUTCDate()).padStart(2, "0");
    const hours = String(date.getUTCHours()).padStart(2, "0");
    const minutes = String(date.getUTCMinutes()).padStart(2, "0");
    return `${year}-${month}-${day} ${hours}:${minutes}`;
  }

  const primaryPluginChannelHost = pluginChannelHosts.find((host) => host.channel === "feishu") ?? pluginChannelHosts[0] ?? null;
  const primaryPluginChannelSnapshot =
    (primaryPluginChannelHost ? pluginChannelSnapshots[primaryPluginChannelHost.channel] : null) ??
    Object.values(pluginChannelSnapshots)[0] ??
    null;
  const hasReadyOfficialFeishuPlugin =
    pluginChannelHosts.some((host) => host.status === "ready") ||
    (feishuSetupProgress?.plugin_installed === true && officialFeishuRuntimeStatus?.running === true);
  const hasErroredOfficialFeishuPlugin = pluginChannelHosts.some((host) => host.status === "error");
  const hasInstalledOfficialFeishuPlugin =
    pluginChannelHosts.length > 0 || feishuSetupProgress?.plugin_installed === true;
  const pendingFeishuPairingCount = feishuSetupProgress?.pending_pairings ?? feishuPairingRequests.filter((request) => request.status === "pending").length;
  const pendingFeishuPairingRequest =
    feishuPairingRequests.find((request) => request.status === "pending") ?? null;
  const feishuOnboardingState = buildFeishuOnboardingState({
    summaryState: feishuSetupProgress?.summary_state ?? null,
    setupProgress: feishuSetupProgress,
    installerMode: feishuInstallerSession.mode ?? null,
  });
  const [feishuOnboardingPanelMode, setFeishuOnboardingPanelMode] = useState<"guided" | "skipped">("guided");
  const [feishuOnboardingSelectedPath, setFeishuOnboardingSelectedPath] = useState<
    "existing_robot" | "create_robot" | null
  >(null);
  const [feishuOnboardingSkippedSignature, setFeishuOnboardingSkippedSignature] = useState<string | null>(null);
  const feishuOnboardingProgressSignature = [
    feishuOnboardingState.currentStep,
    feishuOnboardingState.mode,
    feishuOnboardingState.canContinue ? "continue" : "blocked",
    feishuOnboardingState.skipped ? "backend-skipped" : "active",
  ].join("|");
  const feishuOnboardingIsSkipped =
    feishuOnboardingState.skipped ||
    (feishuOnboardingPanelMode === "skipped" && feishuOnboardingSkippedSignature === feishuOnboardingProgressSignature);
  const feishuOnboardingBackendBranch =
    feishuOnboardingState.currentStep === "existing_robot" || feishuOnboardingState.currentStep === "create_robot"
      ? feishuOnboardingState.currentStep
      : null;
  const feishuOnboardingEffectiveBranch = feishuOnboardingSelectedPath ?? feishuOnboardingBackendBranch;
  const feishuOnboardingHeaderStep = feishuOnboardingBackendBranch
    ? feishuOnboardingEffectiveBranch ?? feishuOnboardingState.currentStep
    : feishuOnboardingState.currentStep;
  const feishuOnboardingHeaderMode = feishuOnboardingEffectiveBranch ?? feishuOnboardingState.mode;
  const feishuOnboardingPanelDisplay = resolveFeishuOnboardingPanelDisplay(
    feishuOnboardingState,
    feishuOnboardingIsSkipped,
    feishuOnboardingEffectiveBranch,
  );
  const showFeishuInstallerGuidedPanel = shouldShowFeishuInstallerGuidedPanel(
    feishuOnboardingEffectiveBranch,
    feishuInstallerSession,
  );
  const feishuGuidedInlineError = resolveFeishuGuidedInlineError(
    feishuConnectorError,
    feishuOnboardingHeaderStep,
    feishuOnboardingEffectiveBranch,
  );
  const feishuGuidedInlineNotice = resolveFeishuGuidedInlineNotice(
    feishuConnectorNotice,
    feishuOnboardingHeaderStep,
    feishuOnboardingEffectiveBranch,
  );
  const feishuAuthorizationInlineError = resolveFeishuAuthorizationInlineError(feishuConnectorError);
  const feishuInstallerDisplayMode = feishuInstallerSession.mode ?? feishuInstallerStartingMode;
  const feishuInstallerFlowLabel = resolveFeishuInstallerFlowLabel(feishuInstallerDisplayMode);
  const feishuInstallerQrBlock = extractFeishuInstallerQrBlock(feishuInstallerSession.recent_output);
  const feishuInstallerDisplayLines = sanitizeFeishuInstallerDisplayLines(feishuInstallerSession.recent_output);
  const feishuInstallerStartupHint = feishuInstallerBusy && feishuInstallerStartingMode
    ? `正在启动${resolveFeishuInstallerFlowLabel(feishuInstallerStartingMode)}，请稍候...`
    : null;
  useEffect(() => {
    if (
      feishuOnboardingPanelMode === "skipped" &&
      feishuOnboardingSkippedSignature &&
      feishuOnboardingSkippedSignature !== feishuOnboardingProgressSignature
    ) {
      setFeishuOnboardingPanelMode("guided");
      setFeishuOnboardingSkippedSignature(null);
    }
  }, [feishuOnboardingPanelMode, feishuOnboardingProgressSignature, feishuOnboardingSkippedSignature]);
  const feishuAuthorizationAction = getFeishuAuthorizationAction();
  const feishuRoutingStatus = getFeishuRoutingStatus();
  const feishuRoutingActionAvailable = Boolean(onOpenEmployees);
  const feishuOnboardingPrimaryActionLabel = feishuOnboardingIsSkipped
    ? "重新打开引导"
    : feishuOnboardingHeaderStep === "environment"
      ? retryingFeishuConnector
        ? "检测中..."
        : "重新检测环境"
      : feishuOnboardingHeaderStep === "plugin"
        ? installingOfficialFeishuPlugin
          ? "安装中..."
          : "安装官方插件"
      : feishuOnboardingHeaderStep === "create_robot"
        ? feishuInstallerBusy && feishuInstallerStartingMode === "create"
          ? "启动中..."
          : feishuOnboardingPanelDisplay.primaryActionLabel
      : feishuOnboardingHeaderStep === "authorize"
        ? retryingFeishuConnector || installingOfficialFeishuPlugin
          ? feishuAuthorizationAction.busyLabel
          : feishuAuthorizationAction.label
        : feishuOnboardingHeaderStep === "approve_pairing"
          ? feishuPairingActionLoading === "approve"
            ? "批准中..."
            : "批准这次接入"
        : feishuOnboardingHeaderStep === "routing"
          ? feishuRoutingActionAvailable
            ? feishuRoutingStatus.actionLabel
            : "请从员工中心继续"
          : feishuOnboardingPanelDisplay.primaryActionLabel;
  const feishuOnboardingPrimaryActionDisabled = feishuOnboardingHeaderStep === "existing_robot"
    ? validatingFeishuCredentials
    : feishuOnboardingHeaderStep === "create_robot"
      ? feishuInstallerBusy
      : feishuOnboardingHeaderStep === "environment"
        ? retryingFeishuConnector
        : feishuOnboardingHeaderStep === "plugin"
          ? installingOfficialFeishuPlugin
        : feishuOnboardingHeaderStep === "authorize"
          ? retryingFeishuConnector || installingOfficialFeishuPlugin
          : feishuOnboardingHeaderStep === "approve_pairing"
            ? feishuPairingActionLoading !== null || !pendingFeishuPairingRequest
          : feishuOnboardingHeaderStep === "routing"
            ? !feishuRoutingActionAvailable
          : false;

  function getFeishuSetupSummary() {
    if (feishuOnboardingState.skipped) {
      return {
        title: "已跳过飞书引导，可以继续使用其他功能",
        description: "飞书接入暂时不会阻塞设置窗口里的其他入口，后续仍可随时回来继续完成。",
        primaryActionLabel: "继续使用",
      };
    }
    const summaryState = feishuSetupProgress?.summary_state;
    if (summaryState === "env_missing") {
      return {
        title: "这台电脑还没有准备好飞书连接环境",
        description: "请先安装 Node.js LTS，完成后回到这里重新检测环境。",
        primaryActionLabel: "重新检测环境",
      };
    }
    if (summaryState === "plugin_not_installed") {
      return {
        title: "先安装飞书官方插件，再继续机器人接入",
        description: "当前电脑还没有安装飞书官方插件。安装完成后，才能继续新建机器人或绑定已有机器人。",
        primaryActionLabel: "安装官方插件",
      };
    }
    if (summaryState === "plugin_starting") {
      const authApproved = feishuSetupProgress?.auth_status === "approved";
      return {
        title: authApproved ? "正在恢复飞书连接" : "机器人信息已准备好，下一步启动飞书连接组件",
        description: authApproved
          ? "WorkClaw 会自动尝试恢复上次已接通的飞书连接；如果恢复失败，再手动点击“启动连接”。"
          : "可以继续安装并启动官方插件，然后回到飞书完成授权。",
        primaryActionLabel: authApproved ? "重新启动连接" : "安装并启动",
      };
    }
    if (summaryState === "awaiting_auth") {
      return {
        title: "飞书连接已启动，还需要完成飞书授权",
        description: "请回到飞书中的机器人会话完成授权，然后回到这里刷新状态。",
        primaryActionLabel: "刷新授权状态",
      };
    }
    if (summaryState === "awaiting_pairing_approval") {
      return {
        title: "飞书里已有新的接入请求，WorkClaw 还需要你批准",
        description: "机器人已经返回 pairing code。请在这里批准这次接入请求，批准后它才能真正开始收发消息。",
        primaryActionLabel: "批准这次接入",
      };
    }
    if (summaryState === "ready_for_routing") {
      return {
        title: "飞书已接通，还需要设置谁来接待消息",
        description: "接待员工或群聊范围未完全配置，完成后才能稳定接待飞书消息。",
        primaryActionLabel: "查看接待设置",
      };
    }
    if (summaryState === "runtime_error") {
      return {
        title: "飞书连接出现异常，需要重新检查",
        description: summarizeConnectorIssue(feishuSetupProgress?.runtime_last_error || officialFeishuRuntimeStatus?.last_error),
        primaryActionLabel: "重新检测",
      };
    }
    if (summaryState === "ready_to_bind") {
      return {
        title: "官方插件已准备好，下一步请选择接入方式",
        description: "你可以直接新建机器人，也可以切换到绑定已有机器人后继续完成接入。",
        primaryActionLabel: "继续设置",
      };
    }
    if (feishuSetupProgress?.runtime_running) {
      return {
        title: "飞书已接通，正在接收消息",
        description: "后续可以继续调整接待员工、群聊规则和高级设置。",
        primaryActionLabel: "查看连接详情",
      };
    }
    return {
      title: "飞书还未接入，完成下方步骤后即可开始接待消息",
      description: "请先安装官方插件，再创建机器人或绑定已有机器人，最后完成授权和接待设置。",
      primaryActionLabel: "开始设置",
    };
  }

  function getFeishuEnvironmentLabel(ready: boolean, fallback: string) {
    return ready ? "已就绪" : fallback;
  }

  function getFeishuAuthorizationAction() {
    if (officialFeishuRuntimeStatus?.running) {
      return {
        label: "重新启动连接",
        busyLabel: "处理中...",
      };
    }
    if (feishuSetupProgress?.plugin_installed) {
      return {
        label: "启动连接",
        busyLabel: "处理中...",
      };
    }
    return {
      label: "安装并启动",
      busyLabel: "处理中...",
    };
  }

  function getFeishuRoutingStatus() {
    const authApproved = feishuSetupProgress?.auth_status === "approved";
    const hasDefaultEmployee = Boolean(feishuSetupProgress?.default_routing_employee_name?.trim());
    const scopedCount = feishuSetupProgress?.scoped_routing_count ?? 0;

    if (!authApproved) {
      return {
        label: "待完成授权",
        description: "先回到飞书完成授权，接待设置才会生效。",
        actionLabel: "先完成授权",
      };
    }

    if (hasDefaultEmployee) {
      return {
        label: scopedCount > 0 ? "已可接待" : "已可接待",
        description:
          scopedCount > 0
            ? `默认接待员工和 ${scopedCount} 条群聊范围规则都已生效。`
            : "默认接待员工已设置，未命中规则的消息会自动回退给该员工。",
        actionLabel: scopedCount > 0 ? "调整接待设置" : "查看接待员工",
      };
    }

    if (scopedCount > 0) {
      return {
        label: "部分可接待",
        description: `已配置 ${scopedCount} 条群聊范围规则，但还没有默认接待员工。未命中规则的消息暂时无人接待。`,
        actionLabel: "补充默认员工",
      };
    }

    return {
      label: "还差默认员工",
      description: "飞书已接通，但还没有默认接待员工或群聊范围规则，消息暂时不会稳定分配。",
      actionLabel: "去设置接待员工",
    };
  }

  function renderFeishuAdvancedField(field: FeishuAdvancedFieldConfig) {
    const value = feishuAdvancedSettings[field.key];
    const updateValue = (nextValue: string) =>
      setFeishuAdvancedSettings((state) => ({
        ...state,
        [field.key]: nextValue,
      }));

    return (
      <label key={field.key} className="space-y-1.5">
        <div className="flex items-center justify-between gap-3">
          <div className="text-[11px] font-medium text-gray-700">{field.label}</div>
          <div className="text-[10px] text-gray-400">{field.kind === "textarea" ? "JSON / 模板" : "文本值"}</div>
        </div>
        <div className="text-[11px] leading-5 text-gray-500">{field.description}</div>
        {field.kind === "textarea" ? (
          <textarea
            aria-label={field.label}
            value={value}
            onChange={(event) => updateValue(event.target.value)}
            rows={field.rows ?? 5}
            className="w-full rounded border border-gray-200 bg-gray-50 px-3 py-2 font-mono text-[11px] text-gray-900"
          />
        ) : (
          <input
            aria-label={field.label}
            value={value}
            onChange={(event) => updateValue(event.target.value)}
            className="w-full rounded border border-gray-200 bg-gray-50 px-3 py-2 text-[11px] text-gray-900"
          />
        )}
      </label>
    );
  }

  useEffect(() => {
    if (activeTab !== "feishu" || !feishuInstallerSession.running) {
      return;
    }
    const timer = window.setInterval(() => {
      void Promise.all([
        loadFeishuInstallerSessionStatus(),
        loadConnectorSettings(),
        loadFeishuSetupProgress(),
      ]);
    }, 1500);
    return () => window.clearInterval(timer);
  }, [activeTab, feishuInstallerSession.running]);

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    const completionNotice = resolveFeishuInstallerCompletionNotice(feishuInstallerSession);
    if (!completionNotice) {
      return;
    }

    const completionKey = [
      feishuInstallerSession.mode ?? "",
      feishuInstallerSession.last_output_at ?? "",
      feishuInstallerSession.last_error ?? "",
      getLatestInstallerOutputLine(feishuInstallerSession),
    ].join("|");
    if (!completionKey || handledFeishuInstallerCompletionRef.current === completionKey) {
      return;
    }
    handledFeishuInstallerCompletionRef.current = completionKey;

    void Promise.all([
      loadConnectorSettings(),
      loadConnectorStatuses(),
      loadFeishuSetupProgress(),
    ]).finally(() => {
      setFeishuConnectorNotice(completionNotice);
    });
  }, [activeTab, feishuInstallerSession]);

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

  function summarizeConnectorIssue(rawIssue: string | null | undefined) {
    const normalized = String(rawIssue || "").trim().toLowerCase();
    if (!normalized) {
      return "无";
    }
    if (normalized.includes("signature mismatch")) {
      return "签名校验失败";
    }
    if (normalized.includes("secret") || normalized.includes("credential") || normalized.includes("token")) {
      return "凭据配置异常";
    }
    if (normalized.includes("timeout")) {
      return "连接超时";
    }
    if (normalized.includes("refused") || normalized.includes("unreachable") || normalized.includes("network")) {
      return "连接不可用";
    }
    return "连接异常";
  }

  function resolveFeishuConnectorStatus() {
    if (officialFeishuRuntimeStatus?.running) {
      return {
        dotClass: "bg-emerald-500",
        stateLabel: "运行中",
        label: "飞书官方插件运行中",
        detail: "官方插件宿主已启动，正在按 OpenClaw 兼容模式接收飞书消息。",
        error: officialFeishuRuntimeStatus.last_error ?? "",
      };
    }
    return {
      dotClass: "bg-gray-300",
      stateLabel: "未启动",
      label: "未启动",
      detail: hasInstalledOfficialFeishuPlugin
        ? "官方插件宿主尚未启动。保存配置或刷新插件状态后会尝试拉起运行时。"
        : "当前飞书只支持官方插件主路径。请先安装或绑定飞书官方插件，再启动运行时。",
      error: officialFeishuRuntimeStatus?.last_error ?? "",
    };
  }

  function applyOfficialFeishuRuntimeStatus(
    status: OpenClawPluginFeishuRuntimeStatus | null | undefined,
    options?: { showStartErrorNotice?: boolean },
  ) {
    if (!status) {
      return;
    }
    setOfficialFeishuRuntimeStatus(status);
    if (options?.showStartErrorNotice && !status.running && status.last_error?.trim()) {
      setFeishuConnectorError(`官方插件启动失败: ${status.last_error.trim()}`);
    }
  }

  function summarizeOfficialFeishuRuntimeLogs(status: OpenClawPluginFeishuRuntimeStatus | null | undefined) {
    const logs = Array.isArray(status?.recent_logs) ? status?.recent_logs.filter((entry) => String(entry ?? "").trim()) : [];
    if (!logs || logs.length === 0) {
      return "暂无";
    }
    return logs.slice(-3).join(" | ");
  }

  function getFeishuConnectionDetailSummary() {
    const status = resolveFeishuConnectorStatus();
    if (status.error?.trim()) {
      return summarizeConnectorIssue(status.error);
    }
    if (feishuSetupProgress?.auth_status !== "approved") {
      return "连接已启动，但还需要在飞书里完成授权。";
    }
    if (!feishuSetupProgress?.default_routing_employee_name && (feishuSetupProgress?.scoped_routing_count ?? 0) === 0) {
      return "连接正常，但还没有设置默认接待员工或群聊范围。";
    }
    if (officialFeishuRuntimeStatus?.running) {
      return "连接正常，正在接收飞书消息。";
    }
    return "当前连接尚未启动。";
  }

  function buildFeishuDiagnosticSummary() {
    const status = resolveFeishuConnectorStatus();
    const lines = [
      `当前状态: ${status.label}`,
      `插件版本: ${feishuSetupProgress?.plugin_version || primaryPluginChannelHost?.version || "未识别"}`,
      `当前接入账号: ${primaryPluginChannelSnapshot?.snapshot.defaultAccountId || "未识别"}`,
      `授权状态: ${feishuSetupProgress?.auth_status === "approved" ? "已完成" : "待完成"}`,
      `默认接待员工: ${feishuSetupProgress?.default_routing_employee_name || "未设置"}`,
      `群聊范围规则: ${feishuSetupProgress?.scoped_routing_count ?? 0} 条`,
      `最近一次事件: ${formatCompactDateTime(officialFeishuRuntimeStatus?.last_event_at)}`,
      `诊断摘要: ${getFeishuConnectionDetailSummary()}`,
      `最近日志: ${summarizeOfficialFeishuRuntimeLogs(officialFeishuRuntimeStatus)}`,
    ];
    return lines.join("\n");
  }

  async function handleValidateFeishuCredentials() {
    const appId = feishuConnectorSettings.app_id.trim();
    const appSecret = feishuConnectorSettings.app_secret.trim();
    if (!appId || !appSecret) {
      setFeishuConnectorError("请先填写已有机器人的 App ID 和 App Secret");
      return;
    }

    setValidatingFeishuCredentials(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      const probe = await invoke<OpenClawPluginFeishuCredentialProbeResult>(
        "probe_openclaw_plugin_feishu_credentials",
        {
          appId,
          appSecret,
        },
      );
      if (!probe.ok) {
        setFeishuCredentialProbe(null);
        setFeishuConnectorError(`已有机器人校验失败: ${probe.error || "无法获取机器人信息"}`);
        return;
      }
      setFeishuCredentialProbe(probe);
      const botLabel = probe.bot_name?.trim() ? `（${probe.bot_name.trim()}）` : "";
      setFeishuConnectorNotice(`机器人信息验证成功${botLabel}`);
    } catch (error) {
      setFeishuCredentialProbe(null);
      setFeishuConnectorError("验证机器人信息失败: " + String(error));
    } finally {
      setValidatingFeishuCredentials(false);
    }
  }

  async function handleSaveFeishuConnector() {
    setSavingFeishuConnector(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      await invoke("set_feishu_gateway_settings", {
        settings: feishuConnectorSettings,
      });
      const saved = await invoke<FeishuGatewaySettings>("get_feishu_gateway_settings");
      setFeishuConnectorSettings(saved);
      await loadConnectorStatuses();
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice("飞书官方插件配置已保存");
    } catch (error) {
      setFeishuConnectorError("保存飞书官方插件配置失败: " + String(error));
    } finally {
      setSavingFeishuConnector(false);
    }
  }

  async function handleSaveFeishuAdvancedSettings() {
    setSavingFeishuAdvancedSettings(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      const saved = await invoke<OpenClawPluginFeishuAdvancedSettings>(
        "set_openclaw_plugin_feishu_advanced_settings",
        {
          settings: {
            groups_json: feishuAdvancedSettings.groups_json,
            dms_json: feishuAdvancedSettings.dms_json,
            footer_json: feishuAdvancedSettings.footer_json,
            account_overrides_json: feishuAdvancedSettings.account_overrides_json,
            render_mode: feishuAdvancedSettings.render_mode,
            streaming: feishuAdvancedSettings.streaming,
            text_chunk_limit: feishuAdvancedSettings.text_chunk_limit,
            chunk_mode: feishuAdvancedSettings.chunk_mode,
            reply_in_thread: feishuAdvancedSettings.reply_in_thread,
            group_session_scope: feishuAdvancedSettings.group_session_scope,
            topic_session_mode: feishuAdvancedSettings.topic_session_mode,
            markdown_mode: feishuAdvancedSettings.markdown_mode,
            markdown_table_mode: feishuAdvancedSettings.markdown_table_mode,
            heartbeat_visibility: feishuAdvancedSettings.heartbeat_visibility,
            heartbeat_interval_ms: feishuAdvancedSettings.heartbeat_interval_ms,
            media_max_mb: feishuAdvancedSettings.media_max_mb,
            http_timeout_ms: feishuAdvancedSettings.http_timeout_ms,
            config_writes: feishuAdvancedSettings.config_writes,
            webhook_host: feishuAdvancedSettings.webhook_host,
            webhook_port: feishuAdvancedSettings.webhook_port,
            dynamic_agent_creation_enabled: feishuAdvancedSettings.dynamic_agent_creation_enabled,
            dynamic_agent_creation_workspace_template:
              feishuAdvancedSettings.dynamic_agent_creation_workspace_template,
            dynamic_agent_creation_agent_dir_template:
              feishuAdvancedSettings.dynamic_agent_creation_agent_dir_template,
            dynamic_agent_creation_max_agents:
              feishuAdvancedSettings.dynamic_agent_creation_max_agents,
          },
        },
      );
      setFeishuAdvancedSettings(saved);
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice("飞书高级配置已保存");
    } catch (error) {
      setFeishuConnectorError("保存飞书高级配置失败: " + String(error));
    } finally {
      setSavingFeishuAdvancedSettings(false);
    }
  }

  async function handleStartFeishuInstaller(mode: OpenClawLarkInstallerMode) {
    setFeishuInstallerBusy(true);
    setFeishuInstallerStartingMode(mode);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      if (!hasInstalledOfficialFeishuPlugin) {
        await invoke<OpenClawPluginInstallRecord>("install_openclaw_plugin_from_npm", {
          pluginId: "openclaw-lark",
          npmSpec: "@larksuite/openclaw-lark",
        });
      }
      const status = await invoke<OpenClawLarkInstallerSessionStatus>("start_openclaw_lark_installer_session", {
        mode,
        appId: mode === "link" ? feishuConnectorSettings.app_id.trim() : null,
        appSecret: mode === "link" ? feishuConnectorSettings.app_secret.trim() : null,
      });
      setFeishuInstallerSession(status);
      setFeishuInstallerInput("");
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice(mode === "create" ? "已启动飞书官方创建机器人向导" : "已启动飞书官方绑定机器人向导");
    } catch (error) {
      setFeishuConnectorError(
        `${mode === "create" ? "启动飞书官方创建机器人向导" : "启动飞书官方绑定机器人向导"}失败: ${String(error)}`,
      );
    } finally {
      setFeishuInstallerBusy(false);
      setFeishuInstallerStartingMode(null);
    }
  }

  async function handleSendFeishuInstallerInput() {
    const input = feishuInstallerInput.trim();
    if (!input) return;
    setFeishuInstallerBusy(true);
    setFeishuConnectorError("");
    try {
      const status = await invoke<OpenClawLarkInstallerSessionStatus>("send_openclaw_lark_installer_input", {
        input,
      });
      setFeishuInstallerSession(status);
      setFeishuInstallerInput("");
    } catch (error) {
      setFeishuConnectorError("发送安装向导输入失败: " + String(error));
    } finally {
      setFeishuInstallerBusy(false);
    }
  }

  async function handleStopFeishuInstallerSession() {
    setFeishuInstallerBusy(true);
    setFeishuConnectorError("");
    try {
      const status = await invoke<OpenClawLarkInstallerSessionStatus>("stop_openclaw_lark_installer_session");
      setFeishuInstallerSession(status);
      setFeishuConnectorNotice("已停止飞书官方安装向导");
    } catch (error) {
      setFeishuConnectorError("停止飞书官方安装向导失败: " + String(error));
    } finally {
      setFeishuInstallerBusy(false);
    }
  }

  async function handleRetryFeishuConnector() {
    setRetryingFeishuConnector(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      const runtimeStatus = await invoke<OpenClawPluginFeishuRuntimeStatus | null>("start_openclaw_plugin_feishu_runtime", {
        pluginId: primaryPluginChannelHost?.plugin_id || "openclaw-lark",
        accountId: null,
      });
      if (runtimeStatus) {
        applyOfficialFeishuRuntimeStatus(runtimeStatus, { showStartErrorNotice: true });
      } else {
        await loadConnectorStatuses();
      }
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice(
        runtimeStatus ? (runtimeStatus.running ? "已触发飞书官方插件启动" : "已刷新飞书官方插件状态") : "已触发飞书官方插件启动",
      );
    } catch (error) {
      setFeishuConnectorError("刷新飞书官方插件状态失败: " + String(error));
    } finally {
      setRetryingFeishuConnector(false);
    }
  }

  async function handleInstallOfficialFeishuPlugin() {
    setInstallingOfficialFeishuPlugin(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      await invoke<OpenClawPluginInstallRecord>("install_openclaw_plugin_from_npm", {
        pluginId: "openclaw-lark",
        npmSpec: "@larksuite/openclaw-lark",
      });
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice("飞书官方插件已安装");
    } catch (error) {
      setFeishuConnectorError("安装飞书官方插件失败: " + String(error));
    } finally {
      setInstallingOfficialFeishuPlugin(false);
    }
  }

  async function handleResolveFeishuPairingRequest(requestId: string, action: "approve" | "deny") {
    setFeishuPairingActionLoading(action);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      await invoke<FeishuPairingRequestRecord>(
        action === "approve" ? "approve_feishu_pairing_request" : "deny_feishu_pairing_request",
        {
          requestId,
          resolvedByUser: "settings-ui",
        },
      );
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice(action === "approve" ? "已批准飞书接入请求" : "已拒绝飞书接入请求");
    } catch (error) {
      setFeishuConnectorError(`${action === "approve" ? "批准" : "拒绝"}飞书接入请求失败: ${String(error)}`);
    } finally {
      setFeishuPairingActionLoading(null);
    }
  }

  async function handleInstallAndStartFeishuConnector() {
    setRetryingFeishuConnector(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      if (!feishuConnectorSettings.app_id.trim() || !feishuConnectorSettings.app_secret.trim()) {
        setFeishuConnectorError("请先填写并保存已有机器人的 App ID 和 App Secret");
        return;
      }

      await invoke("set_feishu_gateway_settings", {
        settings: feishuConnectorSettings,
      });
      const saved = await invoke<FeishuGatewaySettings>("get_feishu_gateway_settings");
      setFeishuConnectorSettings(saved);

      if (!hasInstalledOfficialFeishuPlugin) {
        await invoke<OpenClawPluginInstallRecord>("install_openclaw_plugin_from_npm", {
          pluginId: "openclaw-lark",
          npmSpec: "@larksuite/openclaw-lark",
        });
      }

      const runtimeStatus = await invoke<OpenClawPluginFeishuRuntimeStatus | null>("start_openclaw_plugin_feishu_runtime", {
        pluginId: primaryPluginChannelHost?.plugin_id || "openclaw-lark",
        accountId: null,
      });
      if (runtimeStatus) {
        applyOfficialFeishuRuntimeStatus(runtimeStatus, { showStartErrorNotice: true });
      }
      await loadConnectorStatuses();
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice(runtimeStatus?.running ? "飞书连接组件已启动" : "已尝试启动飞书连接组件");
    } catch (error) {
      setFeishuConnectorError("安装并启动飞书连接失败: " + String(error));
    } finally {
      setRetryingFeishuConnector(false);
    }
  }

  async function handleOpenFeishuOfficialDocs() {
    try {
      await openExternalUrl(FEISHU_OFFICIAL_PLUGIN_DOC_URL);
    } catch (error) {
      setFeishuConnectorError(getErrorMessage(error, "打开官方文档失败，请稍后重试"));
    }
  }

  async function handleCopyFeishuDiagnostics() {
    try {
      await navigator?.clipboard?.writeText?.(buildFeishuDiagnosticSummary());
      setFeishuConnectorNotice("连接诊断摘要已复制");
    } catch (error) {
      setFeishuConnectorError(getErrorMessage(error, "复制连接诊断摘要失败，请稍后重试"));
    }
  }

  async function handleRefreshFeishuSetup() {
    setRetryingFeishuConnector(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      await Promise.all([
        loadConnectorSettings(),
        loadConnectorStatuses(),
        loadConnectorPlatformData(),
        loadFeishuSetupProgress(),
      ]);
    } catch (error) {
      setFeishuConnectorError("刷新飞书接入状态失败: " + String(error));
    } finally {
      setRetryingFeishuConnector(false);
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
    setSearchForm((current) => applySearchPresetToForm(value, current));
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
    const validationError = validateSearchConfigForm(searchForm);
    if (validationError) {
      setSearchError(validationError);
      setSearchTestResult(null);
      return;
    }
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
      setSearchForm(EMPTY_SEARCH_CONFIG_FORM);
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
      setSearchForm(EMPTY_SEARCH_CONFIG_FORM);
      setSearchError("");
      setSearchTestResult(null);
    }
    loadSearchConfigs();
  }

  const inputCls = "sm-input w-full text-sm py-1.5";
  const labelCls = "sm-field-label";
  const parsedMcpEnv = parseMcpEnvJson(mcpForm.env);
  const mcpApiKeyEnvKeys = Object.keys(parsedMcpEnv.env).filter((key) => key.toUpperCase().includes("API_KEY"));
  const feishuSetupSummary = getFeishuSetupSummary();
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
    <SettingsShell
      onClose={onClose}
      tabs={
        <SettingsTabNav
          activeTab={activeTab}
          onSelectTab={setActiveTab}
          showCapabilityRoutingSettings={SHOW_CAPABILITY_ROUTING_SETTINGS}
          showHealthSettings={SHOW_HEALTH_SETTINGS}
          showMcpSettings={SHOW_MCP_SETTINGS}
          showAutoRoutingSettings={SHOW_AUTO_ROUTING_SETTINGS}
        />
      }
    >

      {activeTab === "models" && (
        <div className="space-y-4">
          <ModelsSettingsSection
            models={models}
            providers={providers}
            onModelsChange={setModels}
            onProvidersChange={setProviders}
            showDevModelSetupTools={showDevModelSetupTools}
            onDevResetFirstUseOnboarding={onDevResetFirstUseOnboarding}
            onDevOpenQuickModelSetup={onDevOpenQuickModelSetup}
          />
        </div>
      )}
      <DesktopSettingsSection models={models} visible={activeTab === "desktop"} />

      {activeTab === "models" && showDevModelSetupTools && (
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
              placeholder="例如: deepseek-chat / qwen3.5-plus / kimi-k2"
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
                  setSearchForm(EMPTY_SEARCH_CONFIG_FORM);
                  setSearchError("");
                  setSearchTestResult(null);
                }}
                className="text-xs text-gray-400 hover:text-gray-600"
              >
                取消编辑
              </button>
            )}
          </div>
          <SearchConfigForm
            form={searchForm}
            onFormChange={setSearchForm}
            onApplyPreset={applySearchPreset}
            showApiKey={showSearchApiKey}
            onToggleApiKey={() => setShowSearchApiKey((value) => !value)}
            error={searchError}
            testResult={searchTestResult}
            testing={searchTesting}
            saving={false}
            onTest={handleTestSearch}
            onSave={handleSaveSearch}
            labelClassName={labelCls}
            inputClassName={inputCls}
            panelClassName="space-y-3"
            actionClassName="flex gap-2 pt-1"
            saveLabel={editingSearchId ? "保存修改" : "保存"}
          />
        </div>
      </>)}

      {activeTab === "feishu" && (
        <div className="space-y-3">
          <div className="bg-white rounded-lg p-4 space-y-4">
            <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
              <div className="space-y-1">
                <div className="text-sm font-medium text-gray-900">飞书连接</div>
                <div className="text-xs text-gray-500">先完成机器人接入，再安装飞书官方插件并完成授权，最后补充接待员工设置。</div>
              </div>
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => void handleRefreshFeishuSetup()}
                  disabled={retryingFeishuConnector}
                  className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
                >
                  {retryingFeishuConnector ? "检测中..." : "重新检测"}
                </button>
                <button
                  type="button"
                  onClick={() => void handleOpenFeishuOfficialDocs()}
                  className="inline-flex h-8 items-center rounded border border-blue-200 bg-blue-50 px-3 text-xs text-blue-700 hover:bg-blue-100"
                >
                  查看官方文档
                </button>
              </div>
            </div>

            {feishuConnectorError && !feishuGuidedInlineError && !feishuAuthorizationInlineError ? (
              <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700">
                {feishuConnectorError}
              </div>
            ) : null}
            {feishuConnectorNotice && !feishuGuidedInlineNotice ? (
              <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-700">
                {feishuConnectorNotice}
              </div>
            ) : null}

            <div className="rounded-xl border border-blue-200 bg-blue-50 p-4">
              <div className="text-base font-medium text-blue-950">{feishuSetupSummary.title}</div>
              <div className="mt-1 text-sm text-blue-900">{feishuSetupSummary.description}</div>
              <div className="mt-3 grid grid-cols-1 gap-2 md:grid-cols-4">
                <div className="rounded border border-blue-100 bg-white/80 px-3 py-2">
                  <div className="text-[11px] text-blue-700">运行环境</div>
                  <div className="text-sm font-medium text-gray-900">
                    {feishuEnvironmentStatus?.can_start_runtime ? "已准备好" : "待检查"}
                  </div>
                </div>
                <div className="rounded border border-blue-100 bg-white/80 px-3 py-2">
                  <div className="text-[11px] text-blue-700">机器人信息</div>
                  <div className="text-sm font-medium text-gray-900">
                    {feishuSetupProgress?.credentials_configured ? "已填写" : "未填写"}
                  </div>
                </div>
                <div className="rounded border border-blue-100 bg-white/80 px-3 py-2">
                  <div className="text-[11px] text-blue-700">连接组件</div>
                  <div className="text-sm font-medium text-gray-900">
                    {feishuSetupProgress?.plugin_installed ? "已安装" : "未安装"}
                  </div>
                </div>
                <div className="rounded border border-blue-100 bg-white/80 px-3 py-2">
                  <div className="text-[11px] text-blue-700">授权与接待</div>
                  <div className="text-sm font-medium text-gray-900">
                    {feishuRoutingStatus.label}
                  </div>
                </div>
              </div>
              <div
                data-testid="feishu-onboarding-state"
                data-current-step={feishuOnboardingState.currentStep}
                data-skipped={String(feishuOnboardingState.skipped)}
                className="mt-3 rounded border border-blue-100 bg-white/70 px-3 py-2 text-xs text-blue-900"
              >
                引导步骤：{formatFeishuOnboardingStepLabel(feishuOnboardingHeaderStep)} ·
                {feishuOnboardingHeaderStep === "create_robot"
                  ? "创建机器人"
                  : feishuOnboardingHeaderStep === "existing_robot"
                    ? "绑定已有机器人"
                    : feishuOnboardingHeaderStep === "plugin"
                      ? "安装官方插件"
                      : feishuOnboardingHeaderMode === "create_robot"
                        ? "创建机器人"
                        : "绑定已有机器人"} ·
                {feishuInstallerBusy && feishuInstallerStartingMode
                  ? "正在启动向导"
                  : feishuOnboardingState.canContinue
                    ? "可继续使用其余功能"
                    : "仍需完成当前引导"}
              </div>
              <div data-testid="feishu-onboarding-step" className="rounded-lg border border-blue-100 bg-white/90 px-3 py-3">
                {feishuOnboardingState.currentStep === "existing_robot" || feishuOnboardingState.currentStep === "create_robot" ? (
                  <div className="mb-3 flex flex-wrap items-center gap-2">
                    <button
                      type="button"
                      onClick={() => {
                        setFeishuOnboardingSelectedPath("existing_robot");
                        setFeishuOnboardingPanelMode("guided");
                        setFeishuOnboardingSkippedSignature(null);
                      }}
                      className={`h-8 px-3 rounded border text-xs ${
                        feishuOnboardingEffectiveBranch === "existing_robot"
                          ? "border-blue-600 bg-blue-600 text-white"
                          : "border-blue-200 bg-white text-blue-700 hover:bg-blue-50"
                      }`}
                    >
                      绑定已有机器人
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setFeishuOnboardingSelectedPath("create_robot");
                        setFeishuOnboardingPanelMode("guided");
                        setFeishuOnboardingSkippedSignature(null);
                      }}
                      className={`h-8 px-3 rounded border text-xs ${
                        feishuOnboardingEffectiveBranch === "create_robot"
                          ? "border-blue-600 bg-blue-600 text-white"
                          : "border-blue-200 bg-white text-blue-700 hover:bg-blue-50"
                      }`}
                    >
                      新建机器人
                    </button>
                  </div>
                ) : null}
                <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
                  <div className="space-y-1">
                    <div className="text-sm font-medium text-blue-950">{feishuOnboardingPanelDisplay.title}</div>
                    <div className="text-xs text-blue-900">{feishuOnboardingPanelDisplay.body}</div>
                  </div>
                  <div
                    className={`rounded-full border px-3 py-1 text-[11px] font-medium ${feishuOnboardingPanelDisplay.badgeClassName}`}
                  >
                    {feishuOnboardingPanelDisplay.badgeLabel}
                  </div>
                </div>
                <div className="mt-4 flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={() => {
                      if (feishuOnboardingIsSkipped) {
                        setFeishuOnboardingPanelMode("guided");
                        setFeishuOnboardingSkippedSignature(null);
                        return;
                      }
                      if (feishuOnboardingEffectiveBranch === "create_robot") {
                        void handleStartFeishuInstaller("create");
                        return;
                      }
                      if (feishuOnboardingEffectiveBranch === "existing_robot") {
                        void handleValidateFeishuCredentials();
                        return;
                      }
                      if (feishuOnboardingHeaderStep === "environment") {
                        void handleRefreshFeishuSetup();
                        return;
                      }
                      if (feishuOnboardingHeaderStep === "plugin") {
                        void handleInstallOfficialFeishuPlugin();
                        return;
                      }
                      if (feishuOnboardingHeaderStep === "authorize") {
                        void handleInstallAndStartFeishuConnector();
                        return;
                      }
                      if (feishuOnboardingHeaderStep === "approve_pairing") {
                        if (pendingFeishuPairingRequest) {
                          void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "approve");
                        }
                        return;
                      }
                      if (feishuOnboardingHeaderStep === "routing") {
                        onOpenEmployees?.();
                      }
                    }}
                    disabled={feishuOnboardingPrimaryActionDisabled}
                    className="h-8 px-3 rounded bg-blue-600 text-xs text-white hover:bg-blue-700"
                  >
                    {feishuOnboardingPrimaryActionLabel}
                  </button>
                  {!feishuOnboardingIsSkipped ? (
                    <button
                      type="button"
                      onClick={() => {
                        setFeishuOnboardingPanelMode("skipped");
                        setFeishuOnboardingSkippedSignature(feishuOnboardingProgressSignature);
                      }}
                      className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50"
                    >
                      暂时跳过
                    </button>
                  ) : null}
                  {!feishuOnboardingIsSkipped &&
                  (feishuOnboardingState.currentStep === "authorize" ||
                    feishuOnboardingState.currentStep === "approve_pairing") ? (
                    <button
                      type="button"
                      onClick={() => void handleRefreshFeishuSetup()}
                      disabled={retryingFeishuConnector}
                      className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
                    >
                      {retryingFeishuConnector ? "检测中..." : "刷新授权状态"}
                    </button>
                  ) : null}
                </div>
                {feishuGuidedInlineError ? (
                  <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700">
                    {feishuGuidedInlineError}
                  </div>
                ) : null}
                {feishuGuidedInlineNotice ? (
                  <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-700">
                    {feishuGuidedInlineNotice}
                  </div>
                ) : null}
                {!feishuOnboardingIsSkipped &&
                feishuOnboardingHeaderStep === "approve_pairing" &&
                pendingFeishuPairingRequest ? (
                  <div className="mt-3 rounded-lg border border-amber-200 bg-white p-3" data-testid="feishu-guided-pairing-panel">
                    <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
                      <div className="space-y-1">
                        <div className="text-sm font-medium text-gray-900">飞书已经发来了接入请求</div>
                        <div className="text-xs text-gray-600">
                          这一步不是再去授权，而是由 WorkClaw 批准这次接入请求。批准后，这个飞书发送者才能真正开始和机器人对话。
                        </div>
                      </div>
                      <div className="rounded-full border border-amber-200 bg-amber-50 px-3 py-1 text-[11px] font-medium text-amber-700">
                        等待批准
                      </div>
                    </div>
                    <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-3">
                      <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                        <div className="text-[11px] text-gray-500">发送者</div>
                        <div className="text-sm font-medium text-gray-900 break-all">{pendingFeishuPairingRequest.sender_id}</div>
                      </div>
                      <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                        <div className="text-[11px] text-gray-500">Pairing Code</div>
                        <div className="text-sm font-medium text-gray-900">{pendingFeishuPairingRequest.code || "未返回"}</div>
                      </div>
                      <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                        <div className="text-[11px] text-gray-500">发起时间</div>
                        <div className="text-sm font-medium text-gray-900">{formatCompactDateTime(pendingFeishuPairingRequest.created_at)}</div>
                      </div>
                    </div>
                    <div className="mt-3 flex flex-wrap gap-2">
                      <button
                        type="button"
                        onClick={() => void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "approve")}
                        disabled={feishuPairingActionLoading !== null}
                        className="h-8 px-3 rounded bg-amber-600 text-xs text-white hover:bg-amber-700 disabled:bg-amber-300"
                      >
                        {feishuPairingActionLoading === "approve" ? "批准中..." : "批准这次接入"}
                      </button>
                      <button
                        type="button"
                        onClick={() => void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "deny")}
                        disabled={feishuPairingActionLoading !== null}
                        className="h-8 px-3 rounded border border-red-200 bg-white text-xs text-red-700 hover:bg-red-50 disabled:bg-gray-100"
                      >
                        {feishuPairingActionLoading === "deny" ? "拒绝中..." : "拒绝这次接入"}
                      </button>
                    </div>
                  </div>
                ) : null}
                {feishuInstallerStartupHint && !showFeishuInstallerGuidedPanel ? (
                  <div className="mt-3 rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-2 text-xs text-indigo-700">
                    {feishuInstallerStartupHint}
                  </div>
                ) : null}
                {showFeishuInstallerGuidedPanel ? (
                  <div className="mt-3 rounded-lg border border-indigo-200 bg-white p-3" data-testid="feishu-guided-installer-panel">
                    <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
                      <div className="space-y-1">
                        <div className="text-sm font-medium text-gray-900">{feishuInstallerFlowLabel}正在这里继续</div>
                        <div className="text-xs text-gray-600">
                          不用再往下翻到高级控制台。扫码、等待结果和下一步提示都会先显示在这里。
                        </div>
                      </div>
                      <div className="rounded-full border border-indigo-200 bg-indigo-50 px-3 py-1 text-[11px] font-medium text-indigo-700">
                        {feishuInstallerBusy && feishuInstallerStartingMode ? "正在启动" : feishuInstallerSession.running ? "向导运行中" : "向导已结束"}
                      </div>
                    </div>
                    {feishuInstallerStartupHint ? (
                      <div className="mt-3 rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-2 text-xs text-indigo-700">
                        {feishuInstallerStartupHint}
                      </div>
                    ) : null}
                    <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-3">
                      <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                        <div className="text-[11px] text-gray-500">当前模式</div>
                        <div className="text-sm font-medium text-gray-900">
                          {feishuInstallerDisplayMode === "create"
                            ? "新建机器人"
                            : feishuInstallerDisplayMode === "link"
                              ? "绑定已有机器人"
                              : "未启动"}
                        </div>
                      </div>
                      <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                        <div className="text-[11px] text-gray-500">提示</div>
                        <div className="text-sm font-medium text-gray-900">
                          {feishuInstallerStartupHint || feishuInstallerSession.prompt_hint || "暂无"}
                        </div>
                      </div>
                      <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                        <div className="text-[11px] text-gray-500">下一步</div>
                        <div className="text-sm font-medium text-gray-900">
                          {feishuInstallerBusy && feishuInstallerStartingMode
                            ? "等待向导启动"
                            : feishuInstallerSession.running
                              ? "按当前提示继续"
                              : "准备启动连接并完成授权"}
                        </div>
                      </div>
                    </div>
                    {feishuInstallerQrBlock.length > 0 ? (
                      <div className="mt-3 rounded-lg border border-gray-900 bg-[#050816] px-3 py-3 text-xs text-gray-100">
                        <div className="mb-2 text-[11px] font-medium text-indigo-200">请使用飞书扫码继续</div>
                        <pre
                          data-testid="feishu-guided-installer-qr"
                          className="overflow-x-auto whitespace-pre font-mono leading-none"
                        >
                          {feishuInstallerQrBlock.join("\n")}
                        </pre>
                      </div>
                    ) : null}
                    <div className="mt-3 rounded-lg border border-gray-900 bg-[#050816] px-3 py-3 text-xs text-gray-100">
                      <div className="mb-2 text-[11px] font-medium text-indigo-200">向导输出</div>
                      <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-all font-mono">
                        {feishuInstallerDisplayLines.length > 0
                          ? feishuInstallerDisplayLines.join("\n")
                          : feishuInstallerStartupHint || "暂无安装向导输出"}
                      </pre>
                    </div>
                    <div className="mt-3 flex flex-wrap gap-2">
                      <button
                        type="button"
                        onClick={() => void handleRefreshFeishuSetup()}
                        disabled={retryingFeishuConnector}
                        className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
                      >
                        {retryingFeishuConnector ? "检测中..." : "刷新状态"}
                      </button>
                      <button
                        type="button"
                        onClick={() => void handleStopFeishuInstallerSession()}
                        disabled={feishuInstallerBusy || !feishuInstallerSession.running}
                        className="h-8 px-3 rounded border border-red-200 bg-white text-xs text-red-700 hover:bg-red-50 disabled:bg-gray-100"
                      >
                        停止向导
                      </button>
                    </div>
                  </div>
                ) : null}
                {feishuOnboardingIsSkipped ? (
                  <div className="mt-3 rounded-lg border border-blue-100 bg-white/80 px-3 py-2 text-xs text-blue-900">
                    {feishuOnboardingPanelDisplay.body}
                  </div>
                ) : null}
              </div>
            </div>
          </div>

          <details className="rounded-lg border border-gray-200 bg-white p-4">
            <summary className="cursor-pointer text-sm font-medium text-gray-900">高级设置与控制台</summary>
            <div className="mt-4 space-y-3">
              <div className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
            <div>
              <div className="text-sm font-medium text-gray-900">检查运行环境</div>
              <div className="text-xs text-gray-500 mt-1">不内置运行环境；如果电脑未安装 Node.js，系统会在这里提示你先完成安装。</div>
            </div>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-3">
                <div className="text-[11px] text-gray-500">Node.js</div>
                <div className="mt-1 text-sm font-medium text-gray-900">
                  {getFeishuEnvironmentLabel(Boolean(feishuEnvironmentStatus?.node_available), "未检测到")}
                </div>
                <div className="mt-1 text-[11px] text-gray-500">{feishuEnvironmentStatus?.node_version || "请安装 Node.js LTS"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-3">
                <div className="text-[11px] text-gray-500">npm</div>
                <div className="mt-1 text-sm font-medium text-gray-900">
                  {getFeishuEnvironmentLabel(Boolean(feishuEnvironmentStatus?.npm_available), "未检测到")}
                </div>
                <div className="mt-1 text-[11px] text-gray-500">{feishuEnvironmentStatus?.npm_version || "安装 Node.js 后通常会一起提供"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-3">
                <div className="text-[11px] text-gray-500">飞书连接组件运行条件</div>
                <div className="mt-1 text-sm font-medium text-gray-900">
                  {feishuEnvironmentStatus?.can_start_runtime ? "已准备好" : "暂未满足"}
                </div>
                <div className="mt-1 text-[11px] text-gray-500">{feishuEnvironmentStatus?.error || "完成环境检查后即可继续后续步骤"}</div>
              </div>
            </div>
            {!feishuEnvironmentStatus?.can_start_runtime ? (
              <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
                当前电脑还没有安装飞书连接所需环境。请先安装 Node.js LTS，完成后重新打开 WorkClaw 或回到这里点击“重新检测”。
              </div>
            ) : null}
          </div>

          {feishuOnboardingEffectiveBranch !== "create_robot" ? (
            <div className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
              <div>
                <div className="text-sm font-medium text-gray-900">绑定已有机器人</div>
                <div className="text-xs text-gray-500 mt-1">这里只需要填写已有机器人的 App ID 和 App Secret；当前版本不再展示 webhook 相关配置。</div>
              </div>
              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                <label className="space-y-1">
                  <div className="text-[11px] font-medium text-gray-700">App ID</div>
                  <input
                    value={feishuConnectorSettings.app_id}
                    onChange={(event) => setFeishuConnectorSettings((state) => ({ ...state, app_id: event.target.value }))}
                    className="w-full rounded border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-gray-900"
                    placeholder="cli_xxx"
                  />
                </label>
                <label className="space-y-1">
                  <div className="text-[11px] font-medium text-gray-700">App Secret</div>
                  <input
                    type="password"
                    value={feishuConnectorSettings.app_secret}
                    onChange={(event) => setFeishuConnectorSettings((state) => ({ ...state, app_secret: event.target.value }))}
                    className="w-full rounded border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-gray-900"
                    placeholder="填写机器人的 App Secret"
                  />
                </label>
              </div>
              {feishuCredentialProbe?.ok ? (
                <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-800">
                  已识别机器人
                  {feishuCredentialProbe.bot_name ? `：${feishuCredentialProbe.bot_name}` : ""}。
                  {feishuCredentialProbe.bot_open_id ? ` open_id：${feishuCredentialProbe.bot_open_id}` : ""}
                </div>
              ) : null}
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => void handleValidateFeishuCredentials()}
                  disabled={validatingFeishuCredentials}
                  className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50 disabled:bg-gray-100"
                >
                  {validatingFeishuCredentials ? "验证中..." : "验证机器人信息"}
                </button>
                <button
                  type="button"
                  onClick={() => void handleSaveFeishuConnector()}
                  disabled={savingFeishuConnector}
                  className="h-8 px-3 rounded bg-blue-600 text-xs text-white hover:bg-blue-700 disabled:bg-blue-300"
                >
                  {savingFeishuConnector ? "保存中..." : "保存并继续"}
                </button>
              </div>
            </div>
          ) : null}

          <div data-testid="feishu-authorization-step" className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
            <div>
              <div className="text-sm font-medium text-gray-900">
                {pendingFeishuPairingCount > 0 ? "批准飞书接入请求" : "完成飞书授权"}
              </div>
              <div className="text-xs text-gray-500 mt-1">
                {pendingFeishuPairingCount > 0
                  ? "飞书里的机器人已经发来了接入请求。请先在这里批准这次接入，再继续后续配置。"
                  : "安装并启动后，请回到飞书中的机器人会话按提示完成授权，然后回到这里刷新状态。"}
              </div>
            </div>
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3 text-xs text-gray-700 space-y-1">
              {pendingFeishuPairingCount > 0 ? (
                <>
                  <div>1. 飞书里已经生成了 pairing request</div>
                  <div>2. 在这里点击“批准这次接入”</div>
                  <div>3. 批准后再继续配置接待员工</div>
                </>
              ) : (
                <>
                  <div>1. 在飞书中打开机器人会话</div>
                  <div>2. 按提示完成授权</div>
                  <div>3. 如果机器人提示 access not configured，下一步回来批准接入请求</div>
                </>
              )}
            </div>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">连接组件</div>
                <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.plugin_installed ? "已安装" : "未安装"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">运行状态</div>
                <div className="text-sm font-medium text-gray-900">{officialFeishuRuntimeStatus?.running ? "运行中" : "未启动"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">授权状态</div>
                <div className="text-sm font-medium text-gray-900">
                  {pendingFeishuPairingCount > 0
                    ? "待批准接入"
                    : feishuSetupProgress?.auth_status === "approved"
                      ? "已完成"
                      : "待完成"}
                </div>
              </div>
            </div>
            {pendingFeishuPairingRequest ? (
              <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-3 text-xs text-amber-900 space-y-1">
                <div>发送者：{pendingFeishuPairingRequest.sender_id}</div>
                <div>Pairing Code：{pendingFeishuPairingRequest.code || "未返回"}</div>
                <div>发起时间：{formatCompactDateTime(pendingFeishuPairingRequest.created_at)}</div>
              </div>
            ) : null}
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => void handleInstallAndStartFeishuConnector()}
                disabled={retryingFeishuConnector || installingOfficialFeishuPlugin}
                className="h-8 px-3 rounded bg-indigo-600 text-xs text-white hover:bg-indigo-700 disabled:bg-indigo-300"
              >
                {retryingFeishuConnector || installingOfficialFeishuPlugin
                  ? feishuAuthorizationAction.busyLabel
                  : feishuAuthorizationAction.label}
              </button>
              <button
                type="button"
                onClick={() => void handleRefreshFeishuSetup()}
                disabled={retryingFeishuConnector}
                className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
              >
                刷新授权状态
              </button>
              {pendingFeishuPairingRequest ? (
                <>
                  <button
                    type="button"
                    onClick={() => void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "approve")}
                    disabled={feishuPairingActionLoading !== null}
                    className="h-8 px-3 rounded bg-amber-600 text-xs text-white hover:bg-amber-700 disabled:bg-amber-300"
                  >
                    {feishuPairingActionLoading === "approve" ? "批准中..." : "批准这次接入"}
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "deny")}
                    disabled={feishuPairingActionLoading !== null}
                    className="h-8 px-3 rounded border border-red-200 bg-white text-xs text-red-700 hover:bg-red-50 disabled:bg-gray-100"
                  >
                    {feishuPairingActionLoading === "deny" ? "拒绝中..." : "拒绝这次接入"}
                  </button>
                </>
              ) : null}
              <button
                type="button"
                onClick={() => void handleStartFeishuInstaller("create")}
                disabled={feishuInstallerBusy}
                className="h-8 px-3 rounded border border-indigo-200 bg-white text-xs text-indigo-700 hover:bg-indigo-50 disabled:bg-gray-100"
                >
                  {feishuInstallerBusy && feishuInstallerStartingMode === "create" ? "启动中..." : "新建机器人向导（高级）"}
              </button>
            </div>
            {feishuAuthorizationInlineError && feishuOnboardingHeaderStep !== "authorize" ? (
              <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700">
                {feishuAuthorizationInlineError}
              </div>
            ) : null}
            <details
              className="rounded-lg border border-gray-100 bg-gray-50 p-3"
              open={feishuInstallerSession.running || feishuInstallerSession.recent_output.length > 0}
            >
              <summary className="cursor-pointer text-xs font-medium text-gray-700">查看安装向导输出</summary>
              <div className="mt-3 space-y-3">
                <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
                  <div className="rounded border border-gray-200 bg-white px-3 py-2">
                    <div className="text-[11px] text-gray-500">向导状态</div>
                    <div className="text-sm font-medium text-gray-900">
                      {feishuInstallerBusy && feishuInstallerStartingMode ? "正在启动" : feishuInstallerSession.running ? "运行中" : "未运行"}
                    </div>
                  </div>
                  <div className="rounded border border-gray-200 bg-white px-3 py-2">
                    <div className="text-[11px] text-gray-500">当前模式</div>
                    <div className="text-sm font-medium text-gray-900">
                      {feishuInstallerDisplayMode === "create"
                        ? "新建机器人"
                        : feishuInstallerDisplayMode === "link"
                          ? "绑定已有机器人"
                          : "未启动"}
                    </div>
                  </div>
                  <div className="rounded border border-gray-200 bg-white px-3 py-2">
                    <div className="text-[11px] text-gray-500">提示</div>
                    <div className="text-sm font-medium text-gray-900">
                      {feishuInstallerStartupHint || feishuInstallerSession.prompt_hint || "暂无"}
                    </div>
                  </div>
                </div>
                <div className="rounded-lg border border-gray-900 bg-[#050816] px-3 py-3 text-xs text-gray-100">
                  <pre className="max-h-72 overflow-auto whitespace-pre-wrap break-all font-mono">
                    {feishuInstallerSession.recent_output.length > 0
                      ? feishuInstallerSession.recent_output.join("\n")
                      : feishuInstallerStartupHint || "暂无安装向导输出"}
                  </pre>
                </div>
                <div className="flex flex-col gap-2 md:flex-row">
                  <input
                    value={feishuInstallerInput}
                    onChange={(event) => setFeishuInstallerInput(event.target.value)}
                    placeholder="需要时手动输入，例如 App ID、App Secret 或回车"
                    className="flex-1 rounded border border-gray-200 bg-white px-3 py-2 text-xs text-gray-900"
                  />
                  <button
                    type="button"
                    onClick={() => void handleSendFeishuInstallerInput()}
                    disabled={feishuInstallerBusy || !feishuInstallerInput.trim()}
                    className="h-9 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
                  >
                    发送输入
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleStopFeishuInstallerSession()}
                    disabled={feishuInstallerBusy || !feishuInstallerSession.running}
                    className="h-9 px-3 rounded border border-red-200 bg-white text-xs text-red-700 hover:bg-red-50 disabled:bg-gray-100"
                  >
                    停止向导
                  </button>
                </div>
                <div className="text-[11px] text-gray-500">
                  如果你的电脑已安装 OpenClaw，当前向导也会优先命中 WorkClaw 内置的受控 openclaw shim，不会写到外部 OpenClaw 配置。
                </div>
              </div>
            </details>
          </div>

          <div className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
            <div>
              <div className="text-sm font-medium text-gray-900">接待设置</div>
              <div className="text-xs text-gray-500 mt-1">飞书接通后，还需要指定默认接待员工或配置群聊范围，消息才会稳定落到正确员工。</div>
            </div>
            <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-3">
              <div className="text-sm font-medium text-blue-950">{feishuRoutingStatus.label}</div>
              <div className="mt-1 text-xs text-blue-900">{feishuRoutingStatus.description}</div>
            </div>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">授权状态</div>
                <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.auth_status === "approved" ? "已完成" : "待完成"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">默认接待员工</div>
                <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.default_routing_employee_name || "未设置"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">群聊范围规则</div>
                <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.scoped_routing_count ?? 0} 条</div>
              </div>
            </div>
            <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-2 text-xs text-blue-800">
              接待员工的具体配置入口在员工详情页。完成当前接入后，请关闭设置窗口并前往员工详情中的“飞书接待”继续配置。
            </div>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => onOpenEmployees?.()}
                className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50"
              >
                {feishuRoutingStatus.actionLabel}
              </button>
            </div>
          </div>

          <details className="rounded-lg border border-gray-200 bg-white p-4">
            <summary className="cursor-pointer text-sm font-medium text-gray-900">连接详情</summary>
            <div className="mt-2 text-xs text-gray-500">这里展示当前连接是否正常、最近一次事件，以及排查问题时最有用的诊断摘要。</div>
            <div className="mt-3 rounded-lg border border-blue-100 bg-blue-50 px-3 py-3 text-sm text-blue-900">
              {getFeishuConnectionDetailSummary()}
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => void handleRefreshFeishuSetup()}
                disabled={retryingFeishuConnector}
                className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
              >
                {retryingFeishuConnector ? "检测中..." : "重新检测"}
              </button>
              <button
                type="button"
                onClick={() => void handleCopyFeishuDiagnostics()}
                className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50"
              >
                复制诊断摘要
              </button>
            </div>
            <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">当前状态</div>
                <div className="text-sm font-medium text-gray-900">{resolveFeishuConnectorStatus().label}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">插件版本</div>
                <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.plugin_version || primaryPluginChannelHost?.version || "未识别"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">当前接入账号</div>
                <div className="text-sm font-medium text-gray-900">{primaryPluginChannelSnapshot?.snapshot.defaultAccountId || "未识别"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">待完成授权</div>
                <div className="text-sm font-medium text-gray-900">{pendingFeishuPairingCount}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2 md:col-span-2">
                <div className="text-[11px] text-gray-500">最近一次事件</div>
                <div className="text-sm font-medium text-gray-900">{formatCompactDateTime(officialFeishuRuntimeStatus?.last_event_at)}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2 md:col-span-2">
                <div className="text-[11px] text-gray-500">最近问题</div>
                <div className="text-sm font-medium text-gray-900">{summarizeConnectorIssue(resolveFeishuConnectorStatus().error)}</div>
              </div>
            </div>
            <details className="mt-3 rounded-lg border border-gray-100 bg-gray-50 p-3">
              <summary className="cursor-pointer text-xs font-medium text-gray-700">原始日志（最近 3 条）</summary>
              <div className="mt-2 text-xs text-gray-700 whitespace-pre-wrap break-all">
                {summarizeOfficialFeishuRuntimeLogs(officialFeishuRuntimeStatus)}
              </div>
            </details>
          </details>

          <details className="rounded-lg border border-gray-200 bg-white p-4">
            <summary className="cursor-pointer text-sm font-medium text-gray-900">高级设置</summary>
            <div className="mt-2 text-xs text-gray-500">这里可以调整消息格式、接待规则和其他进阶选项。默认设置通常已经够用。</div>
            <div className="mt-3 rounded-lg border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-800">
              建议先完成接入和接待配置，再按需调整这里的参数；不确定时保持默认值通常更稳妥。
            </div>
            <div className="mt-4 space-y-4">
              <details open className="rounded-lg border border-gray-100 bg-gray-50/70 p-3">
                <summary className="cursor-pointer text-sm font-medium text-gray-900">消息与展示</summary>
                <div className="mt-2 text-xs text-gray-500">调整消息输出格式、分块策略和 Markdown 展示方式。</div>
                <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
                  {FEISHU_ADVANCED_MESSAGE_FIELDS.map(renderFeishuAdvancedField)}
                </div>
              </details>

              <details className="rounded-lg border border-gray-100 bg-gray-50/70 p-3">
                <summary className="cursor-pointer text-sm font-medium text-gray-900">群聊与私聊规则</summary>
                <div className="mt-2 text-xs text-gray-500">按群聊或私聊对象自定义启用状态、会话范围和回复规则。</div>
                <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
                  {FEISHU_ADVANCED_ROUTING_FIELDS.map(renderFeishuAdvancedField)}
                </div>
              </details>

              <details className="rounded-lg border border-gray-100 bg-gray-50/70 p-3">
                <summary className="cursor-pointer text-sm font-medium text-gray-900">运行与行为</summary>
                <div className="mt-2 text-xs text-gray-500">调整心跳、媒体限制、超时和插件运行行为。</div>
                <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
                  {FEISHU_ADVANCED_RUNTIME_FIELDS.map(renderFeishuAdvancedField)}
                </div>
                <details className="mt-4 rounded-lg border border-gray-200 bg-white p-3">
                  <summary className="cursor-pointer text-sm font-medium text-gray-800">动态 Agent 相关</summary>
                  <div className="mt-2 text-xs text-gray-500">只有在需要按飞书会话动态生成 Agent 时才需要调整这里。</div>
                  <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
                    {FEISHU_ADVANCED_DYNAMIC_AGENT_FIELDS.map(renderFeishuAdvancedField)}
                  </div>
                </details>
              </details>

              <div className="flex justify-end">
                <button
                  type="button"
                  onClick={() => void handleSaveFeishuAdvancedSettings()}
                  disabled={savingFeishuAdvancedSettings}
                  className="sm-btn sm-btn-primary h-9 rounded-lg px-4 text-sm disabled:opacity-60"
                >
                  {savingFeishuAdvancedSettings ? "保存中..." : "保存高级配置"}
                </button>
              </div>
            </div>
          </details>
            </div>
          </details>
        </div>
      )}


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

    </SettingsShell>
  );
}
