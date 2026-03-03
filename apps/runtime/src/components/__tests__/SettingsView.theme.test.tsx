import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView semantic theme", () => {
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
      return Promise.resolve(null);
    });
  });

  test("uses semantic classes and keeps regular-user tabs minimal", async () => {
    render(<SettingsView onClose={() => {}} />);

    const modelsTab = screen.getByRole("button", { name: "模型连接" });
    expect(modelsTab).toHaveClass("sm-btn");
    expect(screen.queryByRole("button", { name: "Providers" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "能力路由" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "健康检查" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "MCP 服务器" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "自动路由" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "飞书协作" })).not.toBeInTheDocument();

    const searchTab = screen.getByRole("button", { name: "搜索引擎" });
    expect(searchTab).toHaveClass("sm-btn");
    fireEvent.click(searchTab);
    await waitFor(() => {
      expect(screen.getByText("快速选择搜索引擎")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "MCP 服务器" }));
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "brave-search" } });
    expect(screen.getByPlaceholderText("请输入 BRAVE_API_KEY")).toBeInTheDocument();
  });
});
