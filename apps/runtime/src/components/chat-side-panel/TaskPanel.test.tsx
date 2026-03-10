import { render, screen } from "@testing-library/react";
import { TaskPanel } from "./TaskPanel";
import type { TaskPanelViewModel } from "./view-model";

describe("TaskPanel", () => {
  test("shows current step even when todo list is missing", () => {
    const model: TaskPanelViewModel = {
      hasTodoList: false,
      totalTasks: 0,
      completedTasks: 0,
      inProgressTasks: 0,
      currentTaskTitle: "执行命令：npm install -g openclaw",
      items: [],
      touchedFileCount: 0,
      webSearchCount: 4,
      latestTouchedFile: "",
      latestSearchQuery: "using-superpowers github claw-hub 安装",
    };

    render(<TaskPanel model={model} />);

    expect(screen.getByText("当前步骤")).toBeInTheDocument();
    expect(screen.getByText("执行命令：npm install -g openclaw")).toBeInTheDocument();
    expect(screen.getByText("未创建任务清单")).toBeInTheDocument();
    expect(screen.getByText("暂无任务项")).toBeInTheDocument();
  });
});
