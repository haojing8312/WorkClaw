import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve(null)),
}));

describe("SettingsView model connection feedback", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_model_configs") return Promise.resolve([]);
      if (command === "list_mcp_servers") return Promise.resolve([]);
      if (command === "list_search_configs") return Promise.resolve([]);
      if (command === "get_routing_settings") {
        return Promise.resolve({ max_call_depth: 4, node_timeout_seconds: 60, retry_count: 0 });
      }
      if (command === "list_builtin_provider_plugins") return Promise.resolve([]);
      if (command === "list_provider_configs") return Promise.resolve([]);
      if (command === "get_capability_routing_policy") return Promise.resolve(null);
      if (command === "list_capability_route_templates") return Promise.resolve([]);
      if (command === "test_connection_cmd") {
        return Promise.resolve({
          ok: false,
          kind: "auth",
          title: "鉴权失败",
          message: "请检查 API Key、组织权限或接口访问范围是否正确。",
          raw_message: "Unauthorized: invalid_api_key",
        });
      }
      return Promise.resolve(null);
    });
  });

  test("shows shared auth guidance for structured model connection failures", async () => {
    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "模型连接" })).toBeInTheDocument();
    });

    fireEvent.change(screen.getByTestId("settings-model-provider-api-key"), {
      target: { value: "sk-auth-test" },
    });
    fireEvent.click(screen.getByRole("button", { name: "测试连接" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "test_connection_cmd",
        expect.objectContaining({
          apiKey: "sk-auth-test",
        }),
      );
    });

    expect(screen.getByText("鉴权失败")).toBeInTheDocument();
    expect(screen.getByText("请检查 API Key、组织权限或接口访问范围是否正确。")).toBeInTheDocument();
    expect(screen.getByText("Unauthorized: invalid_api_key")).toBeInTheDocument();
  });
});
