import { fireEvent, render, screen } from "@testing-library/react";
import { TaskTabStrip } from "../TaskTabStrip";

describe("TaskTabStrip", () => {
  test("renders tabs, highlights the active tab, and forwards interactions", () => {
    const onSelectTab = vi.fn();
    const onCreateTab = vi.fn();
    const onCloseTab = vi.fn();

    render(
      <TaskTabStrip
        tabs={[
          { id: "tab-start", kind: "start-task", title: "开始任务" },
          { id: "tab-session", kind: "session", title: "Session 1", runtimeStatus: "running" },
        ]}
        activeTabId="tab-session"
        onSelectTab={onSelectTab}
        onCreateTab={onCreateTab}
        onCloseTab={onCloseTab}
      />,
    );

    expect(screen.getByRole("tab", { name: "开始任务" })).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("tab", { name: "Session 1" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tablist", { name: "任务标签" })).toHaveClass("gap-1");

    fireEvent.click(screen.getByRole("tab", { name: "开始任务" }));
    expect(onSelectTab).toHaveBeenCalledWith("tab-start");

    fireEvent.click(screen.getByRole("button", { name: "新建任务标签" }));
    expect(onCreateTab).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "关闭标签 Session 1" }));
    expect(onCloseTab).toHaveBeenCalledWith("tab-session");
  });
});
