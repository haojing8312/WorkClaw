import type { Message, StreamItem, ToolCallInfo } from "../../types";

export interface TaskItemView {
  id: string;
  title: string;
  status: "pending" | "in_progress" | "completed";
  priority: string;
}

export interface TaskPanelViewModel {
  hasTodoList: boolean;
  totalTasks: number;
  completedTasks: number;
  inProgressTasks: number;
  currentTaskTitle: string;
  items: TaskItemView[];
  touchedFileCount: number;
  webSearchCount: number;
  latestTouchedFile: string;
  latestSearchQuery: string;
}

export interface SessionTouchedFile {
  path: string;
  tool: "write_file" | "edit";
}

export interface WebSearchResultView {
  title: string;
  url: string;
  snippet: string;
  domain: string;
}

export interface WebSearchEntryView {
  id: string;
  query: string;
  status: "running" | "completed" | "error";
  results: WebSearchResultView[];
  rawOutput?: string;
}

export interface TaskJourneyStepView {
  id: string;
  kind: "planning" | "research" | "delivery" | "error";
  title: string;
  detail: string;
  status: "running" | "completed" | "error";
  count: number;
}

export interface DeliverableView {
  path: string;
  tool: "write_file" | "edit";
  category: "primary" | "secondary";
}

export interface TaskJourneyViewModel {
  status: "running" | "completed" | "failed" | "partial";
  currentTaskTitle: string;
  steps: TaskJourneyStepView[];
  deliverables: DeliverableView[];
  warnings: string[];
}

function getToolDisplayLabel(name: ToolCallInfo["name"]): string {
  switch (name) {
    case "write_file":
      return "写入文件";
    case "edit":
      return "编辑文件";
    case "web_search":
      return "资料搜索";
    case "todo_write":
      return "任务清单";
    default:
      return name;
  }
}

function flattenToolCalls(messages: Message[]): ToolCallInfo[] {
  const streamToolCalls = messages.flatMap((message) =>
    (message.streamItems || [])
      .filter((item: StreamItem) => item.type === "tool_call" && item.toolCall)
      .map((item) => item.toolCall!)
  );

  const legacyToolCalls = messages.flatMap((message) => message.toolCalls || []);

  return [...streamToolCalls, ...legacyToolCalls];
}

function normalizeTaskStatus(value: unknown): TaskItemView["status"] {
  if (value === "completed" || value === "in_progress") return value;
  return "pending";
}

function readTouchedPath(tc: ToolCallInfo): string {
  const input = tc.input || {};
  const candidate = input.path || input.file_path;
  return typeof candidate === "string" ? candidate : "";
}

function classifyDeliverable(path: string): DeliverableView["category"] {
  const lowerPath = path.toLowerCase();
  if (
    lowerPath.endsWith(".docx") ||
    lowerPath.endsWith(".doc") ||
    lowerPath.endsWith(".pdf") ||
    lowerPath.endsWith(".html")
  ) {
    return "primary";
  }
  return "secondary";
}

function inferCurrentTaskTitle(toolCalls: ToolCallInfo[]): string {
  const latestWrite = [...toolCalls]
    .reverse()
    .find((tc) => (tc.name === "write_file" || tc.name === "edit") && tc.status === "completed");
  if (latestWrite) {
    return "生成交付文件";
  }
  const latestSearch = [...toolCalls].reverse().find((tc) => tc.name === "web_search");
  if (latestSearch) {
    return "搜索资料";
  }
  return "处理中";
}

function extractDomain(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return "";
  }
}

function parseWebSearchResults(output?: string): WebSearchResultView[] {
  if (!output) return [];

  try {
    const parsed = JSON.parse(output);
    const results = Array.isArray(parsed?.results)
      ? parsed.results
      : Array.isArray(parsed?.items)
      ? parsed.items
      : [];
    return results
      .map((item: any) => ({
        title: String(item?.title || ""),
        url: String(item?.url || item?.link || ""),
        snippet: String(item?.snippet || item?.summary || ""),
        domain: extractDomain(String(item?.url || item?.link || "")),
      }))
      .filter((item: WebSearchResultView) => item.title || item.url);
  } catch {
    return output
      .split(/\n+/)
      .map((line: string) => line.trim())
      .filter(Boolean)
      .reduce<WebSearchResultView[]>((acc, line: string) => {
        const match = line.match(/^(\d+)\.\s+(.*)$/);
        if (match) {
          acc.push({ title: match[2], url: "", snippet: "", domain: "" });
        } else if (acc.length > 0 && !acc[acc.length - 1].url && /^https?:\/\//.test(line)) {
          acc[acc.length - 1].url = line;
          acc[acc.length - 1].domain = extractDomain(line);
        } else if (acc.length > 0 && !acc[acc.length - 1].snippet) {
          acc[acc.length - 1].snippet = line;
        }
        return acc;
      }, []);
  }
}

export function extractSessionTouchedFiles(messages: Message[]): SessionTouchedFile[] {
  return flattenToolCalls(messages)
    .filter((tc) => tc.name === "write_file" || tc.name === "edit")
    .map((tc) => {
      const path = readTouchedPath(tc);
      return path
        ? {
            path,
            tool: tc.name as "write_file" | "edit",
          }
        : null;
    })
    .filter((item): item is SessionTouchedFile => Boolean(item));
}

export function buildTaskPanelViewModel(messages: Message[]): TaskPanelViewModel {
  const toolCalls = flattenToolCalls(messages);
  const todoCalls = toolCalls.filter((tc) => tc.name === "todo_write");
  const latestTodoCall = todoCalls[todoCalls.length - 1];
  const todos = Array.isArray(latestTodoCall?.input?.todos) ? latestTodoCall.input.todos : [];
  const todoItems: TaskItemView[] = todos.map((todo: any, index) => ({
    id: String(todo?.id || `todo-${index}`),
    title: String(todo?.content || "(无标题任务)"),
    status: normalizeTaskStatus(todo?.status),
    priority: String(todo?.priority || "medium"),
  }));
  const fallbackJourney = buildTaskJourneyViewModel(messages);
  const fallbackItems: TaskItemView[] = fallbackJourney.steps
    .filter((step) => step.kind !== "error")
    .map((step, index) => ({
      id: step.id || `journey-${index}`,
      title: step.title,
      status: step.status === "running" ? "in_progress" : "completed",
      priority: "medium",
    }));
  const items = todoItems.length > 0 ? todoItems : fallbackItems;
  const completedTasks = items.filter((item) => item.status === "completed").length;
  const inProgressItems = items.filter((item) => item.status === "in_progress");
  const touchedFiles = extractSessionTouchedFiles(messages);
  const webSearches = buildWebSearchViewModel(messages);

  return {
    hasTodoList: items.length > 0,
    totalTasks: items.length,
    completedTasks,
    inProgressTasks: inProgressItems.length,
    currentTaskTitle:
      inProgressItems[0]?.title ||
      items.find((item) => item.status === "pending")?.title ||
      fallbackJourney.currentTaskTitle ||
      "",
    items,
    touchedFileCount: touchedFiles.length,
    webSearchCount: webSearches.length,
    latestTouchedFile: touchedFiles[touchedFiles.length - 1]?.path || "",
    latestSearchQuery: webSearches[webSearches.length - 1]?.query || "",
  };
}

export function buildWebSearchViewModel(messages: Message[]): WebSearchEntryView[] {
  return flattenToolCalls(messages)
    .filter((tc) => tc.name === "web_search")
    .map((tc, index) => ({
      id: tc.id || `web-search-${index}`,
      query: String(tc.input?.query || ""),
      status: tc.status,
      results: parseWebSearchResults(tc.output),
      rawOutput: tc.output,
    }))
    .filter((entry) => entry.query || entry.results.length > 0);
}

export function buildTaskJourneyViewModel(messages: Message[]): TaskJourneyViewModel {
  const toolCalls = flattenToolCalls(messages);
  const todoCalls = toolCalls.filter((tc) => tc.name === "todo_write");
  const latestTodoCall = todoCalls[todoCalls.length - 1];
  const todos = Array.isArray(latestTodoCall?.input?.todos) ? latestTodoCall.input.todos : [];
  const inProgressTodo = todos.find((todo: any) => todo?.status === "in_progress");

  const deliverables = toolCalls
    .filter((tc) => (tc.name === "write_file" || tc.name === "edit") && tc.status === "completed")
    .map((tc) => {
      const path = readTouchedPath(tc);
      return path
        ? {
            path,
            tool: tc.name as "write_file" | "edit",
            category: classifyDeliverable(path),
          }
        : null;
    })
    .filter((item): item is DeliverableView => Boolean(item));

  const steps: TaskJourneyStepView[] = [];
  const warnings: string[] = [];

  for (let index = 0; index < toolCalls.length; index += 1) {
    const toolCall = toolCalls[index];
    const output = String(toolCall.output || "").trim();

    if (toolCall.status === "error") {
      let count = 1;
      while (index + count < toolCalls.length) {
        const next = toolCalls[index + count];
        if (next.name !== toolCall.name || next.status !== "error" || String(next.output || "").trim() !== output) {
          break;
        }
        count += 1;
      }
      const warning = `${toolCall.name} 失败 ${count} 次：${output || "未知错误"}`;
      const displayName = getToolDisplayLabel(toolCall.name);
      warnings.push(warning);
      steps.push({
        id: toolCall.id,
        kind: "error",
        title: `${displayName}失败，已重试 ${count} 次`,
        detail: warning,
        status: "error",
        count,
      });
      index += count - 1;
      continue;
    }

    if (toolCall.name === "web_search") {
      steps.push({
        id: toolCall.id,
        kind: "research",
        title: "已完成资料搜索",
        detail: String(toolCall.input?.query || ""),
        status: toolCall.status,
        count: 1,
      });
      continue;
    }

    if (toolCall.name === "todo_write") {
      steps.push({
        id: toolCall.id,
        kind: "planning",
        title: "已更新任务清单",
        detail: `${todos.length} 个任务项`,
        status: toolCall.status,
        count: 1,
      });
      continue;
    }

    if (toolCall.name === "write_file" || toolCall.name === "edit") {
      const path = readTouchedPath(toolCall);
      steps.push({
        id: toolCall.id,
        kind: "delivery",
        title: "已生成交付文件",
        detail: path,
        status: toolCall.status,
        count: 1,
      });
    }
  }

  const hasError = warnings.length > 0;
  const hasDeliverable = deliverables.length > 0;
  const hasRunning = toolCalls.some((toolCall) => toolCall.status === "running");

  return {
    status: hasRunning ? "running" : hasError ? (hasDeliverable ? "partial" : "failed") : "completed",
    currentTaskTitle:
      String(inProgressTodo?.content || "") || inferCurrentTaskTitle(toolCalls),
    steps,
    deliverables,
    warnings,
  };
}
