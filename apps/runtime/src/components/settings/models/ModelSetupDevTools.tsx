type ModelSetupDevToolsProps = {
  show: boolean;
  onResetFirstUseOnboarding?: () => void;
  onOpenQuickModelSetup?: () => void;
};

export function ModelSetupDevTools({
  show,
  onResetFirstUseOnboarding,
  onOpenQuickModelSetup,
}: ModelSetupDevToolsProps) {
  if (!show) return null;

  return (
    <div data-testid="model-setup-dev-tools" className="mt-4 rounded-2xl border border-amber-200 bg-amber-50/80 p-4">
      <div className="text-xs font-semibold uppercase tracking-[0.14em] text-amber-700">Dev Only</div>
      <div className="mt-1 text-sm font-medium text-amber-950">首次引导调试入口</div>
      <div className="mt-1 text-xs leading-5 text-amber-800/80">
        用于在开发阶段反复测试首次连接引导，不会在正式环境展示。
      </div>
      <div className="mt-3 grid gap-2 sm:grid-cols-2">
        <button
          type="button"
          onClick={onResetFirstUseOnboarding}
          className="sm-btn rounded-xl border border-amber-300 bg-white px-4 py-2 text-sm text-amber-900 hover:bg-amber-100"
        >
          重置首次引导状态
        </button>
        <button
          type="button"
          onClick={onOpenQuickModelSetup}
          className="sm-btn rounded-xl border border-amber-300 bg-white px-4 py-2 text-sm text-amber-900 hover:bg-amber-100"
        >
          打开首次配置弹层
        </button>
      </div>
    </div>
  );
}
