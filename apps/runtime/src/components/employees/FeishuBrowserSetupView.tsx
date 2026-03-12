type Session = {
  session_id: string;
  provider: string;
  step: string;
  app_id: string | null;
  app_secret_present: boolean;
};

type Props = {
  session: Session;
  onRetry: () => Promise<void>;
  onOpenBrowser: () => Promise<void>;
  onCancel: () => Promise<void>;
};

export function FeishuBrowserSetupView({ session, onRetry, onOpenBrowser, onCancel }: Props) {
  const message =
    session.step === "LOGIN_REQUIRED" ? "请先登录飞书" : `当前步骤：${session.step}`;

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
      <div className="text-sm font-medium text-gray-900">飞书浏览器配置向导</div>
      <div className="text-sm text-gray-700">{message}</div>
      <div className="flex gap-2">
        <button
          className="rounded bg-blue-600 px-3 py-1.5 text-sm text-white"
          onClick={() => void onOpenBrowser()}
        >
          打开浏览器
        </button>
        <button
          className="rounded bg-gray-100 px-3 py-1.5 text-sm text-gray-800"
          onClick={() => void onRetry()}
        >
          重试当前步骤
        </button>
        <button
          className="rounded bg-gray-100 px-3 py-1.5 text-sm text-gray-800"
          onClick={() => void onCancel()}
        >
          取消
        </button>
      </div>
    </div>
  );
}
