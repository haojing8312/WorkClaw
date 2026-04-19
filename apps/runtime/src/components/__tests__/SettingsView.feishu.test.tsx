import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve(null)),
}));

describe("SettingsView connector tab", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string, payload?: Record<string, unknown>) => {
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
      if (command === "list_im_channel_registry") {
        return Promise.resolve([
          {
            channel: "feishu",
            display_name: "Feishu",
            host_kind: "openclaw_plugin",
            status: "stopped",
            summary: "飞书渠道由 OpenClaw 官方插件宿主提供，WorkClaw 只负责路由、会话与回复生命周期。",
            detail: "插件版本 2026.3.17 · 账号 default · 运行时未启动",
            capabilities: ["media", "reactions", "threads", "outbound", "pairing"],
            instance_id: "default",
            last_error: null,
            plugin_host: null,
            runtime_status: null,
            diagnostics: null,
            monitor_status: null,
            connector_settings: null,
            automation_status: {
              channel: "feishu",
              host_kind: "openclaw_plugin",
              should_restore: false,
              restored: false,
              monitor_restored: false,
              detail: "Feishu runtime did not meet auto-restore conditions",
              error: null,
            },
            recent_action: null,
          },
          {
            channel: "wecom",
            display_name: "企业微信连接器",
            host_kind: "connector",
            status: "not_configured",
            summary: "企业微信渠道将复用与 OpenClaw 兼容的 connector host 形态接入。",
            detail: "未配置凭据",
            capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
            instance_id: null,
            last_error: null,
            plugin_host: null,
            runtime_status: null,
            diagnostics: null,
            monitor_status: null,
            connector_settings: {
              corp_id: "",
              agent_id: "",
              agent_secret: "",
              sidecar_base_url: "",
            },
            automation_status: null,
            recent_action: null,
          },
        ]);
      }
      if (command === "set_im_channel_host_running") {
        return Promise.resolve({
          channel: payload?.channel || "feishu",
          display_name: "Feishu",
          host_kind: "openclaw_plugin",
          status: payload?.desiredRunning ? "running" : "stopped",
          summary: "飞书渠道由 OpenClaw 官方插件宿主提供，WorkClaw 只负责路由、会话与回复生命周期。",
          detail: payload?.desiredRunning ? "插件版本 2026.3.17 · 账号 default · 运行时已启动" : "插件版本 2026.3.17 · 账号 default · 运行时未启动",
          capabilities: ["media", "reactions", "threads", "outbound", "pairing"],
          instance_id: "default",
          last_error: null,
          plugin_host: null,
          runtime_status: null,
          diagnostics: null,
          monitor_status: null,
          connector_settings: null,
          automation_status: {
            channel: payload?.channel || "feishu",
            host_kind: "openclaw_plugin",
            should_restore: false,
            restored: Boolean(payload?.desiredRunning),
            monitor_restored: false,
            detail: "host state updated for feishu",
            error: null,
          },
          recent_action: {
            channel: payload?.channel || "feishu",
            action: payload?.desiredRunning ? "set_running" : "set_stopped",
            desired_running: Boolean(payload?.desiredRunning),
            ok: true,
            detail: "host state updated for feishu",
            error: null,
            source: "settings-ui",
            occurred_at: "2026-04-14T09:00:00Z",
          },
        });
      }

      return Promise.resolve(null);
    });
  });

  test("shows connector overview copy but keeps routing data lazy-loaded", async () => {
    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "渠道连接器" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));
    await waitFor(() => {
      expect(screen.getByTestId("connector-panel-feishu")).toBeInTheDocument();
    });
    expect(screen.getByText("飞书连接")).toBeInTheDocument();
    expect(screen.getByText("渠道宿主总览")).toBeInTheDocument();
    expect(screen.getByText("这里展示 WorkClaw 当前接入的 IM 渠道宿主形态。飞书走 OpenClaw 官方插件宿主，企业微信走 connector 宿主。")).toBeInTheDocument();
    expect(screen.getAllByText("飞书接入概览").length).toBeGreaterThan(0);
    expect(screen.getByText("员工关联入口")).toBeInTheDocument();
    expect(screen.getByText("查看飞书连接是否已启动并可接收事件。")).toBeInTheDocument();
    expect(screen.getByTestId("connector-panel-wecom")).toBeInTheDocument();
    expect(screen.getAllByText("企业微信连接器").length).toBeGreaterThan(0);
    expect(screen.getByText("飞书高级配置")).toBeInTheDocument();
    expect(screen.getByText("飞书宿主详情")).toBeInTheDocument();
    expect(screen.getByText("企业微信宿主详情")).toBeInTheDocument();
    expect(screen.getByText("最近回复：暂无记录")).toBeInTheDocument();
    expect(
      screen.getAllByText("下一步建议：先点击“启动宿主”，再观察是否收到新消息；如需确认接待范围，可查看“员工关联入口”。").length,
    ).toBeGreaterThanOrEqual(2);

    expect(invokeMock.mock.calls.some(([command]) => command === "list_im_routing_bindings")).toBe(false);
    expect(invokeMock.mock.calls.some(([command]) => command === "get_feishu_long_connection_status")).toBe(false);
  });

  test("uses the unified channel host command to start feishu from host details", async () => {
    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByText("飞书宿主详情")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("飞书宿主详情"));
    const panel = await screen.findByTestId("feishu-host-details-panel");
    expect(within(panel).getByText("最近自动恢复")).toBeInTheDocument();
    expect(within(panel).getByText("最近回复状态")).toBeInTheDocument();
    expect(within(panel).getByText("暂无回复记录")).toBeInTheDocument();
    expect(within(panel).getByText("未执行 · Feishu runtime did not meet auto-restore conditions")).toBeInTheDocument();
    fireEvent.click(within(panel).getByRole("button", { name: "启动宿主" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_im_channel_host_running", {
        channel: "feishu",
        desiredRunning: true,
      });
    });
  });

  test("opens employees from reply guidance shortcuts", async () => {
    const onOpenEmployees = vi.fn();

    render(<SettingsView onClose={() => {}} initialTab="feishu" onOpenEmployees={onOpenEmployees} />);

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: "去员工关联入口" }).length).toBeGreaterThanOrEqual(2);
    });

    fireEvent.click(screen.getAllByRole("button", { name: "去员工关联入口" })[0]);
    expect(onOpenEmployees).toHaveBeenCalledTimes(1);
  });

  test("shows projected latest reply completion in feishu host details", async () => {
    invokeMock.mockImplementation((command: string, payload?: Record<string, unknown>) => {
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
      if (command === "list_im_channel_registry") {
        return Promise.resolve([
          {
            channel: "feishu",
            display_name: "Feishu",
            host_kind: "openclaw_plugin",
            status: "running",
            summary: "通过 OpenClaw 官方飞书插件接收与回复消息。",
            detail: "插件版本 2026.3.17 · 账号 default · 运行时已启动",
            capabilities: ["media", "reactions", "threads", "outbound", "pairing"],
            instance_id: "default",
            last_error: null,
            plugin_host: {
              plugin_id: "openclaw-lark",
              npm_spec: "@openclaw/feishu-plugin",
              package_name: "@openclaw/feishu-plugin",
              version: "2026.3.17",
              display_name: "Feishu",
              channel: "feishu",
              status: "ready",
              capabilities: ["media", "reactions", "threads", "outbound", "pairing"],
              install_id: null,
              error: null,
            },
            runtime_status: {
              plugin_id: "openclaw-lark",
              account_id: "default",
              running: true,
              started_at: "2026-04-19T10:00:00Z",
              last_stop_at: null,
              last_event_at: "2026-04-19T10:02:00Z",
              last_error: null,
              pid: 1234,
              port: null,
              recent_logs: ["[reply_trace] id=reply-42 state=Completed"],
              recent_reply_lifecycle: [],
              latest_reply_completion: {
                logicalReplyId: "reply-42",
                phase: "dispatch_idle",
                state: "completed",
                updatedAt: "2026-04-19T10:02:00Z",
              },
            },
            diagnostics: null,
            monitor_status: null,
            connector_settings: null,
            automation_status: null,
            recent_action: null,
          },
          {
            channel: "wecom",
            display_name: "企业微信连接器",
            host_kind: "connector",
            status: "not_configured",
            summary: "企业微信渠道将复用与 OpenClaw 兼容的 connector host 形态接入。",
            detail: "未配置凭据",
            capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
            instance_id: null,
            last_error: null,
            plugin_host: null,
            runtime_status: null,
            diagnostics: null,
            monitor_status: null,
            connector_settings: {
              corp_id: "",
              agent_id: "",
              agent_secret: "",
              sidecar_base_url: "",
            },
            automation_status: null,
            recent_action: null,
          },
        ]);
      }
      if (command === "set_im_channel_host_running") {
        return Promise.resolve({
          channel: payload?.channel || "feishu",
          display_name: "Feishu",
          host_kind: "openclaw_plugin",
          status: payload?.desiredRunning ? "running" : "stopped",
          summary: "通过 OpenClaw 官方飞书插件接收与回复消息。",
          detail: payload?.desiredRunning ? "插件版本 2026.3.17 · 账号 default · 运行时已启动" : "插件版本 2026.3.17 · 账号 default · 运行时未启动",
          capabilities: ["media", "reactions", "threads", "outbound", "pairing"],
          instance_id: "default",
          last_error: null,
          plugin_host: null,
          runtime_status: null,
          diagnostics: null,
          monitor_status: null,
          connector_settings: null,
          automation_status: null,
          recent_action: null,
        });
      }

      return Promise.resolve(null);
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByText("飞书宿主详情")).toBeInTheDocument();
    });

    expect(screen.getByText("最近回复：已完成 · reply-42")).toBeInTheDocument();
    expect(
      screen.getAllByText("下一步建议：这条回复已结束；如需确认最新状态，可点击“刷新宿主状态”，如需调整后续接待可查看“员工关联入口”。").length,
    ).toBeGreaterThanOrEqual(2);
    fireEvent.click(screen.getByText("飞书宿主详情"));
    const panel = await screen.findByTestId("feishu-host-details-panel");
    expect(within(panel).getByText("最近回复状态")).toBeInTheDocument();
    expect(within(panel).getByText("已完成")).toBeInTheDocument();
    expect(within(panel).getByText("reply=reply-42 · phase=dispatch_idle")).toBeInTheDocument();
    expect(within(panel).getAllByText("2026-04-19T10:02:00Z").length).toBeGreaterThanOrEqual(2);
    expect(within(panel).getByText("下一步建议：这条回复已结束；如需确认最新状态，可点击“刷新宿主状态”，如需调整后续接待可查看“员工关联入口”。")).toBeInTheDocument();
  });

  test("shows resumed reply completion as resumed running guidance in feishu host details", async () => {
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
      if (command === "list_im_channel_registry") {
        return Promise.resolve([
          {
            channel: "feishu",
            display_name: "Feishu",
            host_kind: "openclaw_plugin",
            status: "running",
            summary: "通过 OpenClaw 官方飞书插件接收与回复消息。",
            detail: "插件版本 2026.3.17 · 账号 default · 运行时已启动",
            capabilities: ["media", "reactions", "threads", "outbound", "pairing"],
            instance_id: "default",
            last_error: null,
            plugin_host: null,
            runtime_status: {
              plugin_id: "openclaw-lark",
              account_id: "default",
              running: true,
              started_at: "2026-04-19T10:00:00Z",
              last_stop_at: null,
              last_event_at: "2026-04-19T10:03:00Z",
              last_error: null,
              pid: 1234,
              port: null,
              recent_logs: ["[reply_lifecycle] resumed"],
              recent_reply_lifecycle: [],
              latest_reply_completion: {
                logicalReplyId: "reply-resume-1",
                phase: "resumed",
                state: "running",
                updatedAt: "2026-04-19T10:03:00Z",
              },
            },
            diagnostics: null,
            monitor_status: null,
            connector_settings: null,
            automation_status: null,
            recent_action: null,
          },
          {
            channel: "wecom",
            display_name: "企业微信连接器",
            host_kind: "connector",
            status: "not_configured",
            summary: "企业微信渠道将复用与 OpenClaw 兼容的 connector host 形态接入。",
            detail: "未配置凭据",
            capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
            instance_id: null,
            last_error: null,
            plugin_host: null,
            runtime_status: null,
            diagnostics: null,
            monitor_status: null,
            connector_settings: {
              corp_id: "",
              agent_id: "",
              agent_secret: "",
              sidecar_base_url: "",
            },
            automation_status: null,
            recent_action: null,
          },
        ]);
      }
      return Promise.resolve(null);
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByText("飞书宿主详情")).toBeInTheDocument();
    });

    expect(screen.getByText("最近回复：已恢复处理中 · reply-resume-1")).toBeInTheDocument();
    expect(
      screen.getAllByText("下一步建议：宿主已收到继续执行所需的输入或审批结果，当前正在恢复推进这条回复；可先点击“刷新宿主状态”并观察飞书线程是否继续更新。").length,
    ).toBeGreaterThanOrEqual(2);

    fireEvent.click(screen.getByText("飞书宿主详情"));
    const panel = await screen.findByTestId("feishu-host-details-panel");
    expect(within(panel).getByText("已恢复处理中")).toBeInTheDocument();
    expect(within(panel).getByText("reply=reply-resume-1 · phase=resumed")).toBeInTheDocument();
    expect(
      within(panel).getByText("下一步建议：宿主已收到继续执行所需的输入或审批结果，当前正在恢复推进这条回复；可先点击“刷新宿主状态”并观察飞书线程是否继续更新。"),
    ).toBeInTheDocument();
  });

  test("points failed reply guidance to feishu advanced settings when runtime delivery fails", async () => {
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
      if (command === "list_im_channel_registry") {
        return Promise.resolve([
          {
            channel: "feishu",
            display_name: "Feishu",
            host_kind: "openclaw_plugin",
            status: "degraded",
            summary: "通过 OpenClaw 官方飞书插件接收与回复消息。",
            detail: "插件版本 2026.3.17 · 账号 default · 运行时已启动",
            capabilities: ["media", "reactions", "threads", "outbound", "pairing"],
            instance_id: "default",
            last_error: "dispatch failed",
            plugin_host: null,
            runtime_status: {
              plugin_id: "openclaw-lark",
              account_id: "default",
              running: true,
              started_at: "2026-04-19T10:00:00Z",
              last_stop_at: null,
              last_event_at: "2026-04-19T10:05:00Z",
              last_error: "dispatch failed",
              pid: 1234,
              port: null,
              recent_logs: ["[reply_trace] error=dispatch failed"],
              recent_reply_lifecycle: [],
              latest_reply_completion: {
                logicalReplyId: "reply-failed",
                phase: "failed",
                state: "failed",
                updatedAt: "2026-04-19T10:05:00Z",
              },
            },
            diagnostics: null,
            monitor_status: null,
            connector_settings: null,
            automation_status: null,
            recent_action: null,
          },
          {
            channel: "wecom",
            display_name: "企业微信连接器",
            host_kind: "connector",
            status: "not_configured",
            summary: "企业微信渠道将复用与 OpenClaw 兼容的 connector host 形态接入。",
            detail: "未配置凭据",
            capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
            instance_id: null,
            last_error: null,
            plugin_host: null,
            runtime_status: null,
            diagnostics: null,
            monitor_status: null,
            connector_settings: {
              corp_id: "",
              agent_id: "",
              agent_secret: "",
              sidecar_base_url: "",
            },
            automation_status: null,
            recent_action: null,
          },
        ]);
      }
      if (command === "set_im_channel_host_running") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByText("飞书宿主详情")).toBeInTheDocument();
    });

    expect(screen.getByText("最近回复：失败 · reply-failed")).toBeInTheDocument();
    expect(
      screen.getAllByText("下一步建议：先查看“最近问题”和“宿主日志”，必要时点击“刷新宿主状态”；如果持续失败，可回到“飞书高级配置”检查连接。").length,
    ).toBeGreaterThanOrEqual(2);
    fireEvent.click(screen.getAllByRole("button", { name: "打开飞书高级配置" })[0]);
    await waitFor(() => {
      expect(screen.getByTestId("feishu-advanced-settings-form")).toHaveAttribute("open");
    });
  });
});
