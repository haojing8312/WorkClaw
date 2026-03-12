import type { BrowserBridgeInstallStatus } from "../../types";

type Props = {
  status: BrowserBridgeInstallStatus;
  installing: boolean;
  onInstall: () => Promise<void>;
  onOpenExtensionPage: () => Promise<void>;
  onOpenExtensionDir: () => Promise<void>;
  onStartFeishuSetup: () => Promise<void>;
};

export function BrowserBridgeInstallCard({
  status,
  installing,
  onInstall,
  onOpenExtensionPage,
  onOpenExtensionDir,
  onStartFeishuSetup,
}: Props) {
  return (
    <div className="bg-white border border-gray-200 rounded-xl p-4 space-y-3">
      <div>
        <div className="text-sm font-medium text-gray-900">浏览器桥接安装</div>
        <div className="text-xs text-gray-500 mt-1">
          自动安装本地桥接，并引导你在 Chrome 中完成最后一步启用。
        </div>
      </div>

      {status.state === "not_installed" && (
        <button
          type="button"
          disabled={installing}
          onClick={() => void onInstall()}
          className="h-8 px-3 rounded bg-indigo-600 hover:bg-indigo-700 disabled:bg-indigo-300 text-white text-xs"
        >
          {installing ? "安装中..." : "安装浏览器桥接"}
        </button>
      )}

      {status.state === "waiting_for_enable" && (
        <>
          <div className="text-sm text-gray-700">
            请在 Chrome 扩展页开启开发者模式，并加载已为你准备好的 WorkClaw 扩展目录
          </div>
          <div className="flex gap-2">
            <button
              type="button"
              className="rounded bg-gray-100 px-3 py-1.5 text-sm text-gray-800"
              onClick={() => void onOpenExtensionPage()}
            >
              重新打开 Chrome 扩展页
            </button>
            <button
              type="button"
              className="rounded bg-gray-100 px-3 py-1.5 text-sm text-gray-800"
              onClick={() => void onOpenExtensionDir()}
            >
              打开扩展目录
            </button>
          </div>
        </>
      )}

      {status.state === "connected" && (
        <>
          <div className="text-sm text-gray-700">浏览器桥接已启用，可以开始飞书配置</div>
          <button
            type="button"
            className="h-8 px-3 rounded bg-indigo-600 hover:bg-indigo-700 text-white text-xs"
            onClick={() => void onStartFeishuSetup()}
          >
            启动飞书浏览器配置
          </button>
        </>
      )}
    </div>
  );
}
