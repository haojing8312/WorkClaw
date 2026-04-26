import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { WorkspaceFilesPanel } from "./WorkspaceFilesPanel";

const invokeMock = vi.fn();
const writeTextMock = vi.fn(() => Promise.resolve());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("WorkspaceFilesPanel", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    invokeMock.mockReset();
    writeTextMock.mockClear();
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: writeTextMock,
      },
    });

    invokeMock.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "list_workspace_files") {
        return Promise.resolve([
          { path: ".minimax", name: ".minimax", size: 0, kind: "directory" },
          { path: "reports", name: "reports", size: 0, kind: "directory" },
          {
            path: "reports/conflict_report_with_appendix_and_references.html",
            name: "conflict_report_with_appendix_and_references.html",
            size: 26 * 1024,
            kind: "html",
          },
          {
            path: "reports/conflict_brief_with_policy_recommendations.md",
            name: "conflict_brief_with_policy_recommendations.md",
            size: 8 * 1024,
            kind: "markdown",
          },
          {
            path: "reports/screen.png",
            name: "screen.png",
            size: 2048,
            kind: "image",
          },
          { path: "reports/conflict_brief.docx", name: "conflict_brief.docx", size: 17 * 1024, kind: "docx" },
        ]);
      }
      if (command === "read_workspace_file_preview") {
        const relativePath = String(payload?.relativePath || "");
        if (relativePath === "reports/conflict_brief_with_policy_recommendations.md") {
          return Promise.resolve({
            path: relativePath,
            kind: "markdown",
            source: "# 冲突简报\n\n重点信息",
            size: 8192,
            truncated: true,
          });
        }
        if (relativePath === "reports/conflict_brief.docx") {
          return Promise.resolve({
            path: relativePath,
            kind: "docx",
            source: "美国以色列伊朗冲突 Word 简报",
            size: 17408,
          });
        }
        if (relativePath === "reports/screen.png") {
          return Promise.resolve({
            path: relativePath,
            kind: "image",
            source: "data:image/png;base64,aW1hZ2UtYnl0ZXM=",
            size: 2048,
          });
        }
        return Promise.resolve({
          path: relativePath,
          kind: "html",
          source: "<html><body><h1>Conflict Report</h1></body></html>",
          size: 26624,
          previewError: "页面预览失败，已回退到源码预览。",
        });
      }
      if (command === "open_external_url") {
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });
  });

  test("shows tree, markdown dual preview, copy path and open file actions", async () => {
    render(
      <WorkspaceFilesPanel
        workspace="E:\\workspace\\session-side-panel-redesign"
        touchedFiles={["reports/conflict_report_with_appendix_and_references.html"]}
        active
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "reports" })).toBeInTheDocument();
      expect(screen.getByText(".html")).toBeInTheDocument();
      expect(screen.getByText("HTML")).toBeInTheDocument();
      expect(screen.getByText("本轮生成")).toBeInTheDocument();
    });

    const markdownRow = screen.getByRole("button", {
      name: "conflict_brief_with_policy_recommendations.md",
    });
    expect(markdownRow).toHaveAttribute(
      "title",
      "reports/conflict_brief_with_policy_recommendations.md",
    );

    fireEvent.click(markdownRow);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "渲染预览" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "源码预览" })).toBeInTheDocument();
      expect(screen.getByText("冲突简报")).toBeInTheDocument();
      expect(screen.getByText("仅展示前 256 KB 内容")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "复制路径" }));
    expect(writeTextMock).toHaveBeenCalledWith("reports/conflict_brief_with_policy_recommendations.md");

    fireEvent.click(screen.getByRole("button", { name: "打开文件" }));
    expect(invokeMock).toHaveBeenLastCalledWith(
      "open_external_url",
      expect.objectContaining({
        url: expect.stringContaining("reports\\conflict_brief_with_policy_recommendations.md"),
      }),
    );
  });

  test("supports docx text preview and directory collapse", async () => {
    render(
      <WorkspaceFilesPanel
        workspace="E:\\workspace\\session-side-panel-redesign"
        touchedFiles={[]}
        active
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "reports" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getAllByRole("button", { name: "reports" })[0]!);
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "conflict_brief.docx" })).not.toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "reports" }));
    fireEvent.click(await screen.findByRole("button", { name: "conflict_brief.docx" }));

    await waitFor(() => {
      expect(screen.getAllByText("文本预览").length).toBeGreaterThan(0);
      expect(screen.getByText("美国以色列伊朗冲突 Word 简报")).toBeInTheDocument();
    });
  });

  test("shows html fallback notice and keeps source preview available", async () => {
    render(
      <WorkspaceFilesPanel
        workspace="E:\\workspace\\session-side-panel-redesign"
        touchedFiles={["reports/conflict_report_with_appendix_and_references.html"]}
        active
      />
    );

    fireEvent.click(
      await screen.findByRole("button", {
        name: "conflict_report_with_appendix_and_references.html",
      }),
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "页面预览" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "源码预览" })).toBeInTheDocument();
      expect(screen.getByText("页面预览失败，已回退到源码预览。")).toBeInTheDocument();
      expect(screen.getByText(/<html><body><h1>Conflict Report<\/h1><\/body><\/html>/)).toBeInTheDocument();
    });
  });

  test("renders image files inline in the preview pane", async () => {
    render(
      <WorkspaceFilesPanel
        workspace="E:\\workspace\\session-side-panel-redesign"
        touchedFiles={["reports/screen.png"]}
        active
      />
    );

    fireEvent.click(await screen.findByRole("button", { name: "screen.png" }));

    await waitFor(() => {
      expect(screen.getByAltText("reports/screen.png")).toHaveAttribute(
        "src",
        "data:image/png;base64,aW1hZ2UtYnl0ZXM=",
      );
      expect(screen.queryByText("该文件暂不支持内嵌预览，请使用系统默认应用打开。")).not.toBeInTheDocument();
    });
  });
});
