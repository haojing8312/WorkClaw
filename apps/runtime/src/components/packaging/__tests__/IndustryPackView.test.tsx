import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { IndustryPackView } from "../IndustryPackView";

const invokeMock = vi.fn();
const openMock = vi.fn();
const saveMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openMock(...args),
  save: (...args: unknown[]) => saveMock(...args),
}));

describe("IndustryPackView", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();
    saveMock.mockReset();
  });

  test("defaults pack metadata from selected root and selects all skills", async () => {
    openMock.mockResolvedValueOnce("C:\\workspace\\teacher-suite");
    invokeMock.mockImplementation((command: string) => {
      if (command === "scan_workclaw_dirs") {
        return Promise.resolve([
          {
            dir_path: "C:\\workspace\\teacher-suite\\teacher-helper",
            slug: "teacher-helper",
            front_matter: { name: "Teacher Helper", description: "desc", version: "1.0.0" },
            tags: ["教师", "备课"],
          },
          {
            dir_path: "C:\\workspace\\teacher-suite\\grader",
            slug: "grader",
            front_matter: { name: "Auto Grader", description: "desc", version: "1.0.0" },
            tags: ["教师", "作业"],
          },
        ]);
      }
      return Promise.resolve(null);
    });

    render(<IndustryPackView />);

    fireEvent.click(screen.getByRole("button", { name: "选择技能根目录" }));

    await waitFor(() => {
      expect(screen.getByText("Teacher Helper")).toBeInTheDocument();
      expect(screen.getByText("Auto Grader")).toBeInTheDocument();
    });

    expect(screen.getByLabelText("行业包名称")).toHaveValue("teacher-suite");
    expect(screen.getByLabelText("包 ID")).toHaveValue("teacher-suite");

    expect(screen.getByLabelText("选择 Teacher Helper")).toBeChecked();
    expect(screen.getByLabelText("选择 Auto Grader")).toBeChecked();
    expect(screen.getByText("已选 2 / 2")).toBeInTheDocument();
  });

  test("supports select all toggles and packs selected skills", async () => {
    openMock.mockResolvedValueOnce("C:\\skills");
    saveMock.mockResolvedValueOnce("C:\\packs\\teacher-suite.industrypack");
    invokeMock.mockImplementation((command: string) => {
      if (command === "scan_workclaw_dirs") {
        return Promise.resolve([
          {
            dir_path: "C:\\skills\\teacher-helper",
            slug: "teacher-helper",
            front_matter: { name: "Teacher Helper", description: "desc", version: "1.0.0" },
            tags: ["教师", "备课"],
          },
          {
            dir_path: "C:\\skills\\grader",
            slug: "grader",
            front_matter: { name: "Auto Grader", description: "desc", version: "1.0.0" },
            tags: ["教师", "作业"],
          },
        ]);
      }
      if (command === "pack_industry_bundle") {
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });

    render(<IndustryPackView />);

    fireEvent.click(screen.getByRole("button", { name: "选择技能根目录" }));

    await waitFor(() => {
      expect(screen.getByText("Teacher Helper")).toBeInTheDocument();
      expect(screen.getByText("Auto Grader")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText("行业包名称"), {
      target: { value: "教师行业包" },
    });
    fireEvent.change(screen.getByLabelText("包 ID"), {
      target: { value: "edu-teacher-suite" },
    });
    fireEvent.change(screen.getByLabelText("版本"), {
      target: { value: "1.2.0" },
    });
    fireEvent.change(screen.getByLabelText("行业标签"), {
      target: { value: "教师" },
    });

    fireEvent.click(screen.getByRole("button", { name: "全不选" }));
    expect(screen.getByLabelText("选择 Teacher Helper")).not.toBeChecked();
    expect(screen.getByLabelText("选择 Auto Grader")).not.toBeChecked();

    fireEvent.click(screen.getByRole("button", { name: "全选" }));
    expect(screen.getByLabelText("选择 Teacher Helper")).toBeChecked();
    expect(screen.getByLabelText("选择 Auto Grader")).toBeChecked();

    fireEvent.click(screen.getByRole("button", { name: "导出行业包" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("pack_industry_bundle", {
        skillDirs: ["C:\\skills\\teacher-helper", "C:\\skills\\grader"],
        packName: "教师行业包",
        packId: "edu-teacher-suite",
        version: "1.2.0",
        industryTag: "教师",
        outputPath: "C:\\packs\\teacher-suite.industrypack",
      });
      expect(screen.getByText("行业包导出成功")).toBeInTheDocument();
    });
  });
});
