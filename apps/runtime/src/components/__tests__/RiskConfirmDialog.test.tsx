import { fireEvent, render, screen } from "@testing-library/react";
import { RiskConfirmDialog } from "../RiskConfirmDialog";

describe("RiskConfirmDialog", () => {
  test("renders high risk irreversible warning and calls handlers", () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();

    render(
      <RiskConfirmDialog
        open
        level="high"
        title="删除技能"
        summary="将永久删除本地技能"
        impact="会移除该技能及相关会话入口"
        irreversible
        confirmLabel="确认删除"
        cancelLabel="取消"
        loading={false}
        onConfirm={onConfirm}
        onCancel={onCancel}
      />
    );

    expect(screen.getByText("删除技能")).toBeInTheDocument();
    expect(screen.getByText(/不可逆/)).toBeInTheDocument();
    expect(screen.getByText("会移除该技能及相关会话入口")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(onCancel).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  test("returns null when dialog is closed", () => {
    const { container } = render(
      <RiskConfirmDialog
        open={false}
        level="medium"
        title="安装技能"
        summary="将安装新技能"
        confirmLabel="确认"
        cancelLabel="取消"
        loading={false}
        onConfirm={() => {}}
        onCancel={() => {}}
      />
    );
    expect(container).toBeEmptyDOMElement();
  });
});
