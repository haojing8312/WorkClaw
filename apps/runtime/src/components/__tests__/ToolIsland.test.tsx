import { fireEvent, render, screen } from "@testing-library/react";
import { ToolIsland } from "../ToolIsland";

describe("ToolIsland", () => {
  test("uses lightweight event-row summaries instead of a stacked execution card header", () => {
    render(
      <ToolIsland
        isRunning={false}
        toolCalls={[
          {
            id: "search-1",
            name: "web_search",
            input: { query: "US military presence Middle East 2025" },
            output: "done",
            status: "completed",
          },
          {
            id: "write-1",
            name: "write_file",
            input: { path: "conflict_report.html" },
            output: "done",
            status: "completed",
          },
        ]}
      />,
    );

    expect(screen.getByText("已完成 2 个步骤")).toBeInTheDocument();
    expect(screen.queryByText("执行记录")).not.toBeInTheDocument();
    expect(screen.queryByText("已执行 2 个操作")).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("tool-island-summary"));

    expect(screen.getByText("网页搜索")).toBeInTheDocument();
    expect(screen.getByText("写入文件")).toBeInTheDocument();
    expect(screen.queryByText("web_search")).not.toBeInTheDocument();
    expect(screen.queryByText("write_file")).not.toBeInTheDocument();
  });

  test("aligns to the parent message rail instead of rendering as a centered narrow island", () => {
    render(
      <ToolIsland
        isRunning
        toolCalls={[
          {
            id: "bash-1",
            name: "bash",
            input: { command: "pnpm test" },
            output: "",
            status: "running",
          },
        ]}
      />,
    );

    const summary = screen.getByTestId("tool-island-summary");
    const widthRail = summary.parentElement;

    expect(widthRail).toBeTruthy();
    expect(widthRail?.className).toContain("w-full");
    expect(widthRail?.className).not.toContain("mx-auto");
    expect(widthRail?.className).not.toContain("max-w-[360px]");
  });

  test("renders structured tool summaries instead of raw json blobs", () => {
    render(
      <ToolIsland
        isRunning={false}
        toolCalls={[
          {
            id: "write-structured",
            name: "write_file",
            input: { path: "structured-report.html" },
            output: JSON.stringify({
              ok: true,
              tool: "write_file",
              summary: "成功写入 12 字节到 structured-report.html",
              details: {
                path: "structured-report.html",
                absolute_path: "E:/workspace/structured-report.html",
                bytes_written: 12,
              },
            }),
            status: "completed",
          },
        ]}
      />,
    );

    fireEvent.click(screen.getByText("已完成 1 个步骤"));
    fireEvent.click(screen.getByTestId("tool-island-step-write-structured"));

    expect(screen.getByText("成功写入 12 字节到 structured-report.html")).toBeInTheDocument();
    expect(screen.queryByText(/"summary"/)).not.toBeInTheDocument();
    expect(screen.getByText(/"bytes_written": 12/)).toBeInTheDocument();
  });

  test("shows lightweight capability badges and reads envelope data details", () => {
    render(
      <ToolIsland
        isRunning={false}
        toolManifest={[
          {
            name: "write_file",
            description: "write file",
            display_name: "写入文件",
            category: "filesystem",
            read_only: false,
            destructive: false,
            concurrency_safe: false,
            open_world: false,
            requires_approval: true,
            source: "builtin",
          },
        ]}
        toolCalls={[
          {
            id: "write-envelope",
            name: "write_file",
            input: { path: "notes.md" },
            output: JSON.stringify({
              ok: true,
              summary: "已更新 notes.md",
              data: {
                path: "notes.md",
                bytes_written: 24,
              },
              artifacts: [],
            }),
            status: "completed",
          },
        ]}
      />,
    );

    fireEvent.click(screen.getByTestId("tool-island-summary"));

    expect(screen.getByText("需确认")).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("tool-island-step-write-envelope"));

    expect(screen.getByText("写入文件属于需要确认的执行操作，确认后才会继续。")).toBeInTheDocument();
    expect(screen.getByText(/"bytes_written": 24/)).toBeInTheDocument();
  });
});
