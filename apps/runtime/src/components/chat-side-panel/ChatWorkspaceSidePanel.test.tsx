import { fireEvent, render, screen } from "@testing-library/react";
import { ChatWorkspaceSidePanel } from "./ChatWorkspaceSidePanel";
import type { TaskPanelViewModel, WebSearchEntryView } from "./view-model";

const taskModel: TaskPanelViewModel = {
  hasTodoList: true,
  totalTasks: 2,
  completedTasks: 1,
  inProgressTasks: 1,
  currentTaskTitle: "创建带动画和时间轴的HTML报告",
  items: [
    { id: "t1", title: "创建美国以色列伊朗冲突Word简报", status: "completed", priority: "high" },
    { id: "t2", title: "创建带动画和时间轴的HTML报告", status: "in_progress", priority: "high" },
  ],
  touchedFileCount: 2,
  webSearchCount: 1,
  latestTouchedFile: "conflict_report.html",
  latestSearchQuery: "US military presence Middle East 2025",
};

const webSearchEntries: WebSearchEntryView[] = [
  {
    id: "search-1",
    query: "US military presence Middle East 2025",
    status: "completed",
    results: [],
  },
];

describe("ChatWorkspaceSidePanel", () => {
  test("switches redesigned tabs, supports resize, and resets width on reopen", () => {
    const onClose = vi.fn();
    const onTabChange = vi.fn();
    const { rerender } = render(
      <ChatWorkspaceSidePanel
        open
        tab="tasks"
        onTabChange={onTabChange}
        onClose={onClose}
        workspace="E:\\workspace\\session-side-panel-redesign"
        touchedFiles={["conflict_report.html"]}
        active
        taskModel={taskModel}
        webSearchEntries={webSearchEntries}
      />,
    );

    expect(screen.getByRole("button", { name: "当前任务" })).toHaveClass("bg-blue-100");
    expect(screen.getByTestId("chat-workspace-drawer")).toHaveStyle({ width: "760px" });

    fireEvent.click(screen.getByRole("button", { name: "文件" }));
    expect(onTabChange).toHaveBeenCalledWith("files");

    fireEvent.click(screen.getByRole("button", { name: "Web 搜索" }));
    expect(onTabChange).toHaveBeenCalledWith("websearch");

    fireEvent.click(screen.getByRole("button", { name: "关闭面板" }));
    expect(onClose).toHaveBeenCalled();

    fireEvent.mouseDown(screen.getByTestId("chat-workspace-drawer-resize-handle"));
    fireEvent.mouseMove(window, { clientX: 300 });
    expect(screen.getByTestId("chat-workspace-drawer")).toHaveStyle({ width: "724px" });

    fireEvent.mouseMove(window, { clientX: 950 });
    expect(screen.getByTestId("chat-workspace-drawer")).toHaveStyle({ width: "420px" });

    fireEvent.mouseMove(window, { clientX: -400 });
    expect(screen.getByTestId("chat-workspace-drawer")).toHaveStyle({ width: "1100px" });
    fireEvent.mouseUp(window);

    rerender(
      <ChatWorkspaceSidePanel
        open={false}
        tab="tasks"
        onTabChange={onTabChange}
        onClose={onClose}
        workspace="E:\\workspace\\session-side-panel-redesign"
        touchedFiles={["conflict_report.html"]}
        active={false}
        taskModel={taskModel}
        webSearchEntries={webSearchEntries}
      />,
    );

    rerender(
      <ChatWorkspaceSidePanel
        open
        tab="tasks"
        onTabChange={onTabChange}
        onClose={onClose}
        workspace="E:\\workspace\\session-side-panel-redesign"
        touchedFiles={["conflict_report.html"]}
        active
        taskModel={taskModel}
        webSearchEntries={webSearchEntries}
      />,
    );

    expect(screen.getByTestId("chat-workspace-drawer")).toHaveStyle({ width: "760px" });
  });

  test("starts resizing from pointer down so the handle works in the desktop webview", () => {
    render(
      <ChatWorkspaceSidePanel
        open
        tab="tasks"
        onTabChange={() => {}}
        onClose={() => {}}
        workspace="E:\\workspace\\session-side-panel-redesign"
        touchedFiles={["conflict_report.html"]}
        active
        taskModel={taskModel}
        webSearchEntries={webSearchEntries}
      />,
    );

    fireEvent.pointerDown(screen.getByTestId("chat-workspace-drawer-resize-handle"), {
      pointerId: 7,
      clientX: 760,
    });
    fireEvent.mouseMove(window, { clientX: 240 });

    expect(screen.getByTestId("chat-workspace-drawer")).toHaveStyle({ width: "784px" });
  });
});
