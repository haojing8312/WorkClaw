import { act, renderHook, waitFor } from "@testing-library/react";
import { useSettingsController } from "../useSettingsController";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function installInvokeMock() {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === "list_model_configs") {
      return Promise.resolve([
        {
          id: "model-1",
          name: "Model One",
          api_format: "openai",
          base_url: "https://example.com",
          model_name: "model-one",
          is_default: false,
        },
      ]);
    }
    if (command === "list_provider_configs") {
      return Promise.resolve([
        {
          id: "provider-1",
          provider_key: "openai",
          display_name: "OpenAI",
          protocol_type: "openai",
          base_url: "https://example.com",
          auth_type: "api_key",
          api_key_encrypted: "",
          org_id: "",
          extra_json: "{}",
          enabled: true,
        },
      ]);
    }
    if (command === "get_capability_routing_policy") {
      return Promise.resolve({
        capability: "vision",
        primary_provider_id: "provider-1",
        primary_model: "vision-model",
        fallback_chain_json: JSON.stringify([{ provider_id: "provider-1", model: "fallback-model" }]),
        timeout_ms: 90000,
        retry_count: 1,
        enabled: true,
      });
    }
    if (command === "list_provider_models") {
      return Promise.resolve(["vision-model", "fallback-model"]);
    }
    return Promise.resolve(null);
  });
}

describe("useSettingsController", () => {
  beforeEach(() => {
    installInvokeMock();
  });

  test("loads models and providers on mount", async () => {
    const { result } = renderHook(() => useSettingsController());

    await waitFor(() => {
      expect(result.current.models).toHaveLength(1);
      expect(result.current.providers).toHaveLength(1);
    });

    expect(invokeMock).toHaveBeenCalledWith("list_model_configs");
    expect(invokeMock).toHaveBeenCalledWith("list_provider_configs");
  });

  test("loads a capability routing policy and its fallback rows", async () => {
    const { result } = renderHook(() => useSettingsController());

    await act(async () => {
      await result.current.loadCapabilityRoutingPolicy("vision");
    });

    await waitFor(() => {
      expect(result.current.chatRoutingPolicy.capability).toBe("vision");
      expect(result.current.chatRoutingPolicy.primary_provider_id).toBe("provider-1");
      expect(result.current.chatFallbackRows).toEqual([
        {
          provider_id: "provider-1",
          model: "fallback-model",
        },
      ]);
      expect(result.current.chatPrimaryModels).toEqual(["vision-model", "fallback-model"]);
    });

    expect(invokeMock).toHaveBeenCalledWith("get_capability_routing_policy", {
      capability: "vision",
    });
    expect(invokeMock).toHaveBeenCalledWith("list_provider_models", {
      providerId: "provider-1",
      capability: "vision",
    });
  });

  test("loadChatPrimaryModels returns the available models for the selected provider and capability", async () => {
    const { result } = renderHook(() => useSettingsController());

    let models: string[] = [];
    await act(async () => {
      models = await result.current.loadChatPrimaryModels("provider-1", "vision");
    });

    expect(models).toEqual(["vision-model", "fallback-model"]);
    expect(result.current.chatPrimaryModels).toEqual(["vision-model", "fallback-model"]);
  });
});
