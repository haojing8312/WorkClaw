import { fireEvent, render, screen } from "@testing-library/react";
import { ConnectorDiagnosticsPanel } from "../ConnectorDiagnosticsPanel";

describe("ConnectorDiagnosticsPanel", () => {
  test("renders normalized diagnostics with capability chips and technical details", () => {
    render(
      <ConnectorDiagnosticsPanel
        title="连接器诊断"
        diagnostics={{
          connector: {
            channel: "wecom",
            display_name: "企业微信连接器",
            capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
          },
          status: "authentication_error",
          health: {
            adapter_name: "wecom",
            instance_id: "wecom:wecom-main",
            state: "error",
            last_ok_at: "2026-03-11T10:00:00Z",
            last_error: "signature mismatch",
            reconnect_attempts: 2,
            queue_depth: 3,
            issue: {
              code: "signature_mismatch",
              category: "authentication_error",
              user_message: "签名校验失败",
              technical_message: "signature mismatch",
              retryable: false,
              occurred_at: "2026-03-11T10:00:00Z",
            },
          },
          replay: {
            retained_events: 1,
            acked_events: 0,
          },
        }}
      />,
    );

    expect(screen.getByText("连接器诊断")).toBeInTheDocument();
    expect(screen.getByText("企业微信连接器")).toBeInTheDocument();
    expect(screen.getByText(/签名校验失败/)).toBeInTheDocument();
    expect(screen.getByText("receive_text")).toBeInTheDocument();
    expect(screen.getByText("group_route")).toBeInTheDocument();
    expect(screen.queryByText("原始错误：signature mismatch")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "查看技术详情" }));

    expect(screen.getByText("原始错误：signature mismatch")).toBeInTheDocument();
    expect(screen.getByText("连接标识")).toBeInTheDocument();
    expect(screen.getByText("wecom:wecom-main")).toBeInTheDocument();
  });
});
