import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../App";

const invokeMock = vi.fn();
const INITIAL_MODEL_SETUP_COMPLETED_KEY = "workclaw:initial-model-setup-completed";
let importShouldFailOnce = false;
let importCallCount = 0;
let createShouldConflict = false;
let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../components/Sidebar", () => ({
  Sidebar: (props: any) => (
    <div>
      <button onClick={props.onOpenStartTask}>start-task</button>
      <button onClick={props.onOpenExperts}>experts</button>
      <button onClick={props.onSettings}>settings</button>
    </div>
  ),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: (props: any) => (
    <div data-testid="chat-view">
      chat-view
      <div data-testid="chat-session-title">{props.sessionTitle || ""}</div>
      <div data-testid="chat-work-dir">{props.workDir || ""}</div>
    </div>
  ),
}));

vi.mock("../components/packaging/PackagingView", () => ({
  PackagingView: () => <div data-testid="packaging-view">packaging-view</div>,
}));

vi.mock("../components/SettingsView", () => ({
  SettingsView: () => <div data-testid="settings-view">settings-view</div>,
}));

vi.mock("../components/InstallDialog", () => ({
  InstallDialog: () => <div data-testid="install-dialog">install-dialog</div>,
}));

vi.mock("../components/NewSessionLanding", () => ({
  NewSessionLanding: () => <div data-testid="new-session-landing">new-session-landing</div>,
}));

vi.mock("../components/experts/ExpertsView", () => ({
  ExpertsView: (props: any) => (
    <div data-testid="experts-view">
      <div>我的技能</div>
      <button onClick={props.onInstallSkill}>install-skill</button>
      <button onClick={props.onCreate}>create-expert</button>
      <button onClick={() => props.onStartTaskWithSkill?.("local-test-skill")}>start-task-local</button>
      <button onClick={() => props.onRefreshLocalSkill?.("local-test-skill")}>refresh-local</button>
      <button onClick={() => props.onDeleteSkill?.("local-test-skill")}>delete-local</button>
    </div>
  ),
}));

vi.mock("../components/experts/ExpertCreateView", () => ({
  ExpertCreateView: (props: any) => (
    <div data-testid="experts-new-view">
      <button
        onClick={() =>
          props.onSave({
            name: "测试技能",
            description: "描述",
            whenToUse: "需要自动化整理任务时",
          })
        }
      >
        save-expert
      </button>
      {props.savedPath && <div>{props.savedPath}</div>}
      {props.error && <div>{props.error}</div>}
      <button
        disabled={!props.canRetryImport}
        onClick={() => props.onRetryImport?.()}
      >
        retry-import
      </button>
      <button onClick={props.onBack}>back-expert</button>
    </div>
  ),
}));

describe("App experts routing", () => {
  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    invokeMock.mockReset();
    importShouldFailOnce = false;
    importCallCount = 0;
    createShouldConflict = false;
    window.localStorage.clear();
    window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
    window.location.hash = "";
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
          {
            id: "local-test-skill",
            name: "Local Test Skill",
            description: "local expert skill",
            version: "local",
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
            name: "Brave Search",
            api_format: "search_brave",
            base_url: "https://api.search.brave.com",
            model_name: "",
            is_default: true,
          },
        ]);
      }
      if (command === "list_sessions") {
        return Promise.resolve([]);
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "E:\\workspace\\experts",
          operation_permission_mode: "standard",
        });
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([]);
      }
      if (command === "list_employee_groups") {
        return Promise.resolve([]);
      }
      if (command === "create_local_skill") {
        if (createShouldConflict) {
          return Promise.reject(new Error("技能目录已存在: E:/code/yzpd/skillhub/temp/new-skill"));
        }
        return Promise.resolve("E:/code/yzpd/skillhub/temp/new-skill");
      }
      if (command === "import_local_skill") {
        importCallCount += 1;
        if (importShouldFailOnce && importCallCount === 1) {
          return Promise.reject(new Error("导入失败: 模板解析错误"));
        }
        return Promise.resolve({
          installed: [{
            manifest: {
              id: "local-test-skill",
              name: "测试技能",
              description: "描述",
              version: "local",
              author: "",
              recommended_model: "",
              tags: [],
              created_at: new Date().toISOString(),
            },
          }],
          failed: [],
          missing_mcp: [],
        });
      }
      if (command === "create_session") {
        return Promise.resolve("session-expert-skill-1");
      }
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    cleanup();
    window.localStorage.clear();
    window.location.hash = "";
    consoleErrorSpy.mockRestore();
  });

  test("navigates to experts and create page", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "experts" }));

    await waitFor(() => {
      expect(screen.getByTestId("experts-view")).toBeInTheDocument();
    });

    expect(screen.getByText("我的技能")).toBeInTheDocument();
    expect(screen.queryByText("技能社区")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "create-expert" }));
    await waitFor(() => {
      expect(screen.getByTestId("experts-new-view")).toBeInTheDocument();
    });
  });

  test("saves expert skill through create flow", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "experts" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-expert" })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "create-expert" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "save-expert" })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "save-expert" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "create_local_skill",
        expect.objectContaining({ name: "测试技能" })
      );
    });
    expect(invokeMock).toHaveBeenCalledWith(
      "import_local_skill",
      expect.objectContaining({ dirPath: "E:/code/yzpd/skillhub/temp/new-skill" })
    );
  });

  test("shows saved path and supports retry when import fails", async () => {
    importShouldFailOnce = true;
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "experts" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-expert" })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "create-expert" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "save-expert" })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "save-expert" }));

    await waitFor(() => {
      expect(screen.getByText("E:/code/yzpd/skillhub/temp/new-skill")).toBeInTheDocument();
      expect(screen.getByText(/导入失败/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "retry-import" }));
    await waitFor(() => {
      const importCalls = invokeMock.mock.calls.filter((c) => c[0] === "import_local_skill");
      expect(importCalls.length).toBe(2);
    });
  });

  test("exposes backend conflict message when create path already exists", async () => {
    createShouldConflict = true;
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "experts" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-expert" })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "create-expert" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "save-expert" })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "save-expert" }));

    await waitFor(() => {
      expect(screen.getByText(/技能目录已存在/)).toBeInTheDocument();
    });
  });

  test("supports refresh and delete actions from experts list", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "experts" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "refresh-local" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "delete-local" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "refresh-local" }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("refresh_local_skill", {
        skillId: "local-test-skill",
      });
    });

    fireEvent.click(screen.getByRole("button", { name: "delete-local" }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("delete_skill", {
        skillId: "local-test-skill",
      });
    });
  });

  test("starts an expert skill by creating a session and opening chat directly", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "experts" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "start-task-local" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "start-task-local" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          skillId: "local-test-skill",
          modelId: "model-a",
          workDir: "E:\\workspace\\experts",
          title: "Local Test Skill",
          employeeId: "",
          permissionMode: "standard",
          sessionMode: "general",
          teamId: "",
        }),
      );
    });
    await waitFor(() => {
      expect(screen.queryByTestId("new-session-landing")).not.toBeInTheDocument();
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });
    expect(screen.getByTestId("chat-session-title")).toHaveTextContent("Local Test Skill");
    expect(screen.getByTestId("chat-work-dir")).toHaveTextContent("E:\\workspace\\experts");
  });

  test("opens install dialog from experts view", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "experts" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "install-skill" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "install-skill" }));

    await waitFor(() => {
      expect(screen.getByTestId("install-dialog")).toBeInTheDocument();
    });
  });

  test("leaves settings when opening experts or start task from sidebar", async () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "settings" }));
    await waitFor(() => {
      expect(screen.getByTestId("settings-view")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "experts" }));
    await waitFor(() => {
      expect(screen.getByTestId("experts-view")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("settings-view")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "settings" }));
    await waitFor(() => {
      expect(screen.getByTestId("settings-view")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "start-task" }));
    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("settings-view")).not.toBeInTheDocument();
  });
});
