import { cleanup, render } from "@testing-library/react";
import { Sidebar } from "../Sidebar";

describe("Sidebar selected session scroll", () => {
  afterEach(() => {
    cleanup();
  });

  test("scrolls the selected session into view when selection changes", () => {
    const scrollIntoView = vi.fn();
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: scrollIntoView,
    });

    const sessions = [
      {
        id: "session-1",
        title: "整理本周周报",
        created_at: new Date().toISOString(),
        model_id: "model-a",
      },
      {
        id: "session-2",
        title: "工部执行会话",
        created_at: new Date().toISOString(),
        model_id: "model-a",
      },
    ];

    const { rerender } = render(
      <Sidebar
        activeMainView="start-task"
        onOpenStartTask={() => {}}
        onOpenExperts={() => {}}
        onOpenEmployees={() => {}}
        selectedSkillId="builtin-general"
        sessions={sessions}
        selectedSessionId={null}
        onSelectSession={() => {}}
        onDeleteSession={() => {}}
        onSettings={() => {}}
        onSearchSessions={() => {}}
        onExportSession={() => {}}
        onCollapse={() => {}}
        collapsed={false}
      />,
    );

    scrollIntoView.mockClear();

    rerender(
      <Sidebar
        activeMainView="start-task"
        onOpenStartTask={() => {}}
        onOpenExperts={() => {}}
        onOpenEmployees={() => {}}
        selectedSkillId="builtin-general"
        sessions={sessions}
        selectedSessionId="session-2"
        onSelectSession={() => {}}
        onDeleteSession={() => {}}
        onSettings={() => {}}
        onSearchSessions={() => {}}
        onExportSession={() => {}}
        onCollapse={() => {}}
        collapsed={false}
      />,
    );

    expect(scrollIntoView).toHaveBeenCalledWith({ block: "nearest", inline: "nearest" });
  });
});
