interface QuickFeishuSetupPanelProps {
  onOpenQuickFeishuSetupFromDialog?: () => void;
  onSkipQuickFeishuSetup?: () => void;
}

export function QuickFeishuSetupPanel({
  onOpenQuickFeishuSetupFromDialog,
  onSkipQuickFeishuSetup,
}: QuickFeishuSetupPanelProps) {
  return (
    <div className="mt-6 space-y-4">
      <div className="rounded-3xl border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] p-5">
        <div className="text-sm font-semibold text-[var(--sm-text)]">飞书接入已准备好进入下一步</div>
        <div className="mt-2 text-sm leading-6 text-[var(--sm-text-muted)]">
          模型和搜索已经配置完成。现在可以继续打开飞书接入向导，也可以暂时跳过，稍后再到“设置 &gt; 渠道连接器 &gt; 飞书”补配。
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <span className="inline-flex items-center rounded-full bg-white px-3 py-1 text-xs text-[var(--sm-text-muted)]">
            可跳过
          </span>
          <span className="inline-flex items-center rounded-full bg-white px-3 py-1 text-xs text-[var(--sm-text-muted)]">
            后续可在设置中重开
          </span>
        </div>
      </div>

      <div className="rounded-2xl border border-[var(--sm-border)] bg-white px-4 py-4">
        <div className="text-sm font-medium text-[var(--sm-text)]">建议顺序</div>
        <div className="mt-3 space-y-2 text-sm text-[var(--sm-text-muted)]">
          <div>1. 检查运行环境</div>
          <div>2. 安装飞书官方插件</div>
          <div>3. 绑定已有机器人或新建机器人</div>
          <div>4. 完成授权并设置接待员工</div>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        <button
          type="button"
          data-testid="quick-feishu-setup-open-settings"
          onClick={onOpenQuickFeishuSetupFromDialog}
          className="sm-btn sm-btn-primary h-11 rounded-xl"
        >
          现在配置飞书
        </button>
        <button
          type="button"
          data-testid="quick-feishu-setup-skip"
          onClick={onSkipQuickFeishuSetup}
          className="sm-btn sm-btn-secondary h-11 rounded-xl"
        >
          暂时跳过，先开始使用
        </button>
      </div>
    </div>
  );
}
