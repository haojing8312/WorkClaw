import { fireEvent, render, screen } from "@testing-library/react";
import { ToolIsland } from "../ToolIsland";

describe("ToolIsland", () => {
  test("uses user-facing summaries instead of raw engineering wording", () => {
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
    expect(screen.queryByText("已执行 2 个操作")).not.toBeInTheDocument();

    fireEvent.click(screen.getByText("已完成 2 个步骤"));

    expect(screen.getByText("网页搜索")).toBeInTheDocument();
    expect(screen.getByText("写入文件")).toBeInTheDocument();
    expect(screen.queryByText("web_search")).not.toBeInTheDocument();
    expect(screen.queryByText("write_file")).not.toBeInTheDocument();
  });
});
