import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../App";

const invokeMock = vi.fn();
const openMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openMock(...args),
  save: vi.fn(),
}));

vi.mock("../components/Sidebar", () => ({
  Sidebar: (props: any) => (
    <div data-testid="sidebar">
      <div data-testid="sidebar-session-count">{props.sessions?.length ?? 0}</div>
      <div data-testid="sidebar-first-session-id">{props.sessions?.[0]?.id ?? ""}</div>
      <div data-testid="sidebar-first-session-title">
        {props.sessions?.[0]?.display_title || props.sessions?.[0]?.title || ""}
      </div>
    </div>
  ),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: (props: any) => (
    <div data-testid="chat-view">
      chat-view
      {props.initialMessage ? <span data-testid="chat-initial-message">{props.initialMessage}</span> : null}
      {props.workDir ? <span data-testid="chat-work-dir">{props.workDir}</span> : null}
    </div>
  ),
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
  SettingsView: () => <div data-testid="settings-view">settings-view</div>,
}));

vi.mock("../components/InstallDialog", () => ({
  InstallDialog: () => <div data-testid="install-dialog">install-dialog</div>,
}));

vi.mock("../components/NewSessionLanding", () => ({
  NewSessionLanding: (props: any) => (
    <div>
      <button onClick={() => props.onCreateSessionWithInitialMessage("整理本地文件")}>
        create-with-input
      </button>
      <button onClick={() => props.onCreateSessionWithInitialMessage("")}>create-empty</button>
    </div>
  ),
}));

describe("App session create flow", () => {
  afterEach(() => {
    cleanup();
    window.localStorage.clear();
    window.location.hash = "";
  });

  beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();

    invokeMock.mockImplementation((command: string, payload?: any) => {
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
            id: "model-non-default",
            name: "Model Non Default",
            api_format: "openai",
            base_url: "https://example.com/non-default",
            model_name: "model-non-default",
            is_default: false,
          },
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
      if (command === "get_sessions") {
        return Promise.resolve([]);
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([]);
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "standard",
        });
      }
      if (command === "create_session") {
        return Promise.resolve("session-new-1");
      }
      if (command === "send_message") {
        return Promise.resolve(null);
      }
      return Promise.resolve(payload ?? null);
    });
  });

  test("creates session and forwards initial message to chat view", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-with-input" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-with-input" }));

    await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          skillId: "builtin-general",
          modelId: "model-a",
          workDir: "",
          permissionMode: "standard",
        })
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });
    expect(screen.getByTestId("chat-initial-message")).toHaveTextContent("整理本地文件");
  });

  test("creates a general session with the default workdir and passes it into chat view", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
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
      if (command === "get_sessions") {
        return Promise.resolve([]);
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([]);
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "E:\\workspace\\workclaw",
          operation_permission_mode: "standard",
        });
      }
      if (command === "create_session") {
        return Promise.resolve("session-new-default-dir");
      }
      return Promise.resolve(payload ?? null);
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-empty" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-empty" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          skillId: "builtin-general",
          modelId: "model-a",
          workDir: "E:\\workspace\\workclaw",
          permissionMode: "standard",
          sessionMode: "general",
        }),
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId("chat-work-dir")).toHaveTextContent("E:\\workspace\\workclaw");
    });
  });

  test("creates empty session without sending first message when input is empty", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-empty" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-empty" }));

    await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          skillId: "builtin-general",
          workDir: "",
          permissionMode: "standard",
        })
      );
    });

    expect(
      invokeMock.mock.calls.some((call) => call[0] === "send_message")
    ).toBe(false);
  });

  test("enters chat immediately and carries initial message", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
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
            id: "model-non-default",
            name: "Model Non Default",
            api_format: "openai",
            base_url: "https://example.com/non-default",
            model_name: "model-non-default",
            is_default: false,
          },
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
      if (command === "get_sessions") {
        return Promise.resolve([]);
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([]);
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "full_access",
        });
      }
      if (command === "create_session") {
        return Promise.resolve("session-new-1");
      }
      return Promise.resolve(payload ?? null);
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-with-input" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-with-input" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });
    expect(screen.getByTestId("chat-initial-message")).toHaveTextContent("整理本地文件");
    expect(invokeMock).toHaveBeenCalledWith(
      "create_session",
      expect.objectContaining({
        permissionMode: "full_access",
      })
    );
  });

  test("does not require directory picker before creating session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-with-input" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-with-input" }));

    await waitFor(() => {
      expect(
        invokeMock.mock.calls.some((call) => call[0] === "create_session")
      ).toBe(true);
    });
    expect(openMock).not.toHaveBeenCalled();
  });

  test("uses the explicit default model instead of the first model in the list", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-with-input" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-with-input" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          modelId: "model-a",
        })
      );
    });
  });

  test("retains the newly created session and reports diagnostics when list refresh fails", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
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
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "standard",
        });
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([]);
      }
      if (command === "create_session") {
        return Promise.resolve("session-new-1");
      }
      if (command === "list_sessions") {
        return Promise.reject(new Error("database is locked"));
      }
      if (command === "record_frontend_diagnostic_event") {
        return Promise.resolve(null);
      }
      return Promise.resolve(payload ?? null);
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-empty" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-empty" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(screen.getByTestId("sidebar-session-count")).toHaveTextContent("1");
      expect(screen.getByTestId("sidebar-first-session-id")).toHaveTextContent("session-new-1");
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "record_frontend_diagnostic_event",
        expect.objectContaining({
          payload: expect.objectContaining({
            kind: "session_list_load_failed",
            message: expect.stringContaining("database is locked"),
          }),
        }),
      );
    });
  });

  test("prefers display_title over title in the sidebar session list", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
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
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "standard",
        });
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([]);
      }
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-1",
            title: "New Chat",
            display_title: "修复登录接口超时",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            work_dir: "",
            employee_id: "",
            session_mode: "general",
            team_id: "",
          },
        ]);
      }
      return Promise.resolve(payload ?? null);
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("sidebar-first-session-title")).toHaveTextContent(
        "修复登录接口超时",
      );
    });
  });

  test("derives optimistic display title from the initial user message for general sessions", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
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
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "standard",
        });
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([]);
      }
      if (command === "create_session") {
        return Promise.resolve("session-new-2");
      }
      if (command === "list_sessions") {
        return Promise.reject(new Error("database is locked"));
      }
      if (command === "record_frontend_diagnostic_event") {
        return Promise.resolve(null);
      }
      return Promise.resolve(payload ?? null);
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-with-input" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-with-input" }));

    await waitFor(() => {
      expect(screen.getByTestId("sidebar-first-session-id")).toHaveTextContent("session-new-2");
      expect(screen.getByTestId("sidebar-first-session-title")).toHaveTextContent("整理本地文件");
    });
  });
});
