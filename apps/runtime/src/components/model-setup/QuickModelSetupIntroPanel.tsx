import { BadgeCheck, Wand2 } from "lucide-react";
import { MODEL_SETUP_OUTCOMES, MODEL_SETUP_STEPS } from "../../app-shell-constants";

export function QuickModelSetupIntroPanel() {
  return (
    <div className="relative overflow-hidden bg-[linear-gradient(180deg,#eff6ff_0%,#f8fafc_100%)] p-6 sm:p-7 lg:overflow-y-auto lg:p-6">
      <div className="absolute inset-x-0 top-0 h-28 bg-[radial-gradient(circle_at_top,_rgba(37,99,235,0.18),_transparent_72%)]" />
      <div className="relative">
        <div className="inline-flex items-center gap-2 rounded-full bg-white/80 px-3 py-1 text-[11px] font-semibold text-[var(--sm-primary-strong)] shadow-[var(--sm-shadow-sm)]">
          <Wand2 className="h-3.5 w-3.5" />
          一次配置，后续复用
        </div>
        <div className="mt-4 text-2xl font-semibold tracking-tight text-[var(--sm-text)]">1 分钟完成模型接入</div>
        <div className="mt-3 text-sm leading-6 text-[var(--sm-text-muted)]">
          先选服务商模板，再填入 API Key。默认参数已经按常见场景预填好，连接通过后即可直接开始任务。
        </div>
        <div className="mt-5 space-y-3">
          {MODEL_SETUP_STEPS.map((step, index) => (
            <div
              key={step.title}
              className="flex items-start gap-3 rounded-2xl border border-white/70 bg-white/70 px-4 py-3 backdrop-blur-sm"
            >
              <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-[var(--sm-primary)] text-sm font-semibold text-white">
                {index + 1}
              </div>
              <div>
                <div className="text-sm font-medium text-[var(--sm-text)]">{step.title}</div>
                <div className="mt-1 text-xs leading-5 text-[var(--sm-text-muted)]">{step.description}</div>
              </div>
            </div>
          ))}
        </div>
        <div className="mt-5 flex flex-wrap gap-2">
          {MODEL_SETUP_OUTCOMES.map((item) => (
            <span
              key={item}
              className="inline-flex items-center gap-1.5 rounded-full border border-white/80 bg-white/85 px-3 py-1.5 text-xs text-[var(--sm-text-muted)] shadow-[var(--sm-shadow-sm)]"
            >
              <BadgeCheck className="h-3.5 w-3.5 text-[var(--sm-primary)]" />
              {item}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}
