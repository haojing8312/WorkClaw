export type ModelProviderApiFormat = "openai" | "anthropic";

export interface ModelProviderCatalogItem {
  id: string;
  label: string;
  name: string;
  providerKey: string;
  apiFormat: ModelProviderApiFormat;
  protocolLabel: string;
  baseUrl: string;
  baseUrlPlaceholder: string;
  defaultModel: string;
  modelNamePlaceholder: string;
  models: string[];
  badge: string;
  helper: string;
  officialConsoleUrl?: string;
  officialDocsUrl?: string;
  officialConsoleLabel?: string;
  officialDocsLabel?: string;
  supportsCustomBaseUrl: boolean;
  supportsCustomModelName: boolean;
  isCustom: boolean;
  customGuidanceTitle?: string;
  customGuidanceLines?: string[];
}

export interface ModelProviderConfigLookup {
  api_format: string;
  base_url: string;
}

export interface ModelProviderFormDefaults {
  name: string;
  api_format: ModelProviderApiFormat;
  base_url: string;
  model_name: string;
}

function createOfficialProvider(
  item: Omit<
    ModelProviderCatalogItem,
    | "isCustom"
    | "supportsCustomBaseUrl"
    | "supportsCustomModelName"
    | "baseUrlPlaceholder"
    | "modelNamePlaceholder"
    | "officialConsoleLabel"
    | "officialDocsLabel"
  >,
): ModelProviderCatalogItem {
  return {
    ...item,
    baseUrlPlaceholder: item.baseUrl,
    modelNamePlaceholder: item.defaultModel,
    officialConsoleLabel: "获取 API Key",
    officialDocsLabel: "查看文档",
    supportsCustomBaseUrl: false,
    supportsCustomModelName: false,
    isCustom: false,
  };
}

function createCustomProvider(
  item: Omit<
    ModelProviderCatalogItem,
    | "isCustom"
    | "supportsCustomBaseUrl"
    | "supportsCustomModelName"
    | "officialConsoleLabel"
    | "officialDocsLabel"
  >,
): ModelProviderCatalogItem {
  return {
    ...item,
    officialConsoleLabel: undefined,
    officialDocsLabel: undefined,
    supportsCustomBaseUrl: true,
    supportsCustomModelName: true,
    isCustom: true,
  };
}

export const MODEL_PROVIDER_CATALOG: ModelProviderCatalogItem[] = [
  createOfficialProvider({
    id: "zhipu",
    label: "智谱 GLM",
    name: "智谱 GLM",
    providerKey: "zhipu",
    apiFormat: "openai",
    protocolLabel: "OpenAI 兼容",
    baseUrl: "https://open.bigmodel.cn/api/paas/v4",
    defaultModel: "glm-5-turbo",
    models: ["glm-5-turbo", "glm-5", "glm-4-plus", "glm-4-air", "glm-4-long", "glm-4-flash"],
    badge: "国内直连",
    helper: "适合国内环境快速接入，官方平台可直接创建项目密钥。",
    officialConsoleUrl: "https://open.bigmodel.cn/usercenter/proj-mgmt/apikeys",
    officialDocsUrl: "https://open.bigmodel.cn/dev/api",
  }),
  createOfficialProvider({
    id: "doubao",
    label: "豆包 / 火山方舟",
    name: "豆包 / 火山方舟",
    providerKey: "doubao",
    apiFormat: "openai",
    protocolLabel: "OpenAI 兼容",
    baseUrl: "https://ark.cn-beijing.volces.com/api/v3",
    defaultModel: "doubao-seed-1.6",
    models: ["doubao-seed-1.6"],
    badge: "国内直连",
    helper: "默认使用豆包综合能力最强的通用模型；部分火山方舟账号需改成你自己的推理接入点 ID。",
    officialConsoleUrl: "https://console.volcengine.com/ark",
    officialDocsUrl: "https://www.volcengine.com/docs/82379",
  }),
  createOfficialProvider({
    id: "openai",
    label: "OpenAI",
    name: "OpenAI",
    providerKey: "openai",
    apiFormat: "openai",
    protocolLabel: "OpenAI 兼容",
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "gpt-5.4",
    models: ["gpt-5.4", "gpt-4o", "gpt-4o-mini", "gpt-4.1", "gpt-4.1-mini", "gpt-4.1-nano", "o3-mini"],
    badge: "通用兼容",
    helper: "生态成熟，适合大多数通用任务和工具调用场景。",
    officialConsoleUrl: "https://platform.openai.com/api-keys",
    officialDocsUrl: "https://platform.openai.com/docs/quickstart/authentication",
  }),
  createOfficialProvider({
    id: "anthropic",
    label: "Claude (Anthropic)",
    name: "Claude",
    providerKey: "anthropic",
    apiFormat: "anthropic",
    protocolLabel: "Claude (Anthropic)",
    baseUrl: "https://api.anthropic.com/v1",
    defaultModel: "claude-sonnet-4-5-20250929",
    models: [
      "claude-sonnet-4-5-20250929",
      "claude-3-5-sonnet-20241022",
      "claude-3-5-haiku-20241022",
    ],
    badge: "长文推理",
    helper: "适合复杂写作、分析和长上下文协作任务。",
    officialConsoleUrl: "https://console.anthropic.com/settings/keys",
    officialDocsUrl: "https://docs.anthropic.com/en/api/getting-started",
  }),
  createOfficialProvider({
    id: "minimax-openai",
    label: "MiniMax (OpenAI 兼容)",
    name: "MiniMax",
    providerKey: "minimax",
    apiFormat: "openai",
    protocolLabel: "OpenAI 兼容",
    baseUrl: "https://api.minimaxi.com/v1",
    defaultModel: "MiniMax-M2.5",
    models: ["MiniMax-M2.5", "MiniMax-M2.1", "MiniMax-M2"],
    badge: "官方双协议",
    helper: "MiniMax 官方支持 OpenAI 和 Anthropic 两种兼容接入方式。",
    officialConsoleUrl: "https://platform.minimaxi.com/",
    officialDocsUrl: "https://platform.minimaxi.com/docs/api-reference/api-overview",
  }),
  createOfficialProvider({
    id: "minimax-anthropic",
    label: "MiniMax (Claude 兼容)",
    name: "MiniMax",
    providerKey: "minimax",
    apiFormat: "anthropic",
    protocolLabel: "Claude (Anthropic)",
    baseUrl: "https://api.minimaxi.com/anthropic",
    defaultModel: "MiniMax-M2.5",
    models: ["MiniMax-M2.5", "MiniMax-M2.1", "MiniMax-M2"],
    badge: "官方双协议",
    helper: "适合希望复用 Anthropic SDK 或 Claude Code 风格接入的场景。",
    officialConsoleUrl: "https://platform.minimaxi.com/",
    officialDocsUrl: "https://platform.minimaxi.com/docs/api-reference/anthropic-api-compatible-cache",
  }),
  createOfficialProvider({
    id: "deepseek",
    label: "DeepSeek",
    name: "DeepSeek",
    providerKey: "deepseek",
    apiFormat: "openai",
    protocolLabel: "OpenAI 兼容",
    baseUrl: "https://api.deepseek.com/v1",
    defaultModel: "deepseek-chat",
    models: ["deepseek-chat", "deepseek-reasoner"],
    badge: "性价比",
    helper: "适合高频日常协作与批量任务场景。",
    officialConsoleUrl: "https://platform.deepseek.com/api-keys",
    officialDocsUrl: "https://api-docs.deepseek.com/",
  }),
  createOfficialProvider({
    id: "qwen-intl",
    label: "Qwen（国际）",
    name: "Qwen（国际）",
    providerKey: "qwen",
    apiFormat: "openai",
    protocolLabel: "OpenAI 兼容",
    baseUrl: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
    defaultModel: "qwen3.5-plus",
    models: [
      "qwen3.5-plus",
      "qwen3-max",
      "qwen-max",
      "qwen-plus",
      "qwen-turbo",
      "qwen-long",
      "qwen-vl-max",
      "qwen-vl-plus",
    ],
    badge: "阿里云国际",
    helper: "适合国际版 DashScope 接入，需使用对应地域 API Key。",
    officialConsoleUrl: "https://help.aliyun.com/zh/model-studio/get-api-key",
    officialDocsUrl: "https://help.aliyun.com/zh/model-studio/user-guide/first-api-call-to-qwen",
  }),
  createOfficialProvider({
    id: "qwen-cn",
    label: "Qwen（国内）",
    name: "Qwen（国内）",
    providerKey: "qwen",
    apiFormat: "openai",
    protocolLabel: "OpenAI 兼容",
    baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    defaultModel: "qwen3.5-plus",
    models: [
      "qwen3.5-plus",
      "qwen3-max",
      "qwen-max",
      "qwen-plus",
      "qwen-turbo",
      "qwen-long",
      "qwen-vl-max",
      "qwen-vl-plus",
    ],
    badge: "阿里云百炼",
    helper: "适合国内百炼接入，默认走北京地域 OpenAI 兼容端点。",
    officialConsoleUrl: "https://help.aliyun.com/zh/model-studio/get-api-key",
    officialDocsUrl: "https://help.aliyun.com/zh/model-studio/user-guide/first-api-call-to-qwen",
  }),
  createOfficialProvider({
    id: "moonshot",
    label: "Moonshot / Kimi",
    name: "Moonshot / Kimi",
    providerKey: "moonshot",
    apiFormat: "openai",
    protocolLabel: "OpenAI 兼容",
    baseUrl: "https://api.moonshot.ai/v1",
    defaultModel: "kimi-k2-0905-preview",
    models: ["kimi-k2-0905-preview", "kimi-k2", "kimi-latest", "moonshot-v1-8k", "moonshot-v1-32k", "moonshot-v1-128k"],
    badge: "长上下文",
    helper: "适合长文本处理与 Kimi 系列模型接入。",
    officialConsoleUrl: "https://platform.moonshot.cn/",
    officialDocsUrl: "https://platform.moonshot.cn/docs",
  }),
  createOfficialProvider({
    id: "yi",
    label: "Yi",
    name: "Yi",
    providerKey: "yi",
    apiFormat: "openai",
    protocolLabel: "OpenAI 兼容",
    baseUrl: "https://api.lingyiwanwu.com/v1",
    defaultModel: "yi-lightning",
    models: ["yi-lightning", "yi-large", "yi-medium"],
    badge: "国产模型",
    helper: "零一万物官方开放平台，适合 Yi 系列模型直连。",
    officialConsoleUrl: "https://platform.01.ai/",
    officialDocsUrl: "https://platform.01.ai/docs",
  }),
  createCustomProvider({
    id: "custom-openai",
    label: "自定义 OpenAI 兼容",
    name: "自定义 OpenAI 兼容",
    providerKey: "openai",
    apiFormat: "openai",
    protocolLabel: "OpenAI 兼容",
    baseUrl: "",
    baseUrlPlaceholder: "https://your-gateway.example.com/v1",
    defaultModel: "gpt-5.4",
    modelNamePlaceholder: "输入你的模型名，例如 qwen3.5-plus 或 deepseek-chat",
    models: [],
    badge: "第三方中转",
    helper: "适用于 OpenRouter、One API、New API、企业私有网关等 OpenAI 兼容服务。",
    customGuidanceTitle: "自定义 OpenAI 兼容",
    customGuidanceLines: [
      "请向你的中转或代理服务商申请 API Key。",
      "需要确认它提供的 Base URL、模型名，以及是否兼容 OpenAI Chat Completions。",
      "通常应填写带 /v1 的接口根地址。",
    ],
  }),
  createCustomProvider({
    id: "custom-anthropic",
    label: "自定义 Claude (Anthropic)",
    name: "自定义 Claude (Anthropic)",
    providerKey: "anthropic",
    apiFormat: "anthropic",
    protocolLabel: "Claude (Anthropic)",
    baseUrl: "",
    baseUrlPlaceholder: "https://your-gateway.example.com/v1",
    defaultModel: "claude-sonnet-4-5-20250929",
    modelNamePlaceholder: "输入你的 Claude 兼容模型名",
    models: [],
    badge: "第三方中转",
    helper: "适用于 Anthropic Messages API 兼容网关或第三方 Claude 代理服务。",
    customGuidanceTitle: "自定义 Claude (Anthropic)",
    customGuidanceLines: [
      "请向你的中转或代理服务商申请 API Key。",
      "需要确认它兼容 Anthropic Messages API，并提供可用的 Base URL 与模型名。",
      "如果服务商仅支持 OpenAI 兼容协议，请改用“自定义 OpenAI 兼容”。",
    ],
  }),
];

export const DEFAULT_MODEL_PROVIDER_ID = "zhipu";

function normalizeBaseUrl(value: string): string {
  return value.trim().replace(/\/+$/, "").toLowerCase();
}

export function getModelProviderCatalogItem(id: string): ModelProviderCatalogItem {
  return (
    MODEL_PROVIDER_CATALOG.find((item) => item.id === id) ??
    MODEL_PROVIDER_CATALOG.find((item) => item.id === DEFAULT_MODEL_PROVIDER_ID) ??
    MODEL_PROVIDER_CATALOG[0]
  );
}

export function buildModelFormFromCatalogItem(
  item: ModelProviderCatalogItem,
): ModelProviderFormDefaults {
  return {
    name: item.name,
    api_format: item.apiFormat,
    base_url: item.baseUrl,
    model_name: item.defaultModel,
  };
}

export function resolveCatalogItemForConfig(
  config: ModelProviderConfigLookup,
): ModelProviderCatalogItem {
  const normalizedBaseUrl = normalizeBaseUrl(config.base_url);
  const officialMatch = MODEL_PROVIDER_CATALOG.find(
    (item) =>
      !item.isCustom &&
      item.apiFormat === config.api_format &&
      normalizeBaseUrl(item.baseUrl) === normalizedBaseUrl,
  );

  if (officialMatch) {
    return officialMatch;
  }

  if (config.api_format === "anthropic") {
    return getModelProviderCatalogItem("custom-anthropic");
  }

  return getModelProviderCatalogItem("custom-openai");
}

export function resolveCatalogItemForProviderIdentity(input: {
  providerKey?: string;
  apiFormat: string;
  baseUrl: string;
}): ModelProviderCatalogItem {
  const exactMatch = resolveCatalogItemForConfig({
    api_format: input.apiFormat,
    base_url: input.baseUrl,
  });

  if (!exactMatch.isCustom) {
    return exactMatch;
  }

  const normalizedProviderKey = (input.providerKey || "").trim().toLowerCase();
  if (normalizedProviderKey) {
    const providerMatches = MODEL_PROVIDER_CATALOG.filter(
      (item) =>
        !item.isCustom &&
        item.providerKey === normalizedProviderKey &&
        item.apiFormat === input.apiFormat,
    );

    if (providerMatches.length === 1) {
      return providerMatches[0];
    }
  }

  return exactMatch;
}
