import { render, screen } from "@testing-library/react";
import { Sidebar } from "../Sidebar";

describe("Sidebar session source badge", () => {
  function renderSidebar() {
    return render(
      <Sidebar
        activeMainView="start-task"
        onOpenStartTask={() => {}}
        onOpenExperts={() => {}}
        onOpenEmployees={() => {}}
        selectedSkillId="builtin-general"
        sessions={[
          {
            id: "s-feishu",
            title: "飞书需求讨论",
            created_at: "2026-03-05T00:00:00Z",
            model_id: "m1",
            source_channel: "feishu",
            source_label: "飞书",
          },
          {
            id: "s-wecom",
            title: "企业微信项目同步",
            created_at: "2026-03-05T00:00:00Z",
            model_id: "m1",
            source_channel: "wecom",
            source_label: "企业微信",
          },
          {
            id: "s-local",
            title: "本地任务",
            created_at: "2026-03-05T00:00:00Z",
            model_id: "m1",
            source_channel: "local",
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
  }

  test("shows source badges for IM-synced sessions only", () => {
    renderSidebar();
    expect(screen.getAllByText("飞书")).toHaveLength(1);
    expect(screen.getAllByText("企业微信")).toHaveLength(1);
    expect(screen.getByText("飞书需求讨论")).toBeInTheDocument();
    expect(screen.getByText("企业微信项目同步")).toBeInTheDocument();
    expect(screen.getByText("本地任务")).toBeInTheDocument();
  });
});
