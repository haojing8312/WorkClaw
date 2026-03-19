import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView connector management", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_model_configs") return Promise.resolve([]);
      if (command === "list_mcp_servers") return Promise.resolve([]);
      if (command === "list_search_configs") return Promise.resolve([]);
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "",
          default_language: "zh-CN",
          immersive_translation_enabled: true,
          immersive_translation_display: "translated_only",
          immersive_translation_trigger: "auto",
          translation_engine: "model_then_free",
          translation_model_id: "",
          launch_at_login: false,
          launch_minimized: false,
          close_to_tray: true,
        });
      }
      if (command === "get_desktop_lifecycle_paths") {
        return Promise.resolve({
          app_data_dir: "",
          cache_dir: "",
          log_dir: "",
          default_work_dir: "",
        });
      }
      if (command === "get_routing_settings") {
        return Promise.resolve({ max_call_depth: 4, node_timeout_seconds: 60, retry_count: 0 });
      }
      if (command === "list_builtin_provider_plugins") return Promise.resolve([]);
      if (command === "list_provider_configs") return Promise.resolve([]);
      if (command === "get_capability_routing_policy") return Promise.resolve(null);
      if (command === "list_capability_route_templates") return Promise.resolve([]);
      if (command === "get_feishu_gateway_settings") {
        return Promise.resolve({
          app_id: "",
          app_secret: "",
          ingress_token: "",
          encrypt_key: "",
          sidecar_base_url: "",
        });
      }
      if (command === "get_feishu_long_connection_status") {
        return Promise.resolve({
          running: false,
          started_at: null,
          queued_events: 0,
        });
      }
      return Promise.resolve(null);
    });
  });

  test("keeps routing assignment out of settings", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));

    await waitFor(() => {
      expect(screen.getByTestId("connector-panel-feishu")).toBeInTheDocument();
    });
    expect(screen.getByText("员工关联入口")).toBeInTheDocument();
    expect(screen.getByText("飞书连接成功后，请前往员工详情中的“飞书接待”配置默认接待员工或指定群聊范围。")).toBeInTheDocument();
    expect(screen.queryByTestId("connector-panel-wecom")).not.toBeInTheDocument();
    expect(screen.queryByText("企业微信")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("路由渠道")).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("upsert_im_routing_binding", expect.anything());
    expect(invokeMock).not.toHaveBeenCalledWith("simulate_im_route", expect.anything());
  });
});
