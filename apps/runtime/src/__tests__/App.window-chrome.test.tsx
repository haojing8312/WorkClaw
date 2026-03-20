import App from "../App";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

const invokeMock = vi.fn();
const minimizeMock = vi.fn().mockResolvedValue(undefined);
const isMaximizedMock = vi.fn().mockResolvedValue(false);
const maximizeMock = vi.fn().mockResolvedValue(undefined);
const unmaximizeMock = vi.fn().mockResolvedValue(undefined);
const closeMock = vi.fn().mockResolvedValue(undefined);
const outerSizeMock = vi.fn().mockResolvedValue({ width: 960, height: 720 });
const setPositionMock = vi.fn().mockResolvedValue(undefined);
const startDraggingMock = vi.fn().mockResolvedValue(undefined);
const onResizedMock = vi.fn();
let resizeHandler: ((event: { payload: { width: number; height: number } }) => void) | null = null;
const cursorPositionMock = vi.fn().mockResolvedValue({ x: 1200, y: 24 });

const mockDesktopWindow = {
  minimize: minimizeMock,
  isMaximized: isMaximizedMock,
  maximize: maximizeMock,
  unmaximize: unmaximizeMock,
  close: closeMock,
  outerSize: outerSizeMock,
  setPosition: setPositionMock,
  startDragging: startDraggingMock,
  onResized: onResizedMock,
};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => mockDesktopWindow,
  cursorPosition: () => cursorPositionMock(),
  PhysicalPosition: class PhysicalPosition {
    x: number;
    y: number;

    constructor(x: number, y: number) {
      this.x = x;
      this.y = y;
    }
  },
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
    outerSizeMock.mockClear();
    setPositionMock.mockClear();
    startDraggingMock.mockClear();
    cursorPositionMock.mockClear();
    onResizedMock.mockReset();
    resizeHandler = null;
    isMaximizedMock.mockResolvedValue(false);
    outerSizeMock.mockResolvedValue({ width: 960, height: 720 });
    cursorPositionMock.mockResolvedValue({ x: 1200, y: 24 });
    onResizedMock.mockImplementation((handler) => {
      resizeHandler = handler;
      return Promise.resolve(() => {
        resizeHandler = null;
      });
    });
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
    expect(screen.getByText("WorkClaw").closest('[data-testid="app-titlebar-drag-region"]')).toBeTruthy();

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

  test("switches maximize control text when the window enters or leaves maximized state", async () => {
    isMaximizedMock.mockResolvedValue(true);
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "还原窗口" })).toBeInTheDocument();
    });

    isMaximizedMock.mockResolvedValue(false);
    resizeHandler?.({ payload: { width: 1200, height: 750 } });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "最大化窗口" })).toBeInTheDocument();
    });
  });

  test("restores and continues dragging when the titlebar is dragged from maximized state", async () => {
    isMaximizedMock.mockResolvedValue(true);
    render(<App />);

    const dragRegion = await waitFor(() => screen.getByTestId("app-titlebar-drag-region"));

    Object.defineProperty(dragRegion, "getBoundingClientRect", {
      value: () => ({
        x: 0,
        y: 0,
        width: 1200,
        height: 44,
        top: 0,
        left: 0,
        right: 1200,
        bottom: 44,
        toJSON: () => ({}),
      }),
    });

    fireEvent.mouseDown(dragRegion, { button: 0, clientX: 300, clientY: 18 });

    await waitFor(() => {
      expect(unmaximizeMock).toHaveBeenCalledTimes(1);
      expect(cursorPositionMock).toHaveBeenCalledTimes(1);
      expect(outerSizeMock).toHaveBeenCalledTimes(1);
      expect(setPositionMock).toHaveBeenCalledTimes(1);
      expect(startDraggingMock).toHaveBeenCalledTimes(1);
    });
  });

  test("starts dragging directly when the titlebar is dragged in restored state", async () => {
    isMaximizedMock.mockResolvedValue(false);
    render(<App />);

    const dragRegion = await waitFor(() => screen.getByTestId("app-titlebar-drag-region"));
    fireEvent.mouseDown(dragRegion, { button: 0, clientX: 240, clientY: 18 });

    await waitFor(() => {
      expect(startDraggingMock).toHaveBeenCalledTimes(1);
    });

    expect(unmaximizeMock).toHaveBeenCalledTimes(0);
    expect(setPositionMock).toHaveBeenCalledTimes(0);
    expect(cursorPositionMock).toHaveBeenCalledTimes(0);
  });
});
