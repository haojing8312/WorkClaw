import { render, screen } from "@testing-library/react";
import { FeishuBrowserSetupView } from "../FeishuBrowserSetupView";

test("shows login-required guidance", () => {
  render(
    <FeishuBrowserSetupView
      session={{
        session_id: "sess-1",
        provider: "feishu",
        step: "LOGIN_REQUIRED",
        app_id: null,
        app_secret_present: false,
      }}
      onRetry={() => Promise.resolve()}
      onOpenBrowser={() => Promise.resolve()}
      onCancel={() => Promise.resolve()}
    />,
  );

  expect(screen.getByText("请先登录飞书")).toBeInTheDocument();
});
