import { fireEvent, render, screen } from "@testing-library/react";
import { Sidebar } from "../Sidebar";

describe("Sidebar display titles", () => {
  test("prefers display_title over title when rendering sessions", () => {
    render(
      <Sidebar
        activeMainView="start-task"
        onOpenStartTask={() => {}}
        onOpenExperts={() => {}}
        onOpenEmployees={() => {}}
        selectedSkillId="builtin-general"
        sessions={[
          {
            id: "session-1",
            title: "New Chat",
            display_title: "修复登录接口超时",
            created_at: new Date().toISOString(),
            model_id: "model-a",
          },
        ]}
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

    expect(screen.getByText("修复登录接口超时")).toBeInTheDocument();
    expect(screen.queryByText("New Chat")).not.toBeInTheDocument();
  });

  test("still falls back to title when display_title is absent", () => {
    render(
      <Sidebar
        activeMainView="start-task"
        onOpenStartTask={() => {}}
        onOpenExperts={() => {}}
        onOpenEmployees={() => {}}
        selectedSkillId="builtin-general"
        sessions={[
          {
            id: "session-1",
            title: "整理本周周报",
            created_at: new Date().toISOString(),
            model_id: "model-a",
          },
        ]}
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

    expect(screen.getByText("整理本周周报")).toBeInTheDocument();
  });
});
