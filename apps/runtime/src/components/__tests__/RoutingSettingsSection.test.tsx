import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { RoutingSettingsSection } from "../settings/routing/RoutingSettingsSection";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("RoutingSettingsSection", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_routing_settings") {
        return Promise.resolve({
          max_call_depth: 6,
          node_timeout_seconds: 90,
          retry_count: 1,
        });
      }
      return Promise.resolve(null);
    });
  });

  test("loads routing settings on mount", async () => {
    render(<RoutingSettingsSection />);

    await waitFor(() => {
      expect(screen.getByLabelText("最大调用深度 (2-8)")).toHaveValue(6);
    });
    expect(screen.getByLabelText("节点超时秒数 (5-600)")).toHaveValue(90);
    expect(screen.getByLabelText("失败重试次数 (0-2)")).toHaveValue(1);
  });

  test("clamps and saves routing settings", async () => {
    render(<RoutingSettingsSection />);

    await waitFor(() => {
      expect(screen.getByLabelText("最大调用深度 (2-8)")).toHaveValue(6);
    });

    fireEvent.change(screen.getByLabelText("最大调用深度 (2-8)"), { target: { value: "99" } });
    fireEvent.change(screen.getByLabelText("节点超时秒数 (5-600)"), { target: { value: "1" } });
    fireEvent.change(screen.getByLabelText("失败重试次数 (0-2)"), { target: { value: "-1" } });
    fireEvent.click(screen.getByRole("button", { name: "保存自动路由设置" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_routing_settings",
        expect.objectContaining({
          settings: {
            max_call_depth: 8,
            node_timeout_seconds: 5,
            retry_count: 0,
          },
        }),
      );
    });
    expect(screen.getByText("已保存")).toBeInTheDocument();
  });
});
