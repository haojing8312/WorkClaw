import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve(null)),
}));

describe("SettingsView risk flow", () => {
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
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "E:\\workspace\\workclaw",
          default_language: "zh-CN",
          immersive_translation_enabled: true,
          immersive_translation_display: "translated_only",
          immersive_translation_trigger: "auto",
          translation_engine: "model_then_free",
          translation_model_id: "",
          launch_at_login: false,
          launch_minimized: false,
          close_to_tray: true,
          operation_permission_mode: "standard",
        });
      }
      return Promise.resolve(null);
    });
  });

  test("does not expose employee deletion flow in settings", async () => {
    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "渠道连接器" })).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: "健康检查" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "自动路由" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "删除员工" })).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("delete_agent_employee", expect.anything());
  });

  test("desktop settings expose only standard and full access permission modes", async () => {
    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "桌面 / 系统" })).toBeInTheDocument();
    });

    const desktopButton = screen.getByRole("button", { name: "桌面 / 系统" });
    fireEvent.click(desktopButton);

    await waitFor(() => {
      expect(screen.getByText("操作权限")).toBeInTheDocument();
    });

    const permissionSection = screen.getByText("操作权限").closest("section") ?? document.body;
    expect(
      within(permissionSection).getByRole("radio", { name: "标准模式（推荐）" })
    ).toBeInTheDocument();
    expect(
      within(permissionSection).getByRole("radio", { name: "全自动模式" })
    ).toBeInTheDocument();
    expect(within(permissionSection).queryByRole("radio", { name: /推荐模式（常见改动自动处理）/ })).not.toBeInTheDocument();
    expect(within(permissionSection).queryByRole("radio", { name: /谨慎模式（关键操作先确认）/ })).not.toBeInTheDocument();
  });

  test("switching desktop permission mode to full access requires confirmation", async () => {
    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "桌面 / 系统" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "桌面 / 系统" }));

    await waitFor(() => {
      expect(screen.getByRole("radio", { name: "全自动模式" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("radio", { name: "全自动模式" }));

    await waitFor(() => {
      expect(screen.getByRole("dialog")).toBeInTheDocument();
    });
    expect(screen.getByText("切换为全自动模式")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "暂不切换" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "标准模式（推荐）" })).toBeChecked();

    fireEvent.click(screen.getByRole("radio", { name: "全自动模式" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "切换为全自动模式" })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "切换为全自动模式" }));

    await waitFor(() => {
      expect(screen.getByRole("radio", { name: "全自动模式" })).toBeChecked();
    });
  });
});
