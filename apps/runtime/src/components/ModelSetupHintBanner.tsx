import { BadgeCheck, Bot, Sparkles } from "lucide-react";
import { MODEL_SETUP_OUTCOMES } from "../app-shell-constants";

interface ModelSetupHintBannerProps {
  show: boolean;
  onDismiss: () => void;
  onOpenQuickSetup: () => void;
}

export function ModelSetupHintBanner({
  show,
  onDismiss,
  onOpenQuickSetup,
}: ModelSetupHintBannerProps) {
  if (!show) {
    return null;
  }

  return (
    <div className="px-4 pt-4">
      <div
        data-testid="model-setup-hint"
        className="relative overflow-hidden rounded-[28px] border border-[var(--sm-primary-soft)] bg-white px-5 py-5 shadow-[0_18px_60px_rgba(37,99,235,0.12)]"
      >
        <div className="absolute inset-y-0 right-0 hidden w-72 bg-[radial-gradient(circle_at_center,_rgba(37,99,235,0.16),_transparent_72%)] md:block" />
        <div className="relative flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
          <div className="min-w-0">
            <div className="inline-flex items-center gap-2 rounded-full bg-[var(--sm-primary-soft)] px-3 py-1 text-[11px] font-semibold text-[var(--sm-primary-strong)]">
              <Sparkles className="h-3.5 w-3.5" />
              首次引导
            </div>
            <div className="mt-3 text-lg font-semibold text-[var(--sm-text)]">先连接一个大模型，智能体才能开始工作</div>
            <div className="mt-2 max-w-2xl text-sm leading-6 text-[var(--sm-text-muted)]">
              只需 1 分钟完成配置。配置后就能创建会话、执行技能和驱动智能体员工协作。
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              {MODEL_SETUP_OUTCOMES.map((item) => (
                <span
                  key={item}
                  className="inline-flex items-center gap-1.5 rounded-full border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] px-3 py-1.5 text-xs text-[var(--sm-text-muted)]"
                >
                  <BadgeCheck className="h-3.5 w-3.5 text-[var(--sm-primary)]" />
                  {item}
                </span>
              ))}
            </div>
          </div>
          <div className="flex flex-col gap-3 xl:min-w-[320px]">
            <div className="rounded-2xl border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] px-4 py-3">
              <div className="flex items-center gap-2 text-sm font-medium text-[var(--sm-text)]">
                <Bot className="h-4 w-4 text-[var(--sm-primary)]" />
                推荐先用快速配置
              </div>
              <div className="mt-1 text-xs leading-5 text-[var(--sm-text-muted)]">
                默认模板会自动带出常用参数，建议先跑通连接；高级参数可在完成后再从侧边栏进入设置调整。
              </div>
            </div>
            <div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap">
              <button
                data-testid="model-setup-hint-open-quick-setup"
                onClick={onOpenQuickSetup}
                className="sm-btn sm-btn-primary min-h-11 flex-1 rounded-xl px-4 text-sm"
              >
                快速配置（1分钟）
              </button>
              <button
                data-testid="model-setup-hint-dismiss"
                onClick={onDismiss}
                className="sm-btn sm-btn-ghost min-h-11 rounded-xl px-4 text-sm"
              >
                稍后再说
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
