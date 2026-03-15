import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ChatView } from "../ChatView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

describe("ChatView semantic theme", () => {
  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });
  });

  test("loads workspace via global list_sessions command", async () => {
    render(
      <ChatView
        skill={{
          id: "builtin-general",
          name: "General",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m1",
            name: "model",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model",
            is_default: true,
          },
        ]}
        sessionId="session-a"
      />
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("list_sessions");
    });

    expect(invokeMock.mock.calls.some((call) => call[0] === "get_sessions")).toBe(false);
  });

  test("shows the provided workdir before session hydration completes", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    render(
      <ChatView
        skill={{
          id: "builtin-general",
          name: "General",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m1",
            name: "model",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model",
            is_default: true,
          },
        ]}
        sessionId="session-a"
        workDir="E:\\workspace\\workclaw"
      />,
    );

    expect(
      await screen.findByText((content) => content.includes("workspace") && content.includes("workclaw")),
    ).toBeInTheDocument();
  });

  test("updates session workspace after selecting a new directory from the header picker", async () => {
    invokeMock.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-a",
            work_dir: "E:\\workspace\\initial",
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "select_directory") {
        expect(payload).toMatchObject({ defaultPath: "E:\\workspace\\initial" });
        return Promise.resolve("E:\\workspace\\picked");
      }
      if (command === "update_session_workspace") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    render(
      <ChatView
        skill={{
          id: "builtin-general",
          name: "General",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m1",
            name: "model",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model",
            is_default: true,
          },
        ]}
        sessionId="session-a"
      />,
    );

    const workspaceButton = await screen.findByRole("button", { name: /initial/ });
    fireEvent.click(workspaceButton);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("update_session_workspace", {
        sessionId: "session-a",
        workspace: "E:\\workspace\\picked",
      });
    });

    expect(await screen.findByRole("button", { name: /picked/ })).toBeInTheDocument();
  });

  test("uses semantic classes in composer shell", async () => {
    render(
      <ChatView
        skill={{
          id: "builtin-general",
          name: "General",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m1",
            name: "model",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model",
            is_default: true,
          },
        ]}
        sessionId="session-a"
      />
    );

    await waitFor(() => {
      expect(screen.getByPlaceholderText("输入消息，Shift+Enter 换行...")).toBeInTheDocument();
    });

    expect(screen.getByPlaceholderText("输入消息，Shift+Enter 换行...")).toHaveClass("sm-textarea");
    expect(screen.getByRole("button", { name: "发送" })).toHaveClass("sm-btn-primary");
  });

  test("shows the explicit default model in the header instead of the first model", async () => {
    render(
      <ChatView
        skill={{
          id: "builtin-general",
          name: "General",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m0",
            name: "first-model",
            api_format: "openai",
            base_url: "https://example.com/first",
            model_name: "first-model",
            is_default: false,
          },
          {
            id: "m1",
            name: "default-model",
            api_format: "openai",
            base_url: "https://example.com/default",
            model_name: "default-model",
            is_default: true,
          },
        ]}
        sessionId="session-a"
      />
    );

    expect(await screen.findByText("default-model")).toBeInTheDocument();
    expect(screen.queryByText("first-model")).not.toBeInTheDocument();
  });

  test("does not expose a manual compact button in the composer", async () => {
    render(
      <ChatView
        skill={{
          id: "builtin-general",
          name: "General",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m1",
            name: "model",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model",
            is_default: true,
          },
        ]}
        sessionId="session-a"
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "发送" })).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: "压缩" })).not.toBeInTheDocument();
  });

  test("treats /compact as a normal user message", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "send_message") return Promise.resolve({ content: "ok" });
      return Promise.resolve(null);
    });

    render(
      <ChatView
        skill={{
          id: "builtin-general",
          name: "General",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m1",
            name: "model",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model",
            is_default: true,
          },
        ]}
        sessionId="session-a"
      />
    );

    const input = await screen.findByPlaceholderText("输入消息，Shift+Enter 换行...");
    fireEvent.change(input, { target: { value: "/compact" } });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        sessionId: "session-a",
        userMessage: "/compact",
      });
    });

    expect(invokeMock.mock.calls.some((call) => call[0] === "compact_context")).toBe(false);
  });

  test("can send quick prompt directly from preset buttons", async () => {
    render(
      <ChatView
        skill={{
          id: "builtin-employee-creator",
          name: "智能体员工助手",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m1",
            name: "model",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model",
            is_default: true,
          },
        ]}
        sessionId="session-a"
        quickPrompts={[{ label: "加技能", prompt: "请帮我给 employee_a 增加 find-skills" }]}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("chat-quick-prompts")).toBeInTheDocument();
      expect(screen.getByText("加技能")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("加技能"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        sessionId: "session-a",
        userMessage: "请帮我给 employee_a 增加 find-skills",
      });
    });
  });

  test("shows employee assistant context banner in update mode", async () => {
    render(
      <ChatView
        skill={{
          id: "builtin-employee-creator",
          name: "智能体员工助手",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m1",
            name: "model",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model",
            is_default: true,
          },
        ]}
        sessionId="session-a"
        employeeAssistantContext={{
          mode: "update",
          employeeName: "项目经理",
          employeeCode: "project_manager",
        }}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("chat-employee-assistant-context")).toHaveTextContent(
        "正在修改：项目经理（project_manager）",
      );
    });
  });

  test("renders markdown table as semantic table elements", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            role: "assistant",
            content: [
              "| 员工 | 主技能 | 附加技能 |",
              "|------|--------|----------|",
              "| 玉帝 | builtin-general | builtin-find-skills |",
            ].join("\n"),
            created_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    render(
      <ChatView
        skill={{
          id: "builtin-general",
          name: "General",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m1",
            name: "model",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model",
            is_default: true,
          },
        ]}
        sessionId="session-table"
      />,
    );

    await waitFor(() => {
      expect(screen.getByRole("table")).toBeInTheDocument();
      expect(screen.getByRole("columnheader", { name: "员工" })).toBeInTheDocument();
    });
  });
});
