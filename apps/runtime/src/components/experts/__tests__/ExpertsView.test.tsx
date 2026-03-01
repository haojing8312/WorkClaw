import { fireEvent, render, screen } from "@testing-library/react";
import { ExpertsView } from "../ExpertsView";
import { SkillManifest } from "../../../types";

const now = new Date().toISOString();

function buildSkill(partial: Partial<SkillManifest>): SkillManifest {
  return {
    id: partial.id ?? "skill-id",
    name: partial.name ?? "skill-name",
    description: partial.description ?? "skill-desc",
    version: partial.version ?? "1.0.0",
    author: partial.author ?? "",
    recommended_model: partial.recommended_model ?? "",
    tags: partial.tags ?? [],
    created_at: partial.created_at ?? now,
  };
}

describe("ExpertsView", () => {
  test("hides builtin skill and shows actions by source type", () => {
    const refreshSpy = vi.fn();
    const deleteSpy = vi.fn();
    const startTaskSpy = vi.fn();
    render(
      <ExpertsView
        skills={[
          buildSkill({ id: "builtin-general", name: "通用助手" }),
          buildSkill({ id: "local-file-organizer", name: "文件整理器" }),
          buildSkill({ id: "encrypted-abc", name: "外部技能" }),
        ]}
        onInstallSkill={() => {}}
        onCreate={() => {}}
        onOpenPackaging={() => {}}
        onStartTaskWithSkill={startTaskSpy}
        onRefreshLocalSkill={refreshSpy}
        onDeleteSkill={deleteSpy}
      />
    );

    expect(screen.queryByText("通用助手")).not.toBeInTheDocument();
    expect(screen.queryByText("文件整理器")).toBeInTheDocument();
    expect(screen.queryByText("外部技能")).toBeInTheDocument();

    const startTaskButtons = screen.getAllByRole("button", { name: "开始任务" });
    const refreshButtons = screen.getAllByRole("button", { name: "刷新" });
    const deleteButtons = screen.getAllByRole("button", { name: "移除" });
    expect(startTaskButtons.length).toBe(2);
    expect(refreshButtons.length).toBe(1);
    expect(deleteButtons.length).toBe(2);
  });

  test("triggers start-task/refresh/delete callbacks", () => {
    const refreshSpy = vi.fn();
    const deleteSpy = vi.fn();
    const startTaskSpy = vi.fn();
    render(
      <ExpertsView
        skills={[buildSkill({ id: "local-file-organizer", name: "文件整理器" })]}
        onInstallSkill={() => {}}
        onCreate={() => {}}
        onOpenPackaging={() => {}}
        onStartTaskWithSkill={startTaskSpy}
        onRefreshLocalSkill={refreshSpy}
        onDeleteSkill={deleteSpy}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "开始任务" }));
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    fireEvent.click(screen.getByRole("button", { name: "移除" }));

    expect(startTaskSpy).toHaveBeenCalledWith("local-file-organizer");
    expect(refreshSpy).toHaveBeenCalledWith("local-file-organizer");
    expect(deleteSpy).toHaveBeenCalledWith("local-file-organizer");
  });
});
