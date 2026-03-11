import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView connector routing wizard", () => {
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
      if (command === "get_feishu_gateway_settings") {
        return Promise.resolve({
          app_id: "",
          app_secret: "",
          ingress_token: "",
          encrypt_key: "",
          sidecar_base_url: "",
        });
      }
      if (command === "get_wecom_gateway_settings") {
        return Promise.resolve({
          corp_id: "",
          agent_id: "",
          agent_secret: "",
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
      if (command === "get_wecom_connector_status") {
        return Promise.resolve({
          running: false,
          started_at: null,
          last_error: null,
          reconnect_attempts: 0,
          queue_depth: 0,
          instance_id: "wecom:wecom-main",
        });
      }
      if (command === "list_im_routing_bindings") {
        return Promise.resolve([
          {
            id: "binding-wecom-1",
            agent_id: "wecom-agent",
            channel: "wecom",
            account_id: "corp-123",
            peer_kind: "group",
            peer_id: "wecom-room-1",
            guild_id: "",
            team_id: "",
            role_ids: [],
            connector_meta: { connector_id: "wecom-main" },
            priority: 90,
            enabled: true,
            created_at: "2026-03-11T00:00:00Z",
            updated_at: "2026-03-11T00:00:00Z",
          },
        ]);
      }
      if (command === "upsert_im_routing_binding") return Promise.resolve("rule-1");
      if (command === "simulate_im_route") {
        return Promise.resolve({ agentId: "main", matchedBy: "binding.channel" });
      }
      return Promise.resolve(null);
    });
  });

  test("saves connector routing rule and can run simulation", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));

    await waitFor(() => {
      expect(screen.getByLabelText("路由渠道")).toBeInTheDocument();
    });
    expect(screen.getAllByText("消息处理规则").length).toBeGreaterThan(0);
    expect(screen.getByText("设置不同渠道的消息应该交给谁处理。")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("路由渠道"), {
      target: { value: "feishu" },
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
    expect(screen.getAllByText("已启用规则数").length).toBeGreaterThan(0);
    expect(screen.getAllByText("1").length).toBeGreaterThan(0);

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

  test("requires choosing a connector channel before saving", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));

    await waitFor(() => {
      expect(screen.getByLabelText("路由渠道")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText("agent_id（如 main）"), {
      target: { value: "peer-agent" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存规则" }));

    expect(screen.getByText("请先选择路由渠道")).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith(
      "upsert_im_routing_binding",
      expect.anything(),
    );
  });

  test("supports wecom rules through the same connector routing wizard", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));

    await waitFor(() => {
      expect(screen.getByLabelText("路由渠道")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText("路由渠道"), {
      target: { value: "wecom" },
    });
    fireEvent.change(screen.getByPlaceholderText("agent_id（如 main）"), {
      target: { value: "wecom-agent" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存规则" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "upsert_im_routing_binding",
        expect.objectContaining({
          input: expect.objectContaining({
            agent_id: "wecom-agent",
            channel: "wecom",
            connector_meta: expect.objectContaining({ connector_id: "wecom" }),
          }),
        }),
      );
    });
    expect(screen.getAllByText("已启用规则数").length).toBeGreaterThan(0);
    expect(screen.getAllByText("1").length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: "模拟路由" }));

    await waitFor(() => {
      expect(screen.getByText("将由：main")).toBeInTheDocument();
      expect(screen.getByText("命中原因：渠道规则")).toBeInTheDocument();
      expect(screen.getByText("规则来源：企业微信")).toBeInTheDocument();
      expect(invokeMock).toHaveBeenCalledWith(
        "simulate_im_route",
        expect.objectContaining({
          payload: expect.objectContaining({ channel: "wecom" }),
        }),
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "查看技术详情" }));
    expect(screen.getByText("matchedBy: binding.channel")).toBeInTheDocument();
    expect(screen.getByText("channel: wecom")).toBeInTheDocument();
  });
});
