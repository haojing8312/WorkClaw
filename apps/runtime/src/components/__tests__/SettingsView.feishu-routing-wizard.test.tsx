import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView feishu routing wizard", () => {
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
          auto_update_enabled: true,
          update_channel: "stable",
          dismissed_update_version: "",
          last_update_check_at: "",
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
      if (command === "list_im_routing_bindings") return Promise.resolve([]);
      if (command === "upsert_im_routing_binding") return Promise.resolve("rule-1");
      if (command === "simulate_im_route") {
        return Promise.resolve({ agentId: "main", matchedBy: "default" });
      }
      return Promise.resolve(null);
    });
  });

  test("saves routing rule from wizard and can run simulation", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));

    await waitFor(() => {
      expect(screen.getByText("渠道连接器路由向导（当前：飞书）")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText("agent_id（如 main）"), {
      target: { value: "peer-agent" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存规则" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "upsert_im_routing_binding",
        expect.objectContaining({
          input: expect.objectContaining({ agent_id: "peer-agent" }),
        }),
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "模拟路由" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "simulate_im_route",
        expect.objectContaining({
          payload: expect.objectContaining({ channel: "feishu" }),
        }),
      );
    });
  });
});
