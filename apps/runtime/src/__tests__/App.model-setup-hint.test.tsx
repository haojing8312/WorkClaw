import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../App";

const invokeMock = vi.fn();
const MODEL_SETUP_HINT_DISMISSED_KEY = "workclaw:model-setup-hint-dismissed";
const INITIAL_MODEL_SETUP_COMPLETED_KEY = "workclaw:initial-model-setup-completed";
let mockModels: Array<{
  id: string;
  name: string;
  api_format: string;
  base_url: string;
  model_name: string;
  is_default: boolean;
}> = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../components/Sidebar", () => ({
  Sidebar: (props: any) => (
    <div>
      <button onClick={props.onOpenStartTask}>open-start-task</button>
    </div>
  ),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: () => <div data-testid="chat-view">chat-view</div>,
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

vi.mock("../components/SettingsView", () => ({
  SettingsView: (props: any) => (
    <div data-testid="settings-view">
      settings-view
      <button onClick={props.onClose}>close-settings</button>
    </div>
  ),
}));

vi.mock("../components/InstallDialog", () => ({
  InstallDialog: () => <div data-testid="install-dialog">install-dialog</div>,
}));

vi.mock("../components/NewSessionLanding", () => ({
  NewSessionLanding: () => <div data-testid="new-session-landing">new-session-landing</div>,
}));

describe("App model setup hint", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    window.localStorage.clear();
    mockModels = [];

    invokeMock.mockImplementation((command: string) => {
      if (command === "list_skills") {
        return Promise.resolve([
          {
            id: "builtin-general",
            name: "General",
            description: "desc",
            version: "1.0.0",
            author: "test",
            recommended_model: "",
            tags: [],
            created_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_model_configs") {
        return Promise.resolve(mockModels);
      }
      if (command === "get_sessions") {
        return Promise.resolve([]);
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([]);
      }
      if (command === "save_model_config") {
        mockModels = [
          {
            id: "model-quick",
            name: "Quick Setup",
            api_format: "openai",
            base_url: "https://api.openai.com/v1",
            model_name: "gpt-4o-mini",
            is_default: true,
          },
        ];
        return Promise.resolve(null);
      }
      if (command === "test_connection_cmd") {
        return Promise.resolve(true);
      }
      return Promise.resolve(null);
    });
  });

  test("shows dismissible hint and opens settings from hint action", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-open-settings"));

    await waitFor(() => {
      expect(screen.getByTestId("settings-view")).toBeInTheDocument();
    });
  });

  test("remembers dismissal across reload when still no model config", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    const firstRender = render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-dismiss"));

    await waitFor(() => {
      expect(screen.queryByTestId("model-setup-hint")).not.toBeInTheDocument();
    });
    expect(window.localStorage.getItem(MODEL_SETUP_HINT_DISMISSED_KEY)).toBe("1");

    firstRender.unmount();
    render(<App />);

    await waitFor(() => {
      expect(screen.queryByTestId("model-setup-hint")).not.toBeInTheDocument();
    });
  });

  test("clears dismissal marker once any model is configured", async () => {
    window.localStorage.setItem(MODEL_SETUP_HINT_DISMISSED_KEY, "1");
    mockModels = [
      {
        id: "model-a",
        name: "Model A",
        api_format: "openai",
        base_url: "https://example.com",
        model_name: "gpt-4o-mini",
        is_default: true,
      },
    ];

    render(<App />);

    await waitFor(() => {
      expect(window.localStorage.getItem(MODEL_SETUP_HINT_DISMISSED_KEY)).toBeNull();
    });
    expect(screen.queryByTestId("model-setup-hint")).not.toBeInTheDocument();
  });

  test("supports quick setup from hint and persists model config", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByTestId("quick-model-setup-api-key"), {
      target: { value: "sk-test-quick-123" },
    });
    fireEvent.click(screen.getByTestId("quick-model-setup-save"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_model_config",
        expect.objectContaining({
          config: expect.objectContaining({
            api_format: "openai",
            base_url: "https://api.openai.com/v1",
            model_name: "gpt-4o-mini",
          }),
          apiKey: "sk-test-quick-123",
        }),
      );
    });

    await waitFor(() => {
      expect(screen.queryByTestId("quick-model-setup-dialog")).not.toBeInTheDocument();
      expect(screen.queryByTestId("model-setup-hint")).not.toBeInTheDocument();
    });
  });

  test("supports testing quick setup connection before save", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByTestId("quick-model-setup-api-key"), {
      target: { value: "sk-test-quick-connection" },
    });
    fireEvent.click(screen.getByTestId("quick-model-setup-test-connection"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "test_connection_cmd",
        expect.objectContaining({
          config: expect.objectContaining({
            api_format: "openai",
            base_url: "https://api.openai.com/v1",
            model_name: "gpt-4o-mini",
          }),
          apiKey: "sk-test-quick-connection",
        }),
      );
    });

    expect(screen.getByTestId("quick-model-setup-test-result")).toHaveTextContent(
      "连接成功，可直接保存并开始",
    );
  });

  test("enforces first-launch model setup gate until at least one model is configured", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-gate")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("model-setup-hint")).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("model-setup-gate-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("quick-model-setup-cancel"));
    expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();

    fireEvent.change(screen.getByTestId("quick-model-setup-api-key"), {
      target: { value: "sk-test-first-launch-gate" },
    });
    fireEvent.click(screen.getByTestId("quick-model-setup-save"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_model_config",
        expect.objectContaining({
          apiKey: "sk-test-first-launch-gate",
        }),
      );
    });

    await waitFor(() => {
      expect(screen.queryByTestId("model-setup-gate")).not.toBeInTheDocument();
      expect(screen.queryByTestId("quick-model-setup-dialog")).not.toBeInTheDocument();
    });
  });

  test("shows non-blocking hint instead of gate after first model setup has been completed once", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("model-setup-gate")).not.toBeInTheDocument();
  });
});
