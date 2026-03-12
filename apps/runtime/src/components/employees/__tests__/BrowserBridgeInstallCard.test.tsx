import { fireEvent, render, screen } from "@testing-library/react";
import { BrowserBridgeInstallCard } from "../BrowserBridgeInstallCard";

describe("BrowserBridgeInstallCard", () => {
  test("renders install action for not installed state", () => {
    render(
      <BrowserBridgeInstallCard
        status={{
          state: "not_installed",
          chrome_found: true,
          native_host_installed: false,
          extension_dir_ready: false,
          bridge_connected: false,
          last_error: null,
        }}
        installing={false}
        onInstall={() => Promise.resolve()}
        onOpenExtensionPage={() => Promise.resolve()}
        onOpenExtensionDir={() => Promise.resolve()}
        onStartFeishuSetup={() => Promise.resolve()}
      />,
    );

    expect(screen.getByText("浏览器桥接安装")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "安装浏览器桥接" })).toBeInTheDocument();
  });

  test("renders waiting actions when user must enable the extension", () => {
    const onOpenExtensionPage = vi.fn(async () => undefined);
    const onOpenExtensionDir = vi.fn(async () => undefined);

    render(
      <BrowserBridgeInstallCard
        status={{
          state: "waiting_for_enable",
          chrome_found: true,
          native_host_installed: true,
          extension_dir_ready: true,
          bridge_connected: false,
          last_error: null,
        }}
        installing={false}
        onInstall={() => Promise.resolve()}
        onOpenExtensionPage={onOpenExtensionPage}
        onOpenExtensionDir={onOpenExtensionDir}
        onStartFeishuSetup={() => Promise.resolve()}
      />,
    );

    expect(
      screen.getByText("请在 Chrome 扩展页开启开发者模式，并加载已为你准备好的 WorkClaw 扩展目录"),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "重新打开 Chrome 扩展页" }));
    fireEvent.click(screen.getByRole("button", { name: "打开扩展目录" }));

    expect(onOpenExtensionPage).toHaveBeenCalledTimes(1);
    expect(onOpenExtensionDir).toHaveBeenCalledTimes(1);
  });

  test("renders start action when bridge is connected", () => {
    const onStartFeishuSetup = vi.fn(async () => undefined);

    render(
      <BrowserBridgeInstallCard
        status={{
          state: "connected",
          chrome_found: true,
          native_host_installed: true,
          extension_dir_ready: true,
          bridge_connected: true,
          last_error: null,
        }}
        installing={false}
        onInstall={() => Promise.resolve()}
        onOpenExtensionPage={() => Promise.resolve()}
        onOpenExtensionDir={() => Promise.resolve()}
        onStartFeishuSetup={onStartFeishuSetup}
      />,
    );

    expect(screen.getByText("浏览器桥接已启用，可以开始飞书配置")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "启动飞书浏览器配置" }));

    expect(onStartFeishuSetup).toHaveBeenCalledTimes(1);
  });
});
