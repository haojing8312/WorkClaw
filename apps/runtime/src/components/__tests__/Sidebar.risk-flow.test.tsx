import { render, screen } from "@testing-library/react";
import { Sidebar } from "../Sidebar";

describe("Sidebar risk flow", () => {
  test("does not expose permission mode controls in sidebar", () => {
    render(
      <Sidebar
        activeMainView="start-task"
        onOpenStartTask={() => {}}
        onOpenExperts={() => {}}
        onOpenEmployees={() => {}}
        selectedSkillId="builtin-general"
        sessions={[]}
        selectedSessionId={null}
        onSelectSession={() => {}}
        onDeleteSession={() => {}}
        onSettings={() => {}}
        onSearchSessions={() => {}}
        onExportSession={() => {}}
        onCollapse={() => {}}
        collapsed={false}
      />
    );

    expect(screen.queryByText("操作确认级别")).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    expect(screen.queryByText("切换为全自动模式")).not.toBeInTheDocument();
  });
});
