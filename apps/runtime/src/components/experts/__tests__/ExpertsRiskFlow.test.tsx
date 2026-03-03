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

describe("Experts risk flow", () => {
  test("remove skill requires high-risk confirmation before callback", () => {
    const deleteSpy = vi.fn();
    render(
      <ExpertsView
        skills={[buildSkill({ id: "local-file-organizer", name: "文件整理器" })]}
        onInstallSkill={() => {}}
        onCreate={() => {}}
        onOpenPackaging={() => {}}
        onInstallFromLibrary={async () => {}}
        onStartTaskWithSkill={() => {}}
        onRefreshLocalSkill={() => {}}
        onCheckClawhubUpdate={() => {}}
        onUpdateClawhubSkill={() => {}}
        onDeleteSkill={deleteSpy}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "移除" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(deleteSpy).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "移除" }));
    fireEvent.click(screen.getByRole("button", { name: "确认移除" }));
    expect(deleteSpy).toHaveBeenCalledWith("local-file-organizer");
    expect(deleteSpy).toHaveBeenCalledTimes(1);
  });
});
