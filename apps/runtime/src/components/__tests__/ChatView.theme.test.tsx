import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ChatView } from "../ChatView";

const invokeMock = vi.fn();
const writeTextMock = vi.fn(() => Promise.resolve());

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
    Object.defineProperty(HTMLElement.prototype, "scrollTo", {
      configurable: true,
      value: vi.fn(),
    });
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: writeTextMock,
      },
    });
    writeTextMock.mockClear();
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

  test("updates session workspace after selecting a new directory from the composer workdir picker", async () => {
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

    const workspaceButton = await screen.findByTestId("chat-composer-workdir-button");
    fireEvent.click(workspaceButton);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("update_session_workspace", {
        sessionId: "session-a",
        workspace: "E:\\workspace\\picked",
      });
    });

    expect(await screen.findByTestId("chat-composer-workdir-label")).toHaveTextContent("picked");
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

    expect(screen.getByTestId("chat-composer-shell")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("输入消息，Shift+Enter 换行...")).toHaveClass("sm-textarea");
    expect(screen.getByRole("button", { name: "发送" })).toHaveClass("sm-btn-primary");
  });

  test("shows the explicit default model in the composer toolbar instead of the first model", async () => {
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

    expect(await screen.findByTestId("chat-composer-model-chip")).toHaveTextContent("default-model");
    expect(screen.queryByText("first-model")).not.toBeInTheDocument();
  });

  test("shows the session model in the composer toolbar when it differs from the current default", async () => {
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
            id: "m-old",
            name: "old-default",
            api_format: "openai",
            base_url: "https://example.com/old",
            model_name: "old-default",
            is_default: false,
          },
          {
            id: "m-new",
            name: "new-default",
            api_format: "openai",
            base_url: "https://example.com/new",
            model_name: "new-default",
            is_default: true,
          },
        ]}
        sessionId="session-a"
        sessionModelId="m-old"
      />
    );

    expect(await screen.findByTestId("chat-composer-model-chip")).toHaveTextContent("old-default");
    expect(screen.queryByText("new-default")).not.toBeInTheDocument();
  });

  test("falls back to the current default model when the session model is no longer available", async () => {
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
            id: "m-new",
            name: "new-default",
            api_format: "openai",
            base_url: "https://example.com/new",
            model_name: "new-default",
            is_default: true,
          },
        ]}
        sessionId="session-a"
        sessionModelId="m-missing"
      />
    );

    expect(await screen.findByTestId("chat-composer-model-chip")).toHaveTextContent("new-default");
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
        request: {
          sessionId: "session-a",
          parts: [{ type: "text", text: "/compact" }],
        },
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
        request: {
          sessionId: "session-a",
          parts: [{ type: "text", text: "请帮我给 employee_a 增加 find-skills" }],
        },
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
              "# 当前工作目录",
              "",
              "当前工作目录中有以下文件：",
              "",
              "| 员工 | 主技能 | 附加技能 |",
              "|------|--------|----------|",
              "| 玉帝 | builtin-general | builtin-find-skills |",
              "",
              "共计 1 个条目。",
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

    const heading = screen.getByRole("heading", { name: "当前工作目录" });
    const table = screen.getByRole("table");
    const headerCell = screen.getByRole("columnheader", { name: "员工" });
    const summary = screen.getByText("当前工作目录中有以下文件：");
    const tableShell = table.parentElement;
    const resultSummary = screen.getByTestId("assistant-result-summary");

    expect(heading.className).toContain("text-[1.75rem]");
    expect(heading.className).toContain("tracking-[-0.02em]");
    expect(summary.className).toContain("leading-7");
    expect(tableShell?.className).toContain("bg-white/90");
    expect(tableShell?.className).toContain("shadow-[0_1px_2px_rgba(15,23,42,0.04)]");
    expect(headerCell.className).toContain("bg-slate-50/90");
    expect(headerCell.className).toContain("font-semibold");
    expect(resultSummary).toHaveTextContent("共计 1 个条目。");
    expect(resultSummary.className).toContain("rounded-2xl");
    expect(resultSummary.className).toContain("bg-slate-50/80");
  });

  test("shows a lightweight copy action for assistant results", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            id: "assistant-copy",
            role: "assistant",
            content: "这是最终结果。",
            created_at: new Date().toISOString(),
          },
        ]);
      }
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
        sessionId="session-copy"
      />,
    );

    const copyButton = await screen.findByTestId("assistant-copy-action-assistant-copy");
    fireEvent.click(copyButton);

    await waitFor(() => {
      expect(writeTextMock).toHaveBeenCalledWith("这是最终结果。");
    });

    expect(copyButton).toHaveAttribute("aria-label", "复制回答");
    expect(copyButton).toHaveAttribute("title", "已复制");
  });

  test("opens chat markdown http links via the desktop external-url command", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            id: "assistant-link",
            role: "assistant",
            content: "来源：[新浪财经](https://finance.sina.com.cn/stock/jdts/2026-03-09/detail-inhqkxfk6921924.d.html)",
            created_at: new Date().toISOString(),
          },
        ]);
      }
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
        sessionId="session-link"
      />,
    );

    const link = await screen.findByRole("link", { name: "新浪财经" });
    fireEvent.click(link);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_external_url", {
        url: "https://finance.sina.com.cn/stock/jdts/2026-03-09/detail-inhqkxfk6921924.d.html",
      });
    });

    expect(await screen.findByTestId("chat-link-toast")).toHaveTextContent("已在浏览器打开");
  });

  test("shows a retry action when opening a chat markdown link fails", async () => {
    let openAttempts = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            id: "assistant-link-error",
            role: "assistant",
            content: "来源：[网易新闻](https://m.163.com/dy/article/KOCKINIG0519CIKK.html)",
            created_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "open_external_url") {
        openAttempts += 1;
        if (openAttempts === 1) {
          return Promise.reject(new Error("blocked"));
        }
        return Promise.resolve(null);
      }
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
        sessionId="session-link-error"
      />,
    );

    const link = await screen.findByRole("link", { name: "网易新闻" });
    fireEvent.click(link);

    expect(await screen.findByTestId("chat-link-toast")).toHaveTextContent("链接打开失败");
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await waitFor(() => {
      expect(openAttempts).toBe(2);
      expect(screen.getByTestId("chat-link-toast")).toHaveTextContent("已在浏览器打开");
    });
  });

  test("shows a copy-link action when opening a chat markdown link fails", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            id: "assistant-link-copy",
            role: "assistant",
            content: "来源：[网易新闻](https://m.163.com/dy/article/KOCKINIG0519CIKK.html)",
            created_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "open_external_url") {
        return Promise.reject(new Error("blocked"));
      }
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
        sessionId="session-link-copy"
      />,
    );

    const link = await screen.findByRole("link", { name: "网易新闻" });
    fireEvent.click(link);

    expect(await screen.findByTestId("chat-link-toast")).toHaveTextContent("链接打开失败");

    fireEvent.click(screen.getByRole("button", { name: "复制链接" }));
    await waitFor(() => {
      expect(writeTextMock).toHaveBeenCalledWith("https://m.163.com/dy/article/KOCKINIG0519CIKK.html");
      expect(screen.getByTestId("chat-link-toast")).toHaveTextContent("链接已复制");
    });
  });
});
