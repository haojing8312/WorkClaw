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
  test("shows refresh/delete actions by skill source type", () => {
    const refreshSpy = vi.fn();
    const deleteSpy = vi.fn();
    render(
      <ExpertsView
        skills={[
          buildSkill({ id: "builtin-general", name: "通用助手" }),
          buildSkill({ id: "local-file-organizer", name: "文件整理器" }),
          buildSkill({ id: "encrypted-abc", name: "外部技能" }),
        ]}
        onCreate={() => {}}
        onOpenPackaging={() => {}}
        onRefreshLocalSkill={refreshSpy}
        onDeleteSkill={deleteSpy}
      />
    );

    expect(screen.queryByText("通用助手")).toBeInTheDocument();
    expect(screen.queryByText("文件整理器")).toBeInTheDocument();
    expect(screen.queryByText("外部技能")).toBeInTheDocument();

    const refreshButtons = screen.getAllByRole("button", { name: "刷新" });
    const deleteButtons = screen.getAllByRole("button", { name: "移除" });
    expect(refreshButtons.length).toBe(1);
    expect(deleteButtons.length).toBe(2);
  });

  test("triggers refresh/delete callbacks", () => {
    const refreshSpy = vi.fn();
    const deleteSpy = vi.fn();
    render(
      <ExpertsView
        skills={[buildSkill({ id: "local-file-organizer", name: "文件整理器" })]}
        onCreate={() => {}}
        onOpenPackaging={() => {}}
        onRefreshLocalSkill={refreshSpy}
        onDeleteSkill={deleteSpy}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    fireEvent.click(screen.getByRole("button", { name: "移除" }));

    expect(refreshSpy).toHaveBeenCalledWith("local-file-organizer");
    expect(deleteSpy).toHaveBeenCalledWith("local-file-organizer");
  });
});
