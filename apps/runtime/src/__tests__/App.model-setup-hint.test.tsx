import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../App";
import { MODEL_PROVIDER_CATALOG } from "../model-provider-catalog";

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
      <button onClick={props.onSettings}>open-settings</button>
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
      {props.showDevModelSetupTools ? (
        <>
          <button onClick={props.onDevResetFirstUseOnboarding}>reset-first-use-onboarding</button>
          <button onClick={props.onDevOpenQuickModelSetup}>open-dev-quick-setup</button>
        </>
      ) : null}
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

    invokeMock.mockImplementation((command: string, payload?: any) => {
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
        const savedConfig = payload?.config;
        mockModels = [
          {
            id: "model-quick",
            name: savedConfig?.name ?? "Quick Setup",
            api_format: savedConfig?.api_format ?? "openai",
            base_url: savedConfig?.base_url ?? "https://open.bigmodel.cn/api/paas/v4",
            model_name: savedConfig?.model_name ?? "glm-4-flash",
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
    expect(
      screen.getByText("只需 1 分钟完成配置。配置后就能创建会话、执行技能和驱动智能体员工协作。"),
    ).toBeInTheDocument();

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
            base_url: "https://open.bigmodel.cn/api/paas/v4",
            model_name: "glm-4-flash",
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
            base_url: "https://open.bigmodel.cn/api/paas/v4",
            model_name: "glm-4-flash",
          }),
          apiKey: "sk-test-quick-connection",
        }),
      );
    });

    expect(screen.getByTestId("quick-model-setup-test-result")).toHaveTextContent(
      "连接成功，可直接保存并开始",
    );
  });

  test("shows the full shared provider list in quick setup", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(MODEL_PROVIDER_CATALOG.length);

    for (const provider of MODEL_PROVIDER_CATALOG) {
      expect(screen.getByRole("option", { name: provider.label })).toBeInTheDocument();
    }
  });

  test("shows official console links for official providers and guidance for custom ones", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    const officialConsoleButton = screen.getByRole("button", { name: "获取 API Key" });
    expect(officialConsoleButton).toBeInTheDocument();

    fireEvent.click(officialConsoleButton);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_external_url", {
        url: "https://open.bigmodel.cn/usercenter/proj-mgmt/apikeys",
      });
    });

    fireEvent.change(screen.getByTestId("quick-model-setup-preset"), {
      target: { value: "custom-openai" },
    });

    expect(screen.queryByRole("button", { name: "获取 API Key" })).not.toBeInTheDocument();
    expect(screen.getByTestId("quick-model-setup-custom-guidance")).toHaveTextContent(
      "请向你的中转或代理服务商申请 API Key。",
    );
  });

  test("opens provider docs from quick setup with explicit desktop command", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "查看文档" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_external_url", {
        url: "https://open.bigmodel.cn/dev/api",
      });
    });
  });

  test("switches quick setup to custom anthropic and saves anthropic config", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByTestId("quick-model-setup-preset"), {
      target: { value: "custom-anthropic" },
    });
    fireEvent.change(screen.getByTestId("quick-model-setup-base-url"), {
      target: { value: "https://claude-proxy.example.com/v1" },
    });
    fireEvent.change(screen.getByTestId("quick-model-setup-model-name"), {
      target: { value: "claude-3-5-sonnet-20241022" },
    });
    fireEvent.change(screen.getByTestId("quick-model-setup-api-key"), {
      target: { value: "sk-ant-proxy-123" },
    });
    fireEvent.click(screen.getByTestId("quick-model-setup-save"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_model_config",
        expect.objectContaining({
          config: expect.objectContaining({
            api_format: "anthropic",
            base_url: "https://claude-proxy.example.com/v1",
            model_name: "claude-3-5-sonnet-20241022",
          }),
          apiKey: "sk-ant-proxy-123",
        }),
      );
    });
  });

  test("lets people reveal the API key in quick setup before saving", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    const apiKeyInput = screen.getByTestId("quick-model-setup-api-key");
    expect(apiKeyInput).toHaveAttribute("type", "password");

    fireEvent.click(screen.getByTestId("quick-model-setup-toggle-api-key-visibility"));
    expect(apiKeyInput).toHaveAttribute("type", "text");

    fireEvent.click(screen.getByTestId("quick-model-setup-toggle-api-key-visibility"));
    expect(apiKeyInput).toHaveAttribute("type", "password");
  });

  test("allows dismissing quick setup with Escape after opening it from the hint", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    fireEvent.keyDown(window, { key: "Escape" });

    await waitFor(() => {
      expect(screen.queryByTestId("quick-model-setup-dialog")).not.toBeInTheDocument();
    });
    expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
  });

  test("enforces first-launch model setup gate until at least one model is configured", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-gate")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("model-setup-hint")).not.toBeInTheDocument();
    expect(
      screen.getByText("完成模型配置后，才能开始任务、创建会话并驱动智能体员工执行技能。现在只需 1 分钟。"),
    ).toBeInTheDocument();

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

  test("keeps quick setup open on first launch when Escape is pressed", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-gate")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-gate-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    expect(screen.getByTestId("model-setup-gate")).toBeInTheDocument();
  });

  test("keeps quick setup content scrollable within the viewport on first launch", async () => {
    const focusSpy = vi.spyOn(HTMLInputElement.prototype, "focus");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-gate")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-gate-open-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("quick-model-setup-dialog")).toBeInTheDocument();
    });

    expect(screen.getByTestId("quick-model-setup-dialog").className).not.toContain("sm:items-center");
    expect(screen.getByTestId("quick-model-setup-panel")).toHaveClass("h-[calc(100vh-2rem)]");
    expect(screen.getByTestId("quick-model-setup-panel")).toHaveClass("max-h-[960px]");
    expect(screen.getByTestId("quick-model-setup-scroll-region")).toHaveClass("overflow-y-auto");
    expect(screen.getByTestId("quick-model-setup-scroll-region")).toHaveClass("flex-1");
    expect(screen.getByTestId("quick-model-setup-actions")).toBeInTheDocument();
    expect(screen.getByTestId("quick-model-setup-save")).toBeInTheDocument();
    expect(focusSpy).toHaveBeenCalledWith({ preventScroll: true });

    focusSpy.mockRestore();
  });

  test("shows non-blocking hint instead of gate after first model setup has been completed once", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("model-setup-gate")).not.toBeInTheDocument();
  });

  test("can reset first-use onboarding from dev settings tools and bring the gate back", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    window.localStorage.setItem(MODEL_SETUP_HINT_DISMISSED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.queryByTestId("model-setup-hint")).not.toBeInTheDocument();
      expect(screen.queryByTestId("model-setup-gate")).not.toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("open-settings"));

    await waitFor(() => {
      expect(screen.getByTestId("settings-view")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("reset-first-use-onboarding"));
    expect(window.localStorage.getItem(INITIAL_MODEL_SETUP_COMPLETED_KEY)).toBeNull();
    expect(window.localStorage.getItem(MODEL_SETUP_HINT_DISMISSED_KEY)).toBeNull();

    fireEvent.click(screen.getByText("close-settings"));

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-gate")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("model-setup-hint")).not.toBeInTheDocument();
  });

  test("can reopen the first-use model setup gate from dev settings tools", async () => {
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-hint")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("model-setup-hint-open-settings"));

    await waitFor(() => {
      expect(screen.getByTestId("settings-view")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("open-dev-quick-setup"));

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-gate")).toBeInTheDocument();
    });
  });
});
