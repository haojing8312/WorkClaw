import { fireEvent, render, screen } from "@testing-library/react";
import { TaskJourneySummary } from "../chat-journey/TaskJourneySummary";

describe("TaskJourneySummary", () => {
  test("uses a neutral delivery card instead of a bright sky banner", () => {
    const onViewFiles = vi.fn();

    render(
      <TaskJourneySummary
        model={{
          status: "completed",
          currentTaskTitle: "生成交付文件",
          steps: [],
          deliverables: [
            { path: "report.html", tool: "write_file", category: "primary" },
            { path: "appendix.md", tool: "write_file", category: "secondary" },
          ],
          warnings: [],
        }}
        onViewFiles={onViewFiles}
      />,
    );

    const button = screen.getByRole("button", { name: "查看此任务中的所有文件" });
    const countBadge = screen.getByText("共 2 个文件");

    expect(button.className).toContain("border-slate-200/80");
    expect(button.className).toContain("bg-white/92");
    expect(button.className).not.toContain("bg-gradient-to-br");
    expect(countBadge.className).toContain("border-slate-200");
    expect(countBadge.className).not.toContain("bg-sky-100");

    fireEvent.click(button);
    expect(onViewFiles).toHaveBeenCalledTimes(1);
  });
});
