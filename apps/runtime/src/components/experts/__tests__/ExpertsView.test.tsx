import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
  afterEach(() => {
    cleanup();
  });

  test("hides builtin general skill and shows source-specific actions", () => {
    const refreshSpy = vi.fn();
    const deleteSpy = vi.fn();
    const startTaskSpy = vi.fn();
    render(
      <ExpertsView
        skills={[
          buildSkill({ id: "builtin-general", name: "通用助手" }),
          buildSkill({ id: "builtin-skill-creator", name: "创建技能" }),
          buildSkill({ id: "local-file-organizer", name: "文件整理器" }),
          buildSkill({ id: "encrypted-abc", name: "外部技能" }),
        ]}
        onInstallSkill={() => {}}
        onCreate={() => {}}
        onOpenPackaging={() => {}}
        onInstallFromLibrary={async () => {}}
        onStartTaskWithSkill={startTaskSpy}
        onRefreshLocalSkill={refreshSpy}
        onCheckClawhubUpdate={() => {}}
        onUpdateClawhubSkill={() => {}}
        onDeleteSkill={deleteSpy}
      />
    );

    expect(screen.queryByText("通用助手")).not.toBeInTheDocument();
    expect(screen.queryByText("创建技能")).toBeInTheDocument();
    expect(screen.queryByText("文件整理器")).toBeInTheDocument();
    expect(screen.queryByText("外部技能")).toBeInTheDocument();

    const startTaskButtons = screen.getAllByRole("button", { name: "开始任务" });
    const refreshButtons = screen.getAllByRole("button", { name: "刷新" });
    const deleteButtons = screen.getAllByRole("button", { name: "移除" });
    expect(startTaskButtons.length).toBe(3);
    expect(refreshButtons.length).toBe(1);
    expect(deleteButtons.length).toBe(2);
    expect(screen.getByText("内置")).toBeInTheDocument();
    expect(screen.getByText("本地")).toBeInTheDocument();
    expect(screen.getByText("已安装")).toBeInTheDocument();
    expect(screen.getByText("系统预置，不支持移除")).toBeInTheDocument();
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
        onInstallFromLibrary={async () => {}}
        onStartTaskWithSkill={startTaskSpy}
        onRefreshLocalSkill={refreshSpy}
        onCheckClawhubUpdate={() => {}}
        onUpdateClawhubSkill={() => {}}
        onDeleteSkill={deleteSpy}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "开始任务" }));
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    fireEvent.click(screen.getByRole("button", { name: "移除" }));
    expect(deleteSpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "确认移除" }));

    expect(startTaskSpy).toHaveBeenCalledWith("local-file-organizer");
    expect(refreshSpy).toHaveBeenCalledWith("local-file-organizer");
    expect(deleteSpy).toHaveBeenCalledWith("local-file-organizer");
  });

  test("shows launch errors inline when a skill session cannot be created", () => {
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
        onDeleteSkill={() => {}}
        launchError="创建会话失败，请稍后重试"
      />
    );

    expect(screen.getByText("创建会话失败，请稍后重试")).toBeInTheDocument();
  });
});
