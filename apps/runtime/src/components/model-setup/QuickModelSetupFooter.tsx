import { CheckCircle2, ChevronRight, CircleAlert } from "lucide-react";
import type { ModelConnectionTestResult } from "../../types";

interface QuickModelTestDisplay {
  title: string;
  message?: string;
  rawMessage?: string | null;
}

interface QuickModelSetupFooterProps {
  canDismissQuickModelSetup: boolean;
  isBlockingInitialModelSetup: boolean;
  quickModelSaving: boolean;
  quickModelTestDisplay: QuickModelTestDisplay | null;
  quickModelTestResult: ModelConnectionTestResult | null;
  quickModelTesting: boolean;
  quickSetupStep: "model" | "search" | "feishu";
  shouldShowQuickModelRawMessage: boolean;
  onCloseQuickModelSetup: () => void;
  onSaveQuickModelSetup: () => void;
  onTestQuickModelSetupConnection: () => void;
}

export function QuickModelSetupFooter({
  canDismissQuickModelSetup,
  isBlockingInitialModelSetup,
  quickModelSaving,
  quickModelTestDisplay,
  quickModelTestResult,
  quickModelTesting,
  quickSetupStep,
  shouldShowQuickModelRawMessage,
  onCloseQuickModelSetup,
  onSaveQuickModelSetup,
  onTestQuickModelSetupConnection,
}: QuickModelSetupFooterProps) {
  return (
    <div className="mt-6 border-t border-[var(--sm-border)] pt-4">
      <div className="text-xs leading-5 text-[var(--sm-text-muted)]">
        {isBlockingInitialModelSetup
          ? "首次使用至少完成模型配置后，才能关闭这个引导。搜索与飞书都可以稍后再配。"
          : "按 Esc 或点击遮罩可直接关闭引导。"}
      </div>
      <div data-testid="quick-model-setup-actions" className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2">
        {quickModelTestResult !== null && (
          <div
            data-testid="quick-model-setup-test-result"
            className={`flex items-start gap-2 rounded-2xl border px-3 py-3 text-xs sm:col-span-2 ${
              quickModelTestResult.ok
                ? "border-green-200 bg-green-50 text-green-700"
                : "border-orange-200 bg-orange-50 text-orange-700"
            }`}
          >
            {quickModelTestResult.ok ? (
              <CheckCircle2 className="mt-0.5 h-4 w-4 flex-shrink-0" />
            ) : (
              <CircleAlert className="mt-0.5 h-4 w-4 flex-shrink-0" />
            )}
            <div className="space-y-1">
              <div className="font-medium">
                {quickModelTestResult.ok ? "连接成功，可直接保存并开始" : quickModelTestDisplay?.title}
              </div>
              {!quickModelTestResult.ok && quickModelTestDisplay?.message ? (
                <div>{quickModelTestDisplay.message}</div>
              ) : null}
              {!quickModelTestResult.ok && shouldShowQuickModelRawMessage ? (
                <div className="whitespace-pre-wrap break-all rounded-xl border border-orange-200/80 bg-white/60 px-2.5 py-2 font-mono text-[11px] text-orange-800/90">
                  {quickModelTestDisplay?.rawMessage}
                </div>
              ) : null}
            </div>
          </div>
        )}
        <button
          type="button"
          data-testid="quick-model-setup-cancel"
          onClick={onCloseQuickModelSetup}
          disabled={!canDismissQuickModelSetup}
          className="sm-btn sm-btn-ghost min-h-11 rounded-xl px-4 text-sm disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isBlockingInitialModelSetup ? "完成配置后可关闭" : "关闭引导"}
        </button>
        {quickSetupStep === "model" && (
          <>
            <button
              data-testid="quick-model-setup-test-connection"
              onClick={onTestQuickModelSetupConnection}
              disabled={quickModelSaving || quickModelTesting}
              className="sm-btn sm-btn-secondary min-h-11 rounded-xl px-4 text-sm disabled:opacity-60"
            >
              {quickModelTesting ? "测试中..." : "测试连接"}
            </button>
            <button
              data-testid="quick-model-setup-save"
              onClick={onSaveQuickModelSetup}
              disabled={quickModelSaving || quickModelTesting}
              className="sm-btn sm-btn-primary min-h-11 rounded-xl px-4 text-sm disabled:opacity-60"
            >
              <ChevronRight className="h-4 w-4" />
              {quickModelSaving ? "保存中..." : "保存并继续"}
            </button>
          </>
        )}
      </div>
    </div>
  );
}
