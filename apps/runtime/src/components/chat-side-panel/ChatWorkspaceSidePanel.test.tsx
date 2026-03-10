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
  test("switches redesigned tabs and closes panel", () => {
    const onClose = vi.fn();
    const onTabChange = vi.fn();

    render(
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
      />
    );

    expect(screen.getByRole("button", { name: "当前任务" })).toHaveClass("bg-blue-100");

    fireEvent.click(screen.getByRole("button", { name: "文件" }));
    expect(onTabChange).toHaveBeenCalledWith("files");

    fireEvent.click(screen.getByRole("button", { name: "Web 搜索" }));
    expect(onTabChange).toHaveBeenCalledWith("websearch");

    fireEvent.click(screen.getByRole("button", { name: "关闭面板" }));
    expect(onClose).toHaveBeenCalled();
  });
});
