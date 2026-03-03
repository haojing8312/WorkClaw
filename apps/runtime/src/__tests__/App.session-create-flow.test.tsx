import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  Sidebar: () => <div data-testid="sidebar">sidebar</div>,
}));

vi.mock("../components/ChatView", () => ({
  ChatView: (props: any) => (
    <div data-testid="chat-view">
      chat-view
      {props.initialMessage ? <span data-testid="chat-initial-message">{props.initialMessage}</span> : null}
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
        })
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });
    expect(screen.getByTestId("chat-initial-message")).toHaveTextContent("整理本地文件");
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
});
