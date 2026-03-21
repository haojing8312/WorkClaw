import { describe, expect, test } from "vitest";
import {
  DEFAULT_MODEL_PROVIDER_ID,
  MODEL_PROVIDER_CATALOG,
  buildModelFormFromCatalogItem,
  resolveCatalogItemForConfig,
  resolveCatalogItemForProviderIdentity,
} from "../model-provider-catalog";

describe("model provider catalog", () => {
  test("contains the full set of official and custom providers", () => {
    expect(MODEL_PROVIDER_CATALOG.map((item) => item.id)).toEqual([
      "zhipu",
      "doubao",
      "openai",
      "anthropic",
      "minimax-openai",
      "minimax-anthropic",
      "deepseek",
      "qwen-intl",
      "qwen-cn",
      "moonshot",
      "yi",
      "custom-openai",
      "custom-anthropic",
    ]);
  });

  test("uses zhipu as the default provider for first-run setup", () => {
    expect(DEFAULT_MODEL_PROVIDER_ID).toBe("zhipu");

    const item = MODEL_PROVIDER_CATALOG.find((entry) => entry.id === DEFAULT_MODEL_PROVIDER_ID);
    expect(item?.officialConsoleUrl).toBeTruthy();
  });

  test("builds model form defaults from a catalog item", () => {
    const item = MODEL_PROVIDER_CATALOG.find((entry) => entry.id === "deepseek");
    expect(item).toBeDefined();

    expect(buildModelFormFromCatalogItem(item!)).toEqual({
      name: "DeepSeek",
      api_format: "openai",
      base_url: "https://api.deepseek.com/v1",
      model_name: "deepseek-chat",
    });
  });

  test("prefers the current agent-oriented default models for official presets", () => {
    const byId = (id: string) => MODEL_PROVIDER_CATALOG.find((entry) => entry.id === id);

    expect(byId("zhipu")?.defaultModel).toBe("glm-5-turbo");
    expect(byId("doubao")?.defaultModel).toBe("doubao-seed-1.6");
    expect(byId("openai")?.defaultModel).toBe("gpt-5.4");
    expect(byId("anthropic")?.defaultModel).toBe("claude-sonnet-4-5-20250929");
    expect(byId("deepseek")?.defaultModel).toBe("deepseek-chat");
    expect(byId("qwen-intl")?.defaultModel).toBe("qwen3.5-plus");
    expect(byId("qwen-cn")?.defaultModel).toBe("qwen3.5-plus");
  });

  test("exposes the official doubao ark preset", () => {
    const item = MODEL_PROVIDER_CATALOG.find((entry) => entry.id === "doubao");
    expect(item).toBeDefined();
    expect(item).toMatchObject({
      providerKey: "doubao",
      apiFormat: "openai",
      baseUrl: "https://ark.cn-beijing.volces.com/api/v3",
      defaultModel: "doubao-seed-1.6",
      models: ["doubao-seed-1.6"],
    });
  });

  test("uses MiniMax domestic defaults for official presets", () => {
    const openaiItem = MODEL_PROVIDER_CATALOG.find((entry) => entry.id === "minimax-openai");
    const anthropicItem = MODEL_PROVIDER_CATALOG.find(
      (entry) => entry.id === "minimax-anthropic",
    );

    expect(openaiItem?.baseUrl).toBe("https://api.minimaxi.com/v1");
    expect(anthropicItem?.baseUrl).toBe("https://api.minimaxi.com/anthropic");
  });

  test("resolves official providers from api format and base url", () => {
    const item = resolveCatalogItemForConfig({
      api_format: "anthropic",
      base_url: "https://api.anthropic.com/v1",
    });

    expect(item.id).toBe("anthropic");
    expect(item.models).toContain("claude-sonnet-4-5-20250929");
  });

  test("falls back unknown openai configs to custom openai", () => {
    const item = resolveCatalogItemForConfig({
      api_format: "openai",
      base_url: "https://proxy.example.com/v1",
    });

    expect(item.id).toBe("custom-openai");
    expect(item.isCustom).toBe(true);
  });

  test("falls back unknown anthropic configs to custom anthropic", () => {
    const item = resolveCatalogItemForConfig({
      api_format: "anthropic",
      base_url: "https://claude-proxy.example.com/v1",
    });

    expect(item.id).toBe("custom-anthropic");
    expect(item.isCustom).toBe(true);
  });

  test("preserves unique official provider identity when base url is proxied", () => {
    const item = resolveCatalogItemForProviderIdentity({
      providerKey: "minimax",
      apiFormat: "openai",
      baseUrl: "http://111.51.78.135:8060/",
    });

    expect(item.id).toBe("minimax-openai");
    expect(item.providerKey).toBe("minimax");
  });
});
