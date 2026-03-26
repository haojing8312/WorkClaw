export interface SearchConfigFormState {
  name: string;
  api_format: string;
  base_url: string;
  model_name: string;
  api_key: string;
}

export const EMPTY_SEARCH_CONFIG_FORM: SearchConfigFormState = {
  name: "",
  api_format: "",
  base_url: "",
  model_name: "",
  api_key: "",
};

export const SEARCH_PRESETS = [
  { label: "— 快速选择 —", value: "", api_format: "", base_url: "", model_name: "" },
  { label: "秘塔搜索 (中文首选)", value: "metaso", api_format: "search_metaso", base_url: "https://metaso.cn", model_name: "" },
  { label: "Brave Search (国际首选)", value: "brave", api_format: "search_brave", base_url: "https://api.search.brave.com", model_name: "" },
  { label: "Tavily (AI 专用)", value: "tavily", api_format: "search_tavily", base_url: "https://api.tavily.com", model_name: "" },
  { label: "博查搜索 (中文 AI)", value: "bocha", api_format: "search_bocha", base_url: "https://api.bochaai.com", model_name: "" },
  { label: "SerpAPI (多引擎)", value: "serpapi", api_format: "search_serpapi", base_url: "https://serpapi.com", model_name: "google" },
] as const;

export function applySearchPresetToForm(
  value: string,
  previous: SearchConfigFormState,
): SearchConfigFormState {
  const preset = SEARCH_PRESETS.find((item) => item.value === value);
  if (!preset || !preset.value) {
    return previous;
  }

  return {
    ...previous,
    name: preset.label.replace(/ \(.*\)/, ""),
    api_format: preset.api_format,
    base_url: preset.base_url,
    model_name: preset.model_name,
  };
}

export function validateSearchConfigForm(form: SearchConfigFormState): string | null {
  if (!form.name.trim()) {
    return "请输入名称";
  }
  if (!form.api_format.trim()) {
    return "请选择搜索引擎模板";
  }
  if (!form.api_key.trim()) {
    return "请输入 API Key";
  }
  return null;
}
