import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { SettingsView } from "../SettingsView";
import { MODEL_PROVIDER_CATALOG } from "../../model-provider-catalog";

const invokeMock = vi.fn();

let mockModels: Array<{
  id: string;
  name: string;
  api_format: string;
  base_url: string;
  model_name: string;
  is_default: boolean;
}> = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView model providers", () => {
  beforeEach(() => {
    mockModels = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "list_model_configs") {
        return Promise.resolve(mockModels);
      }
      if (command === "list_search_configs") {
        return Promise.resolve([]);
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "",
          default_language: "zh-CN",
          immersive_translation_enabled: true,
          immersive_translation_display: "translated_only",
          immersive_translation_trigger: "auto",
          translation_engine: "model_then_free",
          translation_model_id: "",
        });
      }
      if (command === "list_mcp_servers" || command === "list_provider_configs") {
        return Promise.resolve([]);
      }
      if (command === "get_model_api_key") {
        return Promise.resolve("sk-existing");
      }
      if (command === "save_provider_config") {
        return Promise.resolve(null);
      }
      if (command === "save_model_config") {
        return Promise.resolve(null);
      }
      if (command === "test_connection_cmd") {
        return Promise.resolve(true);
      }
      return Promise.resolve(null);
    });
  });

  test("shows the full shared provider list and provider-specific guidance", async () => {
    render(<SettingsView onClose={() => {}} />);

    const providerSelect = await screen.findByTestId("settings-model-provider-preset");
    const options = within(providerSelect).getAllByRole("option");
    expect(options).toHaveLength(MODEL_PROVIDER_CATALOG.length);

    for (const provider of MODEL_PROVIDER_CATALOG) {
      expect(within(providerSelect).getByRole("option", { name: provider.label })).toBeInTheDocument();
    }

    expect(screen.getByRole("link", { name: "获取 API Key" })).toBeInTheDocument();

    fireEvent.change(providerSelect, {
      target: { value: "custom-openai" },
    });

    expect(screen.queryByRole("link", { name: "获取 API Key" })).not.toBeInTheDocument();
    expect(screen.getByTestId("settings-model-provider-custom-guidance")).toHaveTextContent(
      "请向你的中转或代理服务商申请 API Key。",
    );
  });

  test("switches to custom anthropic provider and saves anthropic config", async () => {
    render(<SettingsView onClose={() => {}} />);

    const providerSelect = await screen.findByTestId("settings-model-provider-preset");
    fireEvent.change(providerSelect, {
      target: { value: "custom-anthropic" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-name"), {
      target: { value: "Claude Proxy" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-base-url"), {
      target: { value: "https://claude-proxy.example.com/v1" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-model-name"), {
      target: { value: "claude-3-5-sonnet-20241022" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-api-key"), {
      target: { value: "sk-ant-proxy-123" },
    });

    fireEvent.click(screen.getByTestId("settings-model-provider-save"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_model_config",
        expect.objectContaining({
          config: expect.objectContaining({
            name: "Claude Proxy",
            api_format: "anthropic",
            base_url: "https://claude-proxy.example.com/v1",
            model_name: "claude-3-5-sonnet-20241022",
          }),
          apiKey: "sk-ant-proxy-123",
        }),
      );
    });
  });

  test("maps edited custom anthropic configs back to the custom provider", async () => {
    mockModels = [
      {
        id: "model-custom-ant",
        name: "Claude Proxy",
        api_format: "anthropic",
        base_url: "https://claude-proxy.example.com/v1",
        model_name: "claude-3-5-sonnet-20241022",
        is_default: true,
      },
    ];

    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(await screen.findByRole("button", { name: "编辑" }));

    await waitFor(() => {
      expect(screen.getByTestId("settings-model-provider-preset")).toHaveValue("custom-anthropic");
    });
    expect(screen.getByTestId("settings-model-provider-base-url")).toHaveValue(
      "https://claude-proxy.example.com/v1",
    );
    expect(screen.getByTestId("settings-model-provider-model-name")).toHaveValue(
      "claude-3-5-sonnet-20241022",
    );
    expect(screen.getByTestId("settings-model-provider-custom-guidance")).toBeInTheDocument();
  });
});
