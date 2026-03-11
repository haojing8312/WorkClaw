import { render, screen } from "@testing-library/react";
import { Sidebar } from "../Sidebar";

describe("Sidebar semantic theme", () => {
  test("renders semantic classes for shell, nav button, and form controls", () => {
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

    const logo = screen.getByAltText("WorkClaw Logo");
    expect(logo).toHaveAttribute("src", expect.stringContaining("workclaw-logo"));
    expect(logo).toHaveClass("h-8");
    expect(logo).toHaveClass("w-8");
    expect(screen.queryByText("导航")).not.toBeInTheDocument();
    expect(screen.queryByText("WorkClaw")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "折叠侧边栏" }).closest("div")).toHaveClass("sm-surface");
    expect(screen.getByRole("button", { name: "开始任务" })).toHaveClass("sm-btn");
    expect(screen.getByPlaceholderText("搜索会话...")).toHaveClass("sm-input");
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });
});
