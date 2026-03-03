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
        newSessionPermissionMode="accept_edits"
        onChangeNewSessionPermissionMode={() => {}}
        onDeleteSession={() => {}}
        onSettings={() => {}}
        onSearchSessions={() => {}}
        onExportSession={() => {}}
        onCollapse={() => {}}
        collapsed={false}
      />
    );

    const title = screen.getByText("SkillMint");
    expect(title.closest("div")).toHaveClass("sm-surface");
    expect(screen.getByRole("button", { name: "开始任务" })).toHaveClass("sm-btn");
    expect(screen.getByPlaceholderText("搜索会话...")).toHaveClass("sm-input");
    expect(screen.getByRole("combobox")).toHaveClass("sm-select");
  });
});
