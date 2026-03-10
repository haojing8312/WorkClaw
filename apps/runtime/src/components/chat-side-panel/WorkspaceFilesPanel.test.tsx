import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { WorkspaceFilesPanel } from "./WorkspaceFilesPanel";

const invokeMock = vi.fn();
const writeTextMock = vi.fn(() => Promise.resolve());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("WorkspaceFilesPanel", () => {
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
          { path: "reports/conflict_report.html", name: "conflict_report.html", size: 26 * 1024, kind: "html" },
          { path: "reports/conflict_brief.md", name: "conflict_brief.md", size: 8 * 1024, kind: "markdown" },
          { path: "reports/conflict_brief.docx", name: "conflict_brief.docx", size: 17 * 1024, kind: "docx" },
        ]);
      }
      if (command === "read_workspace_file_preview") {
        const relativePath = String(payload?.relativePath || "");
        if (relativePath === "reports/conflict_brief.md") {
          return Promise.resolve({
            path: relativePath,
            kind: "markdown",
            source: "# 冲突简报\n\n重点信息",
            size: 8192,
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
        return Promise.resolve({
          path: relativePath,
          kind: "html",
          source: "<html><body><h1>Conflict Report</h1></body></html>",
          size: 26624,
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
        touchedFiles={["reports/conflict_report.html"]}
        active
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "reports" })).toBeInTheDocument();
      expect(screen.getByText("conflict_report.html")).toBeInTheDocument();
      expect(screen.getByText("本轮生成")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("conflict_brief.md"));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "渲染预览" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "源码预览" })).toBeInTheDocument();
      expect(screen.getByText("冲突简报")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "复制路径" }));
    expect(writeTextMock).toHaveBeenCalledWith("reports/conflict_brief.md");

    fireEvent.click(screen.getByRole("button", { name: "打开文件" }));
    expect(invokeMock).toHaveBeenLastCalledWith(
      "open_external_url",
      expect.objectContaining({
        url: expect.stringContaining("reports\\conflict_brief.md"),
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

    fireEvent.click(screen.getByRole("button", { name: "reports" }));
    await waitFor(() => {
      expect(screen.queryByText("conflict_brief.docx")).not.toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "reports" }));
    fireEvent.click(await screen.findByText("conflict_brief.docx"));

    await waitFor(() => {
      expect(screen.getAllByText("文本预览").length).toBeGreaterThan(0);
      expect(screen.getByText("美国以色列伊朗冲突 Word 简报")).toBeInTheDocument();
    });
  });
});
