import App from "../App";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

const invokeMock = vi.fn();
const minimizeMock = vi.fn().mockResolvedValue(undefined);
const isMaximizedMock = vi.fn().mockResolvedValue(false);
const maximizeMock = vi.fn().mockResolvedValue(undefined);
const unmaximizeMock = vi.fn().mockResolvedValue(undefined);
const closeMock = vi.fn().mockResolvedValue(undefined);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    minimize: minimizeMock,
    isMaximized: isMaximizedMock,
    maximize: maximizeMock,
    unmaximize: unmaximizeMock,
    close: closeMock,
  }),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../components/Sidebar", () => ({
  Sidebar: () => <div data-testid="sidebar">sidebar</div>,
}));

vi.mock("../components/ChatView", () => ({
  ChatView: () => <div data-testid="chat-view">chat-view</div>,
}));

vi.mock("../components/NewSessionLanding", () => ({
  NewSessionLanding: () => <div data-testid="new-session-landing">new-session-landing</div>,
}));

vi.mock("../components/SettingsView", () => ({
  SettingsView: () => <div data-testid="settings-view">settings-view</div>,
}));

vi.mock("../components/InstallDialog", () => ({
  InstallDialog: () => <div data-testid="install-dialog">install-dialog</div>,
}));

vi.mock("../components/packaging/PackagingView", () => ({
  PackagingView: () => <div data-testid="packaging-view">packaging-view</div>,
}));

vi.mock("../components/experts/ExpertsView", () => ({
  ExpertsView: () => <div data-testid="experts-view">experts-view</div>,
}));

vi.mock("../components/experts/ExpertCreateView", () => ({
  ExpertCreateView: () => <div data-testid="experts-new-view">experts-new-view</div>,
}));

vi.mock("../components/employees/EmployeeHubView", () => ({
  EmployeeHubView: () => <div data-testid="employees-view">employees-view</div>,
}));

describe("App desktop titlebar", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    minimizeMock.mockClear();
    isMaximizedMock.mockClear();
    maximizeMock.mockClear();
    unmaximizeMock.mockClear();
    closeMock.mockClear();
    isMaximizedMock.mockResolvedValue(false);
    window.localStorage.setItem("workclaw:initial-model-setup-completed", "1");
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_skills") {
        return Promise.resolve([
          {
            id: "builtin-general",
            name: "General",
            description: "desc",
            version: "1.0.0",
            author: "test",
            recommended_model: "model-a",
            tags: [],
            created_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_model_configs") {
        return Promise.resolve([
          {
            id: "model-a",
            name: "Model A",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model-a",
            is_default: true,
          },
        ]);
      }
      if (command === "list_search_configs") {
        return Promise.resolve([
          {
            id: "search-a",
            name: "Search A",
            api_format: "openai",
            base_url: "https://search.example.com",
            model_name: "search-model",
            is_default: true,
          },
        ]);
      }
      if (command === "list_sessions") {
        return Promise.resolve([]);
      }
      if (command === "list_agent_employees" || command === "list_employee_groups") {
        return Promise.resolve([]);
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "standard",
        });
      }
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    window.localStorage.clear();
  });

  test("renders a custom titlebar and routes window controls through tauri window actions", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("app-titlebar")).toBeInTheDocument();
    });

    expect(screen.getByText("WorkClaw")).toBeInTheDocument();
    expect(screen.getByTestId("app-titlebar").querySelector(".h-1\\.5")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "最小化窗口" }));
    fireEvent.click(screen.getByRole("button", { name: "最大化窗口" }));
    fireEvent.click(screen.getByRole("button", { name: "关闭窗口" }));

    await waitFor(() => {
      expect(minimizeMock).toHaveBeenCalledTimes(1);
      expect(isMaximizedMock).toHaveBeenCalled();
      expect(maximizeMock).toHaveBeenCalledTimes(1);
      expect(unmaximizeMock).toHaveBeenCalledTimes(0);
      expect(closeMock).toHaveBeenCalledTimes(1);
    });
  });
});
