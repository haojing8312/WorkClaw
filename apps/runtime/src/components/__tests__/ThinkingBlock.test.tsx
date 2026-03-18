import { render, screen } from "@testing-library/react";
import { ThinkingBlock } from "../ThinkingBlock";

describe("ThinkingBlock", () => {
  test("renders as a lightweight meta row instead of a boxed inset card", () => {
    const { container } = render(
      <ThinkingBlock
        status="completed"
        content="先拆解问题，再汇总答案。"
        durationMs={2400}
        expanded={false}
        onToggle={() => {}}
      />,
    );

    const label = screen.getByText("已思考 2.4s");
    const root = label.closest("div")?.parentElement?.parentElement;

    expect(root).toBeTruthy();
    expect(root?.className).toContain("border-b");
    expect(root?.className).toContain("text-slate-400");
    expect(root?.className).not.toContain("rounded-2xl");
    expect(root?.className).not.toContain("bg-slate-50");

    const statusDot = container.querySelector("span.inline-flex.h-4.w-4");
    expect(statusDot?.className).toContain("border-slate-200");
    expect(statusDot?.className).not.toContain("border-emerald-200");
  });

  test("keeps expanded reasoning details lightweight and separated from the meta row", () => {
    const { container } = render(
      <ThinkingBlock
        status="completed"
        content="先拆解问题，再汇总答案。"
        durationMs={2400}
        expanded
        onToggle={() => {}}
      />,
    );

    expect(screen.getByText("先拆解问题，再汇总答案。")).toBeInTheDocument();
    const panel = container.querySelector("div[data-testid='thinking-block-detail']");

    expect(panel).toBeTruthy();
    expect(panel?.className).toContain("border-l");
    expect(panel?.className).not.toContain("rounded-2xl");
  });
});
