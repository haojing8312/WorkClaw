import { render, screen } from "@testing-library/react";
import { Sidebar } from "../Sidebar";

describe("Sidebar runtime status", () => {
  test("renders runtime status labels for active and finished sessions", () => {
    render(
      <Sidebar
        activeMainView="start-task"
        onOpenStartTask={() => {}}
        onOpenExperts={() => {}}
        onOpenEmployees={() => {}}
        selectedSkillId="builtin-general"
        sessions={[
          {
            id: "session-running",
            title: "运行会话",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            runtime_status: "running",
          },
          {
            id: "session-waiting",
            title: "审批会话",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            runtime_status: "waiting_approval",
          },
          {
            id: "session-completed",
            title: "完成会话",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            runtime_status: "completed",
          },
          {
            id: "session-failed",
            title: "失败会话",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            runtime_status: "failed",
          },
          {
            id: "session-idle",
            title: "空闲会话",
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

    expect(screen.getByTestId("session-runtime-status-session-running")).toHaveAttribute("title", "执行中");
    expect(screen.getByTestId("session-runtime-status-session-waiting")).toHaveAttribute("title", "等待确认");
    expect(screen.getByTestId("session-runtime-status-session-completed")).toHaveAttribute("title", "已完成");
    expect(screen.getByTestId("session-runtime-status-session-failed")).toHaveAttribute("title", "执行失败");
    expect(screen.queryByTestId("session-runtime-status-session-idle")).not.toBeInTheDocument();
  });
});
