import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ChatView } from "../ChatView";
import type { ChatMessagePart, PendingAttachment } from "../../types";

const invokeMock = vi.fn<(command: string, payload?: unknown) => Promise<unknown>>();
const listenMock = vi.fn<(eventName: string, callback: unknown) => Promise<() => void>>(
  () => Promise.resolve(() => {}),
);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, payload?: unknown) => invokeMock(command, payload),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (eventName: string, callback: unknown) => listenMock(eventName, callback),
}));

it("defines structured attachment and message-part frontend types", () => {
  const attachment: PendingAttachment = {
    id: "att-1",
    kind: "text-file",
    name: "notes.md",
    mimeType: "text/markdown",
    size: 128,
    text: "# hello",
  };
  const part: ChatMessagePart = {
    type: "file_text",
    name: attachment.name,
    mimeType: attachment.mimeType,
    size: attachment.size,
    text: attachment.text,
  };

  expect(part.type).toBe("file_text");
});

function buildMessages() {
  return [
    {
      id: "assistant-1",
      role: "assistant",
      content: "任务和产出已整理。",
      created_at: new Date().toISOString(),
      runId: "run-1",
      streamItems: [
        {
          type: "tool_call",
          toolCall: {
            id: "todo-1",
            name: "todo_write",
            input: {
              todos: [
                { id: "t1", content: "创建美国以色列伊朗冲突Word简报", status: "completed", priority: "high" },
                { id: "t2", content: "创建带动画和时间轴的HTML报告", status: "in_progress", priority: "high" },
              ],
            },
            output:
              "已更新任务列表（共 2 项）:\n- [completed][high] 创建美国以色列伊朗冲突Word简报 (ID: t1)\n- [in_progress][high] 创建带动画和时间轴的HTML报告 (ID: t2)",
            status: "completed",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-1",
            name: "write_file",
            input: {
              path: "conflict_brief.docx",
              content: "docx placeholder",
            },
            output: "成功写入 1024 字节到 conflict_brief.docx",
            status: "completed",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-2",
            name: "write_file",
            input: {
              path: "conflict_report.html",
              content: "<html><body>report</body></html>",
            },
            output: "成功写入 2048 字节到 conflict_report.html",
            status: "completed",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "search-1",
            name: "web_search",
            input: {
              query: "US military presence Middle East 2025",
            },
            output: JSON.stringify({
              query: "US military presence Middle East 2025",
              results: [
                {
                  title: "2025年美国军事部署看点会有哪些?",
                  url: "https://news.example.com/a",
                  snippet: "2025年美国的军事部署总体可能将呈现收缩状态。",
                },
                {
                  title: "美国在中东的军事力量正显著增加",
                  url: "https://news.example.com/b",
                  snippet: "自2023年10月7日以来，美国已经显著增加了其在中东的军事存在。",
                },
              ],
            }),
            status: "completed",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "search-2",
            name: "web_search",
            input: {
              query: "Iran Israel conflict timeline 2025",
            },
            output: JSON.stringify({
              query: "Iran Israel conflict timeline 2025",
              results: [
                {
                  title: "2025 conflict timeline",
                  url: "https://timeline.example.com",
                  snippet: "Timeline overview.",
                },
              ],
            }),
            status: "completed",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-fail-1",
            name: "write_file",
            input: {},
            output: "工具执行错误: 缺少 path 参数",
            status: "error",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-fail-2",
            name: "write_file",
            input: {},
            output: "工具执行错误: 缺少 path 参数",
            status: "error",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-fail-3",
            name: "write_file",
            input: {},
            output: "工具执行错误: 缺少 path 参数",
            status: "error",
          },
        },
      ],
    },
  ];
}

function buildSplitJourneyMessages() {
  return [
    {
      id: "assistant-a",
      role: "assistant",
      content: "第一轮任务和产出已整理。",
      created_at: "2026-03-11T00:00:01Z",
      runId: "run-a",
      streamItems: [
        {
          type: "tool_call",
          toolCall: {
            id: "todo-a",
            name: "todo_write",
            input: {
              todos: [{ id: "t-a", content: "完成第一轮交付", status: "in_progress", priority: "high" }],
            },
            output: "已更新任务列表（共 1 项）",
            status: "completed",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-a",
            name: "write_file",
            input: {
              path: "round-one-report.html",
              content: "<html></html>",
            },
            output: "成功写入 1000 字节到 round-one-report.html",
            status: "completed",
          },
        },
      ],
    },
    {
      id: "assistant-b",
      role: "assistant",
      content: "第二轮只是补充说明，没有新的交付。",
      created_at: "2026-03-11T00:00:02Z",
      runId: "run-b",
    },
  ];
}

function buildPartialJourneyMessages() {
  return [
    {
      id: "assistant-partial",
      role: "assistant",
      content: "已生成部分文件，仍有补做项。",
      created_at: "2026-03-11T00:00:03Z",
      runId: "run-partial",
      streamItems: [
        {
          type: "tool_call",
          toolCall: {
            id: "todo-partial",
            name: "todo_write",
            input: {
              todos: [{ id: "t-partial", content: "生成报告与附录", status: "completed", priority: "high" }],
            },
            output: "已更新任务列表（共 1 项）",
            status: "completed",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-partial",
            name: "write_file",
            input: {
              path: "partial-report.html",
              content: "<html></html>",
            },
            output: "成功写入 888 字节到 partial-report.html",
            status: "completed",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-partial-error",
            name: "write_file",
            input: {},
            output: "工具执行错误: 缺少 path 参数",
            status: "error",
          },
        },
      ],
    },
  ];
}

function buildRunningJourneyMessages() {
  return [
    {
      id: "assistant-running",
      role: "assistant",
      content: "还在继续生成中。",
      created_at: "2026-03-11T00:00:04Z",
      runId: "run-running",
      streamItems: [
        {
          type: "tool_call",
          toolCall: {
            id: "todo-running",
            name: "todo_write",
            input: {
              todos: [{ id: "t-running", content: "持续生成文件", status: "in_progress", priority: "high" }],
            },
            output: "已更新任务列表（共 1 项）",
            status: "completed",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-running",
            name: "write_file",
            input: {
              path: "running-report.html",
              content: "<html></html>",
            },
            output: "正在写入文件",
            status: "running",
          },
        },
      ],
    },
  ];
}

function buildFailedJourneyMessages() {
  return [
    {
      id: "assistant-failed",
      role: "assistant",
      content: "这轮交付失败，没有产物。",
      created_at: "2026-03-11T00:00:05Z",
      runId: "run-failed",
      streamItems: [
        {
          type: "tool_call",
          toolCall: {
            id: "todo-failed",
            name: "todo_write",
            input: {
              todos: [{ id: "t-failed", content: "生成失败文件", status: "in_progress", priority: "high" }],
            },
            output: "已更新任务列表（共 1 项）",
            status: "completed",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-failed",
            name: "write_file",
            input: {},
            output: "工具执行错误: 无法写入目标路径",
            status: "error",
          },
        },
      ],
    },
  ];
}

function renderChat() {
  return render(
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
      sessionId="session-side-panel-redesign"
    />
  );
}

function renderEmptyChat(overrides?: Partial<React.ComponentProps<typeof ChatView>>) {
  invokeMock.mockImplementation((command: string) => {
    if (command === "get_messages") return Promise.resolve([]);
    if (command === "list_sessions") {
      return Promise.resolve([
        {
          id: "session-side-panel-redesign",
          work_dir: "E:\\workspace\\session-side-panel-redesign",
        },
      ]);
    }
    if (command === "get_sessions") return Promise.resolve([]);
    return Promise.resolve(null);
  });

  return render(
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
      sessionId="session-side-panel-redesign"
      {...overrides}
    />
  );
}

describe("ChatView side panel redesign", () => {
  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    Object.defineProperty(window, "scrollTo", {
      configurable: true,
      value: vi.fn(),
    });
    Object.defineProperty(window, "alert", {
      configurable: true,
      value: vi.fn(),
    });
    invokeMock.mockReset();
    listenMock.mockClear();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve(buildMessages());
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-side-panel-redesign",
            work_dir: "E:\\workspace\\session-side-panel-redesign",
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_workspace_files") {
        return Promise.resolve([
          { path: ".minimax", name: ".minimax", size: 0, kind: "directory" },
          { path: "conflict_brief.docx", name: "conflict_brief.docx", size: 17 * 1024, kind: "binary" },
          { path: "conflict_brief.md", name: "conflict_brief.md", size: 8.8 * 1024, kind: "markdown" },
          { path: "conflict_report.html", name: "conflict_report.html", size: 26.6 * 1024, kind: "html" },
        ]);
      }
      if (command === "read_workspace_file_preview") {
        return Promise.resolve({
          path: "conflict_report.html",
          kind: "html",
          source: "<html><body><h1>Conflict Report</h1></body></html>",
        });
      }
      if (command === "open_external_url") return Promise.resolve(null);
      return Promise.resolve(null);
    });
  });

  test("does not subscribe removed route side-panel events", async () => {
    renderChat();

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_messages", {
        sessionId: "session-side-panel-redesign",
      });
    });

    const registeredEvents = listenMock.mock.calls.map((call) => String(call[0]));
    expect(registeredEvents).toContain("stream-token");
    expect(registeredEvents).not.toContain("skill-route-node-updated");
    expect(registeredEvents).not.toContain("im-route-decision");
  });

  test("replaces old tabs with current task files and web search tabs", async () => {
    renderChat();

    fireEvent.click(screen.getByRole("button", { name: "面板" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "当前任务" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "文件" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Web 搜索" })).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: "附件与工具" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "自动路由" })).not.toBeInTheDocument();
  });

  test("shows current task summary from todo tool calls", async () => {
    renderChat();

    fireEvent.click(screen.getByRole("button", { name: "面板" }));

    await waitFor(() => {
      expect(screen.getAllByText("当前任务").length).toBeGreaterThan(0);
      expect(screen.getByText("创建美国以色列伊朗冲突Word简报")).toBeInTheDocument();
      expect(screen.getAllByText("创建带动画和时间轴的HTML报告").length).toBeGreaterThan(0);
    });

    expect(screen.getByText("总任务数")).toBeInTheDocument();
    expect(screen.getByText("已完成")).toBeInTheDocument();
    expect(screen.getAllByText("进行中").length).toBeGreaterThan(0);
    expect(screen.getByText(/本轮生成文件/)).toBeInTheDocument();
    expect(screen.getByText(/本轮 Web 搜索/)).toBeInTheDocument();
  });

  test("shows workspace files and html dual preview modes", async () => {
    renderChat();

    fireEvent.click(screen.getByRole("button", { name: "面板" }));
    fireEvent.click(await screen.findByRole("button", { name: "文件" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-workspace-drawer")).toHaveStyle({ width: "760px" });
      expect(screen.getByPlaceholderText("搜索文件...")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "conflict_brief.docx" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "conflict_report.html" })).toBeInTheDocument();
      expect(screen.getByText("选择要查看的文件")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "conflict_report.html" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "页面预览" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "源码预览" })).toBeInTheDocument();
    });
  });

  test("shows session web searches and confirms before opening result links", async () => {
    renderChat();

    fireEvent.click(screen.getByRole("button", { name: "面板" }));
    fireEvent.click(await screen.findByRole("button", { name: "Web 搜索" }));

    await waitFor(() => {
      expect(screen.getAllByText("US military presence Middle East 2025").length).toBeGreaterThan(0);
      expect(screen.getByRole("button", { name: /Iran Israel conflict timeline 2025/ })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: /US military presence Middle East 2025/ }));

    await waitFor(() => {
      expect(screen.getByText("2025年美国军事部署看点会有哪些?")).toBeInTheDocument();
      expect(screen.getByText("美国在中东的军事力量正显著增加")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("2025年美国军事部署看点会有哪些?"));

    await waitFor(() => {
      expect(screen.getByText("将在系统浏览器打开此链接，是否继续？")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "继续打开" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_external_url", {
        url: "https://news.example.com/a",
      });
    });
  });

  test("shows a completed-state files entry card in the transcript and opens the files panel on click", async () => {
    renderChat();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "查看此任务中的所有文件" })).toBeInTheDocument();
      expect(screen.getByText("任务已完成，点击查看本次产出文件")).toBeInTheDocument();
      expect(screen.getByText("共 2 个文件")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "查看此任务中的所有文件" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "文件" })).toHaveClass("bg-blue-100");
      expect(screen.getByPlaceholderText("搜索文件...")).toBeInTheDocument();
    });
  });

  test("shows mixed image and text-file attachment previews", async () => {
    renderEmptyChat();

    const fileInput = document.getElementById("file-upload") as HTMLInputElement;
    const imageFile = new File(["image-bytes"], "screen.png", { type: "image/png" });
    const textFile = new File(["console.log('hi')"], "debug.ts", { type: "text/plain" });

    fireEvent.change(fileInput, {
      target: {
        files: [imageFile, textFile],
      },
    });

    expect(await screen.findByText("screen.png")).toBeInTheDocument();
    expect(await screen.findByText("debug.ts")).toBeInTheDocument();
    expect(screen.getByText("图片")).toBeInTheDocument();
    expect(screen.getByText("文本")).toBeInTheDocument();
  });

  test("renders multiple pending attachments and removes one by id", async () => {
    renderEmptyChat();

    const fileInput = document.getElementById("file-upload") as HTMLInputElement;
    const firstFile = new File(["alpha"], "first.txt", { type: "text/plain" });
    const secondFile = new File(["beta"], "second.txt", { type: "text/plain" });

    fireEvent.change(fileInput, {
      target: {
        files: [firstFile, secondFile],
      },
    });

    expect(await screen.findByText("first.txt")).toBeInTheDocument();
    expect(await screen.findByText("second.txt")).toBeInTheDocument();

    const removeButtons = screen.getAllByRole("button", { name: "移除附件" });
    fireEvent.click(removeButtons[0]!);

    await waitFor(() => {
      expect(screen.queryByText("first.txt")).not.toBeInTheDocument();
    });
    expect(screen.getByText("second.txt")).toBeInTheDocument();
  });

  test("rejects unsupported attachment types and oversize files", async () => {
    renderEmptyChat();

    const fileInput = document.getElementById("file-upload") as HTMLInputElement;
    const badFile = new File(["fake"], "clip.mp4", { type: "video/mp4" });
    const largeFile = new File(["big"], "huge.txt", { type: "text/plain" });
    Object.defineProperty(largeFile, "size", {
      configurable: true,
      value: 6 * 1024 * 1024,
    });

    fireEvent.change(fileInput, {
      target: {
        files: [badFile, largeFile],
      },
    });

    await waitFor(() => {
      expect(window.alert).toHaveBeenCalled();
    });
    expect(screen.queryByText("clip.mp4")).not.toBeInTheDocument();
    expect(screen.queryByText("huge.txt")).not.toBeInTheDocument();
  });

  test("sends text plus mixed attachment parts in user order", async () => {
    renderEmptyChat();

    fireEvent.change(screen.getByPlaceholderText("输入消息，Shift+Enter 换行..."), {
      target: { value: "帮我一起分析这些附件" },
    });

    const fileInput = document.getElementById("file-upload") as HTMLInputElement;
    const imageFile = new File(["image-bytes"], "screen.png", { type: "image/png" });
    const textFile = new File(["hello"], "notes.md", { type: "text/plain" });

    fireEvent.change(fileInput, {
      target: {
        files: [imageFile, textFile],
      },
    });

    await screen.findByText("screen.png");
    await screen.findByText("notes.md");

    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-side-panel-redesign",
          parts: [
            { type: "text", text: "帮我一起分析这些附件" },
            expect.objectContaining({
              type: "image",
              name: "screen.png",
              mimeType: "image/png",
            }),
            expect.objectContaining({
              type: "file_text",
              name: "notes.md",
              mimeType: "text/plain",
              text: "hello",
            }),
          ],
        },
      });
    });
  });

  test("injects default prompt when attachments exist and input is empty", async () => {
    renderEmptyChat();

    const fileInput = document.getElementById("file-upload") as HTMLInputElement;
    const imageFile = new File(["image-bytes"], "screen.png", { type: "image/png" });

    fireEvent.change(fileInput, {
      target: {
        files: [imageFile],
      },
    });

    await screen.findByText("screen.png");
    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-side-panel-redesign",
          parts: [
            {
              type: "text",
              text: "请结合这些图片描述主要内容，并提取可见文字。",
            },
            expect.objectContaining({
              type: "image",
              name: "screen.png",
              mimeType: "image/png",
            }),
          ],
        },
      });
    });
  });

  test("maps missing vision route error to a user-friendly message", async () => {
    renderEmptyChat();

    invokeMock.mockImplementation((command: string) => {
      if (command === "send_message") {
        return Promise.reject("VISION_MODEL_NOT_CONFIGURED: 请先在设置中配置图片理解模型");
      }
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-side-panel-redesign",
            work_dir: "E:\\workspace\\session-side-panel-redesign",
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    const fileInput = document.getElementById("file-upload") as HTMLInputElement;
    const imageFile = new File(["image-bytes"], "screen.png", { type: "image/png" });

    fireEvent.change(fileInput, {
      target: {
        files: [imageFile],
      },
    });

    await screen.findByText("screen.png");
    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    expect(await screen.findByText("请先在设置中配置图片理解模型")).toBeInTheDocument();
  });

  test("renders persisted user attachment history from contentParts", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            id: "user-attachment-history",
            role: "user",
            content: "请结合附件一起分析",
            contentParts: [
              { type: "text", text: "请结合附件一起分析" },
              {
                type: "image",
                name: "screen.png",
                mimeType: "image/png",
                size: 12,
                data: "data:image/png;base64,aGVsbG8=",
              },
              {
                type: "file_text",
                name: "debug.ts",
                mimeType: "text/plain",
                size: 18,
                text: "console.log('hi')",
              },
            ],
            created_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-side-panel-redesign",
            work_dir: "E:\\workspace\\session-side-panel-redesign",
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    renderChat();

    expect(await screen.findByText("请结合附件一起分析")).toBeInTheDocument();
    expect(await screen.findByAltText("screen.png")).toBeInTheDocument();
    expect(await screen.findByText("debug.ts")).toBeInTheDocument();
    expect(screen.getByText("文本附件")).toBeInTheDocument();
  });

  test("renders task journey summary after transcript instead of before the first message", async () => {
    renderChat();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "查看此任务中的所有文件" })).toBeInTheDocument();
      expect(screen.getByTestId("chat-message-0")).toBeInTheDocument();
      expect(screen.getByText("任务已完成，点击查看本次产出文件")).toBeInTheDocument();
    });

    const message = screen.getByTestId("chat-message-0");
    const summary = screen.getByRole("button", { name: "查看此任务中的所有文件" });

    expect(message.compareDocumentPosition(summary) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  test("shows the files entry card for partial completion", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve(buildPartialJourneyMessages());
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-side-panel-redesign",
            work_dir: "E:\\workspace\\session-side-panel-redesign",
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_workspace_files") {
        return Promise.resolve([
          { path: "partial-report.html", name: "partial-report.html", size: 12 * 1024, kind: "html" },
        ]);
      }
      return Promise.resolve(null);
    });

    renderChat();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "查看此任务中的所有文件" })).toBeInTheDocument();
      expect(screen.getByText("共 1 个文件")).toBeInTheDocument();
    });
  });

  test("does not show the files entry card while work is still running", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve(buildRunningJourneyMessages());
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-side-panel-redesign",
            work_dir: "E:\\workspace\\session-side-panel-redesign",
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    renderChat();

    await waitFor(() => {
      expect(screen.getByTestId("chat-message-0")).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: "查看此任务中的所有文件" })).not.toBeInTheDocument();
  });

  test("does not show the files entry card when the run failed without deliverables", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve(buildFailedJourneyMessages());
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-side-panel-redesign",
            work_dir: "E:\\workspace\\session-side-panel-redesign",
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    renderChat();

    await waitFor(() => {
      expect(screen.getByTestId("chat-message-0")).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: "查看此任务中的所有文件" })).not.toBeInTheDocument();
  });

  test("anchors task journey summary to the assistant message that produced the deliverables", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve(buildSplitJourneyMessages());
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-side-panel-redesign",
            work_dir: "E:\\workspace\\session-side-panel-redesign",
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_workspace_files") {
        return Promise.resolve([
          { path: "round-one-report.html", name: "round-one-report.html", size: 26.6 * 1024, kind: "html" },
        ]);
      }
      if (command === "open_external_url") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    renderChat();

    await waitFor(() => {
      expect(screen.getByTestId("chat-message-0")).toBeInTheDocument();
      expect(screen.getByTestId("chat-message-1")).toBeInTheDocument();
      expect(screen.getByTestId("task-journey-summary-run-a")).toBeInTheDocument();
    });

    const firstAssistant = screen.getByTestId("chat-message-0");
    const secondAssistant = screen.getByTestId("chat-message-1");
    const summary = screen.getByTestId("task-journey-summary-run-a");

    expect(firstAssistant.compareDocumentPosition(summary) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(summary.compareDocumentPosition(secondAssistant) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.queryByTestId("task-journey-summary-run-b")).not.toBeInTheDocument();
  });

  test("uses user-oriented tool island summary and keeps raw tool payload secondary", async () => {
    renderChat();

    await waitFor(() => {
      const summary = screen.getByTestId("tool-island-summary");
      expect(summary).toBeInTheDocument();
      expect(summary).toHaveTextContent("执行记录");
      expect(summary).toHaveTextContent("8 个步骤");
      expect(summary).toHaveTextContent("3 个异常");
    });

    expect(screen.queryByText("已完成 8 个步骤，3 个待处理")).not.toBeInTheDocument();
    expect(screen.queryByText(/"todos"/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("tool-island-summary"));

    await waitFor(() => {
      expect(screen.getByTestId("tool-island-step-todo-1")).toBeInTheDocument();
    });

    expect(screen.queryByText(/"todos"/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("tool-island-step-todo-1"));

    await waitFor(() => {
      expect(screen.getByText(/"todos"/)).toBeInTheDocument();
    });
  });

  test("does not show top task journey summary for an empty session", async () => {
    renderEmptyChat();

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_messages", {
        sessionId: "session-side-panel-redesign",
      });
    });

    expect(screen.queryByRole("button", { name: "查看此任务中的所有文件" })).not.toBeInTheDocument();
  });

  test("keeps employee assistant entry in guidance state instead of task progress state", async () => {
    renderEmptyChat({
      skill: {
        id: "builtin-employee-creator",
        name: "智能体员工助手",
        description: "desc",
        version: "1.0.0",
        author: "test",
        recommended_model: "",
        tags: [],
        created_at: new Date().toISOString(),
      },
      employeeAssistantContext: {
        mode: "create",
      },
      initialMessage: "请帮我创建一个新的智能体员工。先问我 1-2 个关键问题，再给出配置草案，确认后再执行创建。",
    });

    await waitFor(() => {
      expect(screen.getByTestId("chat-employee-assistant-context")).toHaveTextContent("正在创建：新智能体员工");
      expect(screen.getByText("我会先问 1-2 个关键问题，再给出配置草案，确认后执行创建。")).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: "查看此任务中的所有文件" })).not.toBeInTheDocument();
    expect(screen.queryByText("处理中")).not.toBeInTheDocument();
    expect(screen.queryByText("已完成")).not.toBeInTheDocument();
  });

  test("anchors task journey summary to the assistant message that produced the deliverables", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve(buildSplitJourneyMessages());
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-side-panel-redesign",
            work_dir: "E:\\workspace\\session-side-panel-redesign",
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_workspace_files") {
        return Promise.resolve([
          { path: "round-one-report.html", name: "round-one-report.html", size: 26.6 * 1024, kind: "html" },
        ]);
      }
      if (command === "open_external_url") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    renderChat();

    await waitFor(() => {
      expect(screen.getByTestId("chat-message-0")).toBeInTheDocument();
      expect(screen.getByTestId("chat-message-1")).toBeInTheDocument();
      expect(screen.getByTestId("task-journey-summary-run-a")).toBeInTheDocument();
    });

    const firstAssistant = screen.getByTestId("chat-message-0");
    const secondAssistant = screen.getByTestId("chat-message-1");
    const summary = screen.getByTestId("task-journey-summary-run-a");

    expect(firstAssistant.compareDocumentPosition(summary) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(summary.compareDocumentPosition(secondAssistant) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.queryByTestId("task-journey-summary-run-b")).not.toBeInTheDocument();
  });

  test("uses user-oriented tool island summary and keeps raw tool payload secondary", async () => {
    renderChat();

    await waitFor(() => {
      const summary = screen.getByTestId("tool-island-summary");
      expect(summary).toBeInTheDocument();
      expect(summary).toHaveTextContent("执行记录");
      expect(summary).toHaveTextContent("8 个步骤");
      expect(summary).toHaveTextContent("3 个异常");
    });

    expect(screen.queryByText("已完成 8 个步骤，3 个待处理")).not.toBeInTheDocument();
    expect(screen.queryByText(/"todos"/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("tool-island-summary"));

    await waitFor(() => {
      expect(screen.getByTestId("tool-island-step-todo-1")).toBeInTheDocument();
    });

    expect(screen.queryByText(/"todos"/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("tool-island-step-todo-1"));

    await waitFor(() => {
      expect(screen.getByText(/"todos"/)).toBeInTheDocument();
    });
  });
});
