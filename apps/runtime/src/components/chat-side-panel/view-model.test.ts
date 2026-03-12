import {
  buildTaskJourneyViewModel,
  buildTaskPanelViewModel,
  buildWebSearchViewModel,
  extractSessionTouchedFiles,
} from "./view-model";
import type { Message } from "../../types";

function buildMessages(): Message[] {
  return [
    {
      role: "assistant",
      content: "",
      created_at: new Date().toISOString(),
      streamItems: [
        {
          type: "tool_call",
          toolCall: {
            id: "todo-1",
            name: "todo_write",
            input: {
              todos: [
                { id: "a", content: "task a", status: "completed", priority: "high" },
                { id: "b", content: "task b", status: "in_progress", priority: "medium" },
                { id: "c", content: "task c", status: "pending", priority: "low" },
              ],
            },
            status: "completed",
            output: "ok",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "write-1",
            name: "write_file",
            input: {
              path: "report.html",
            },
            status: "completed",
            output: "ok",
          },
        },
        {
          type: "tool_call",
          toolCall: {
            id: "search-1",
            name: "web_search",
            input: {
              query: "middle east 2025",
            },
            status: "completed",
            output: JSON.stringify({
              results: [
                {
                  title: "Result A",
                  url: "https://example.com/a",
                  snippet: "Snippet A",
                },
              ],
            }),
          },
        },
      ],
    },
  ];
}

describe("chat side panel view-model", () => {
  test("builds task summary from latest todo_write input", () => {
    const model = buildTaskPanelViewModel(buildMessages());

    expect(model.hasTodoList).toBe(true);
    expect(model.totalTasks).toBe(3);
    expect(model.completedTasks).toBe(1);
    expect(model.inProgressTasks).toBe(1);
    expect(model.currentTaskTitle).toBe("task b");
    expect(model.touchedFileCount).toBe(1);
    expect(model.webSearchCount).toBe(1);
    expect(model.latestTouchedFile).toBe("report.html");
  });

  test("extracts session touched files from write_file and edit", () => {
    const messages = buildMessages().concat([
      {
        role: "assistant",
        content: "",
        created_at: new Date().toISOString(),
        toolCalls: [
          {
            id: "edit-1",
            name: "edit",
            input: {
              file_path: "report.md",
            },
            status: "completed",
          },
        ],
      },
    ]);

    expect(extractSessionTouchedFiles(messages)).toEqual([
      { path: "report.html", tool: "write_file" },
      { path: "report.md", tool: "edit" },
    ]);
  });

  test("parses web search results from json output", () => {
    const entries = buildWebSearchViewModel(buildMessages());

    expect(entries).toHaveLength(1);
    expect(entries[0].query).toBe("middle east 2025");
    expect(entries[0].results[0]).toMatchObject({
      title: "Result A",
      url: "https://example.com/a",
      domain: "example.com",
    });
  });

  test("handles malformed search output safely", () => {
    const entries = buildWebSearchViewModel([
      {
        role: "assistant",
        content: "",
        created_at: new Date().toISOString(),
        toolCalls: [
          {
            id: "search-2",
            name: "web_search",
            input: {
              query: "fallback query",
            },
            status: "completed",
            output: "[搜索结果 - 来自 Mock]\n\n1. Result B\n   https://example.com/b\n   Snippet B",
          },
        ],
      },
    ]);

    expect(entries).toHaveLength(1);
    expect(entries[0].results[0]).toMatchObject({
      title: "Result B",
      url: "https://example.com/b",
      snippet: "Snippet B",
    });
  });

  test("derives current task and deliverables without todo_write", () => {
    const model = buildTaskJourneyViewModel([
      {
        role: "assistant",
        content: "正在整理交付。",
        created_at: new Date().toISOString(),
        streamItems: [
          {
            type: "tool_call",
            toolCall: {
              id: "search-1",
              name: "web_search",
              input: { query: "latest conflict timeline" },
              status: "completed",
              output: "ok",
            },
          },
          {
            type: "tool_call",
            toolCall: {
              id: "write-1",
              name: "write_file",
              input: { path: "conflict_report.html" },
              status: "completed",
              output: "成功写入 1024 字节到 conflict_report.html",
            },
          },
        ],
      },
    ]);

    expect(model.currentTaskTitle).toBe("生成交付文件");
    expect(model.status).toBe("completed");
    expect(model.deliverables).toEqual([
      expect.objectContaining({
        path: "conflict_report.html",
        category: "primary",
      }),
    ]);
  });

  test("groups repeated adjacent tool failures into one warning step", () => {
    const model = buildTaskJourneyViewModel([
      {
        role: "assistant",
        content: "",
        created_at: new Date().toISOString(),
        streamItems: [
          {
            type: "tool_call",
            toolCall: {
              id: "write-1",
              name: "write_file",
              input: {},
              status: "error",
              output: "工具执行错误: 缺少 path 参数",
            },
          },
          {
            type: "tool_call",
            toolCall: {
              id: "write-2",
              name: "write_file",
              input: {},
              status: "error",
              output: "工具执行错误: 缺少 path 参数",
            },
          },
          {
            type: "tool_call",
            toolCall: {
              id: "write-3",
              name: "write_file",
              input: {},
              status: "error",
              output: "工具执行错误: 缺少 path 参数",
            },
          },
        ],
      },
    ]);

    expect(model.status).toBe("failed");
    expect(model.warnings).toContain("write_file 失败 3 次：工具执行错误: 缺少 path 参数");
    expect(model.steps).toEqual([
      expect.objectContaining({
        kind: "error",
        count: 3,
      }),
    ]);
  });

  test("keeps empty journey state free of default processing labels", () => {
    const model = buildTaskJourneyViewModel([]);

    expect(model.currentTaskTitle).toBe("");
    expect(model.steps).toEqual([]);
    expect(model.deliverables).toEqual([]);
    expect(model.warnings).toEqual([]);
  });

  test("builds fallback task panel summary from tool journey when todo_write is missing", () => {
    const model = buildTaskPanelViewModel([
      {
        role: "assistant",
        content: "",
        created_at: new Date().toISOString(),
        streamItems: [
          {
            type: "tool_call",
            toolCall: {
              id: "search-1",
              name: "web_search",
              input: { query: "latest conflict updates" },
              status: "completed",
              output: "ok",
            },
          },
          {
            type: "tool_call",
            toolCall: {
              id: "write-1",
              name: "write_file",
              input: { path: "brief.html" },
              status: "completed",
              output: "ok",
            },
          },
        ],
      },
    ]);

    expect(model.hasTodoList).toBe(false);
    expect(model.totalTasks).toBe(0);
    expect(model.completedTasks).toBe(0);
    expect(model.inProgressTasks).toBe(0);
    expect(model.currentTaskTitle).toBe("");
    expect(model.items).toEqual([]);
  });

  test("prefers the latest running tool as current execution item while keeping todo counts", () => {
    const model = buildTaskPanelViewModel([
      {
        role: "assistant",
        content: "",
        created_at: new Date().toISOString(),
        streamItems: [
          {
            type: "tool_call",
            toolCall: {
              id: "todo-1",
              name: "todo_write",
              input: {
                todos: [
                  { id: "a", content: "搜索资料", status: "completed", priority: "high" },
                  { id: "b", content: "安装 ClawHub", status: "completed", priority: "high" },
                ],
              },
              status: "completed",
              output: "ok",
            },
          },
          {
            type: "tool_call",
            toolCall: {
              id: "bash-1",
              name: "bash",
              input: {
                command: "npm cache clean --force && npm install -g n --force",
              },
              status: "running",
              output: "",
            },
          },
        ],
      },
    ]);

    expect(model.hasTodoList).toBe(true);
    expect(model.totalTasks).toBe(2);
    expect(model.completedTasks).toBe(2);
    expect(model.inProgressTasks).toBe(0);
    expect(model.currentTaskTitle).toContain("执行命令");
    expect(model.currentTaskTitle).toContain("npm cache clean");
  });
});
