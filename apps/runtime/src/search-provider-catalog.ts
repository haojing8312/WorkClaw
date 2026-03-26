/**
 * 搜索引擎提供商目录
 * 包含支持的搜索引擎列表及其配置信息
 */

export interface SearchProviderCatalogItem {
  id: string;
  label: string;
  name: string;
  apiFormat: string;
  baseUrl: string;
  badge?: string;
  helper?: string;
  officialConsoleUrl?: string;
  officialDocsUrl?: string;
}

function createSearchProvider(config: SearchProviderCatalogItem): SearchProviderCatalogItem {
  return config;
}

export const SEARCH_PROVIDER_CATALOG: SearchProviderCatalogItem[] = [
  createSearchProvider({
    id: "metaso",
    label: "秘塔搜索 (中文首选)",
    name: "秘塔搜索",
    apiFormat: "search_metaso",
    baseUrl: "https://metaso.cn",
    badge: "中文首选",
    helper: "国内搜索效果好，无广告，适合中文用户。",
    officialConsoleUrl: "https://metaso.cn/",
    officialDocsUrl: "https://metaso.cn/",
  }),
  createSearchProvider({
    id: "brave",
    label: "Brave Search (国际首选)",
    name: "Brave Search",
    apiFormat: "search_brave",
    baseUrl: "https://api.search.brave.com",
    badge: "国际首选",
    helper: "适合国际搜索场景，隐私保护好，结果质量高。",
    officialConsoleUrl: "https://brave.com/search/api/",
    officialDocsUrl: "https://brave.com/search/api/",
  }),
  createSearchProvider({
    id: "tavily",
    label: "Tavily (AI 专用)",
    name: "Tavily",
    apiFormat: "search_tavily",
    baseUrl: "https://api.tavily.com",
    badge: "AI 专用",
    helper: "专为 AI 设计的搜索引擎，适合 AI Agent 获取实时信息。",
    officialConsoleUrl: "https://tavily.com/",
    officialDocsUrl: "https://docs.tavily.com/",
  }),
  createSearchProvider({
    id: "bocha",
    label: "博查搜索 (中文 AI)",
    name: "博查搜索",
    apiFormat: "search_bocha",
    baseUrl: "https://api.bochaai.com",
    badge: "中文 AI",
    helper: "国内 AI 搜索平台，支持多模态搜索。",
    officialConsoleUrl: "https://bochaai.com/",
    officialDocsUrl: "https://bochaai.com/",
  }),
  createSearchProvider({
    id: "serpapi",
    label: "SerpAPI (多引擎)",
    name: "SerpAPI",
    apiFormat: "search_serpapi",
    baseUrl: "https://serpapi.com",
    badge: "多引擎",
    helper: "支持 Google、Bing、百度等多个搜索引擎。",
    officialConsoleUrl: "https://serpapi.com/",
    officialDocsUrl: "https://serpapi.com/",
  }),
];

export function getSearchProviderCatalogItem(id: string): SearchProviderCatalogItem | undefined {
  return SEARCH_PROVIDER_CATALOG.find((item) => item.id === id);
}

export function buildSearchFormFromCatalogItem(item: SearchProviderCatalogItem): {
  name: string;
  api_format: string;
  base_url: string;
  model_name: string;
  api_key: string;
} {
  return {
    name: item.name,
    api_format: item.apiFormat,
    base_url: item.baseUrl,
    model_name: item.apiFormat === "search_serpapi" ? "google" : "",
    api_key: "",
  };
}
