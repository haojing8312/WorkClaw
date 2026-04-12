import type { Ref } from "react";
import {
  BadgeCheck,
  Sparkles,
  X,
} from "lucide-react";
import { QuickFeishuSetupPanel } from "./model-setup/QuickFeishuSetupPanel";
import { QuickModelSetupFooter } from "./model-setup/QuickModelSetupFooter";
import { QuickModelSetupIntroPanel } from "./model-setup/QuickModelSetupIntroPanel";
import { QuickModelSetupModelPanel } from "./model-setup/QuickModelSetupModelPanel";
import { SearchConfigForm } from "./SearchConfigForm";
import {
  DEFAULT_MODEL_PROVIDER_ID,
  MODEL_PROVIDER_CATALOG,
  type ModelProviderCatalogItem,
} from "../model-provider-catalog";
import {
  MODEL_SETUP_OUTCOMES,
  MODEL_SETUP_STEPS,
} from "../app-shell-constants";
import type { SearchConfigFormState } from "../lib/search-config";
import type { ModelConnectionTestResult } from "../types";
import type { QuickModelFormState } from "../scenes/useQuickSetupCoordinator";

interface QuickModelTestDisplay {
  title: string;
  message?: string;
  rawMessage?: string | null;
}

export interface QuickModelSetupDialogProps {
  show: boolean;
  quickSetupStep: "model" | "search" | "feishu";
  canDismissQuickModelSetup: boolean;
  isBlockingInitialModelSetup: boolean;
  quickModelApiKeyInputRef: Ref<HTMLInputElement>;
  quickModelApiKeyVisible: boolean;
  quickModelError: string;
  quickModelForm: QuickModelFormState;
  quickModelPresetKey: string;
  quickModelSaving: boolean;
  quickModelTestDisplay: QuickModelTestDisplay | null;
  quickModelTestResult: ModelConnectionTestResult | null;
  quickModelTesting: boolean;
  quickSearchApiKeyVisible: boolean;
  quickSearchError: string;
  quickSearchForm: SearchConfigFormState;
  quickSearchSaving: boolean;
  quickSearchTestResult: boolean | null;
  quickSearchTesting: boolean;
  selectedQuickModelProvider: ModelProviderCatalogItem;
  shouldShowQuickModelRawMessage: boolean;
  onApplyQuickModelPreset: (providerId: string) => void;
  onApplyQuickSearchPreset: (value: string) => void;
  onCloseQuickModelSetup: () => void;
  onOpenExternalLink: (url: string) => void;
  onQuickModelFormChange: (
    updater: (previous: QuickModelFormState) => QuickModelFormState,
  ) => void;
  onQuickModelApiKeyVisibilityToggle: () => void;
  onQuickModelErrorChange: (value: string) => void;
  onQuickModelTestResultChange: (value: ModelConnectionTestResult | null) => void;
  onQuickSearchFormChange: (next: SearchConfigFormState) => void;
  onQuickSearchApiKeyVisibilityToggle: () => void;
  onQuickSearchErrorChange: (value: string) => void;
  onQuickSearchTestResultChange: (value: boolean | null) => void;
  onSaveQuickModelSetup: () => void;
  onSaveQuickSearchSetup: () => void;
  onSkipQuickSearchSetup?: () => void;
  onSkipQuickFeishuSetup?: () => void;
  onOpenQuickFeishuSetupFromDialog?: () => void;
  onTestQuickModelSetupConnection: () => void;
  onTestQuickSearchSetupConnection: () => void;
}

export function QuickModelSetupDialog({
  show,
  quickSetupStep,
  canDismissQuickModelSetup,
  isBlockingInitialModelSetup,
  quickModelApiKeyInputRef,
  quickModelApiKeyVisible,
  quickModelError,
  quickModelForm,
  quickModelPresetKey,
  quickModelSaving,
  quickModelTestDisplay,
  quickModelTestResult,
  quickModelTesting,
  quickSearchApiKeyVisible,
  quickSearchError,
  quickSearchForm,
  quickSearchSaving,
  quickSearchTestResult,
  quickSearchTesting,
  selectedQuickModelProvider,
  shouldShowQuickModelRawMessage,
  onApplyQuickModelPreset,
  onApplyQuickSearchPreset,
  onCloseQuickModelSetup,
  onOpenExternalLink,
  onQuickModelFormChange,
  onQuickModelApiKeyVisibilityToggle,
  onQuickModelErrorChange,
  onQuickModelTestResultChange,
  onQuickSearchFormChange,
  onQuickSearchApiKeyVisibilityToggle,
  onQuickSearchErrorChange,
  onQuickSearchTestResultChange,
  onSaveQuickModelSetup,
  onSaveQuickSearchSetup,
  onSkipQuickSearchSetup,
  onSkipQuickFeishuSetup,
  onOpenQuickFeishuSetupFromDialog,
  onTestQuickModelSetupConnection,
  onTestQuickSearchSetupConnection,
}: QuickModelSetupDialogProps) {
  if (!show) {
    return null;
  }

  return (
    <div
      data-testid="quick-model-setup-dialog"
      className="fixed inset-x-0 bottom-0 top-11 z-40 flex items-start justify-center overflow-y-auto bg-slate-950/30 px-4 py-4 backdrop-blur-sm sm:py-6"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onCloseQuickModelSetup();
        }
      }}
    >
      <div
        data-testid="quick-model-setup-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="quick-model-setup-title"
        className="h-[calc(100vh-4.75rem)] w-full max-w-[1120px] max-h-[960px] overflow-hidden rounded-[28px] border border-white/80 bg-white shadow-[0_36px_120px_rgba(15,23,42,0.24)]"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="flex h-full min-h-0 flex-col lg:grid lg:grid-cols-[0.9fr_1.1fr]">
          <QuickModelSetupIntroPanel />
          <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden p-6 sm:p-7 lg:p-8">
            <div className="flex items-start justify-between gap-4">
              <div>
                <div id="quick-model-setup-title" className="text-xl font-semibold text-[var(--sm-text)]">
                  {quickSetupStep === "model"
                    ? "快速配置模型"
                    : quickSetupStep === "search"
                      ? "搜索引擎"
                      : "飞书接入（可选）"}
                </div>
                <div className="mt-2 text-sm leading-6 text-[var(--sm-text-muted)]">
                  {quickSetupStep === "model"
                    ? "先完成模型接入，保存后可继续配置搜索，也可以先开始使用。"
                    : quickSetupStep === "search"
                      ? "搜索不是阻塞项。你可以现在配置，也可以先进入 WorkClaw，后续需要联网检索时再补上。"
                      : "飞书不是阻塞项。你可以现在继续配置，也可以先进入 WorkClaw，后面再从设置里补上。"}
                </div>
              </div>
              <button
                type="button"
                data-testid="quick-model-setup-close"
                onClick={onCloseQuickModelSetup}
                disabled={!canDismissQuickModelSetup}
                aria-label="关闭引导"
                className="sm-btn sm-btn-ghost h-10 w-10 rounded-xl disabled:cursor-not-allowed disabled:opacity-50"
              >
                <X className="h-4 w-4" />
              </button>
            </div>

            <div
              data-testid="quick-model-setup-scroll-region"
              className="min-h-0 flex-1 overflow-y-auto pr-1"
            >
              {quickSetupStep === "search" && (
                <div className="mt-6">
                  <SearchConfigForm
                    form={quickSearchForm}
                    onFormChange={(next) => {
                      onQuickSearchFormChange(next);
                      onQuickSearchErrorChange("");
                      onQuickSearchTestResultChange(null);
                    }}
                    onApplyPreset={onApplyQuickSearchPreset}
                    showApiKey={quickSearchApiKeyVisible}
                    onToggleApiKey={onQuickSearchApiKeyVisibilityToggle}
                    error={quickSearchError}
                    testResult={quickSearchTestResult}
                    testing={quickSearchTesting}
                    saving={quickSearchSaving}
                    onTest={onTestQuickSearchSetupConnection}
                    onSave={onSaveQuickSearchSetup}
                    panelClassName="space-y-3"
                    actionClassName="mt-4 grid grid-cols-1 gap-2 sm:grid-cols-3"
                    saveLabel="完成配置"
                    onSecondaryAction={onSkipQuickSearchSetup}
                    secondaryActionLabel="跳过搜索，稍后再配"
                  />
                </div>
              )}
              {quickSetupStep === "feishu" && (
                <QuickFeishuSetupPanel
                  onOpenQuickFeishuSetupFromDialog={onOpenQuickFeishuSetupFromDialog}
                  onSkipQuickFeishuSetup={onSkipQuickFeishuSetup}
                />
              )}
              {quickSetupStep === "model" && (
                <QuickModelSetupModelPanel
                  quickModelApiKeyInputRef={quickModelApiKeyInputRef}
                  quickModelApiKeyVisible={quickModelApiKeyVisible}
                  quickModelError={quickModelError}
                  quickModelForm={quickModelForm}
                  quickModelPresetKey={quickModelPresetKey}
                  selectedQuickModelProvider={selectedQuickModelProvider}
                  onApplyQuickModelPreset={onApplyQuickModelPreset}
                  onOpenExternalLink={onOpenExternalLink}
                  onQuickModelApiKeyVisibilityToggle={onQuickModelApiKeyVisibilityToggle}
                  onQuickModelFormChange={onQuickModelFormChange}
                  onQuickModelTestResultChange={onQuickModelTestResultChange}
                />
              )}
            </div>

            <QuickModelSetupFooter
              canDismissQuickModelSetup={canDismissQuickModelSetup}
              isBlockingInitialModelSetup={isBlockingInitialModelSetup}
              quickModelSaving={quickModelSaving}
              quickModelTestDisplay={quickModelTestDisplay}
              quickModelTestResult={quickModelTestResult}
              quickModelTesting={quickModelTesting}
              quickSetupStep={quickSetupStep}
              shouldShowQuickModelRawMessage={shouldShowQuickModelRawMessage}
              onCloseQuickModelSetup={onCloseQuickModelSetup}
              onSaveQuickModelSetup={onSaveQuickModelSetup}
              onTestQuickModelSetupConnection={onTestQuickModelSetupConnection}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

interface ModelSetupGateProps {
  show: boolean;
  onOpenQuickModelSetup: () => void;
}

export function ModelSetupGate({
  show,
  onOpenQuickModelSetup,
}: ModelSetupGateProps) {
  if (!show) {
    return null;
  }

  return (
    <div
      data-testid="model-setup-gate"
      className="fixed inset-x-0 bottom-0 top-11 z-30 flex items-center justify-center bg-[radial-gradient(circle_at_top,_rgba(37,99,235,0.16),_rgba(241,245,249,0.92)_46%,_rgba(241,245,249,0.98)_100%)] px-4 py-6 backdrop-blur-sm"
    >
      <div className="w-full max-w-4xl overflow-hidden rounded-[32px] border border-white/80 bg-white shadow-[0_40px_120px_rgba(15,23,42,0.18)]">
        <div className="grid gap-6 p-6 lg:grid-cols-[1.2fr_0.8fr] lg:p-8">
          <div>
            <div className="inline-flex items-center gap-2 rounded-full bg-[var(--sm-primary-soft)] px-3 py-1 text-[11px] font-semibold text-[var(--sm-primary-strong)]">
              <Sparkles className="h-3.5 w-3.5" />
              首次启动必做一步
            </div>
            <div className="mt-4 text-[30px] font-semibold leading-tight tracking-tight text-[var(--sm-text)]">
              首次使用需要先连接一个大模型
            </div>
            <div className="mt-3 max-w-2xl text-base leading-7 text-[var(--sm-text-muted)]">
              完成模型配置后，才能开始任务、创建会话并驱动智能体员工执行技能。现在只需 1 分钟。
            </div>
            <div className="mt-5 flex flex-wrap gap-2">
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
            <div className="mt-6 flex flex-col gap-2 sm:flex-row sm:flex-wrap">
              <button
                data-testid="model-setup-gate-open-quick-setup"
                onClick={onOpenQuickModelSetup}
                className="sm-btn sm-btn-primary min-h-12 rounded-xl px-5 text-sm"
              >
                快速配置（1分钟）
              </button>
            </div>
          </div>
          <div className="rounded-[26px] border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] p-5">
            <div className="flex items-center gap-2 text-sm font-medium text-[var(--sm-text)]">
              <Sparkles className="h-4 w-4 text-[var(--sm-primary)]" />
              推荐流程
            </div>
            <div className="mt-1 text-xs leading-5 text-[var(--sm-text-muted)]">
              优先选择快速配置，模板会自动补齐常用 URL 和模型名。
            </div>
            <div className="mt-4 space-y-3">
              {MODEL_SETUP_STEPS.map((step, index) => (
                <div key={step.title} className="flex items-start gap-3">
                  <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-white text-sm font-semibold text-[var(--sm-primary-strong)] shadow-[var(--sm-shadow-sm)]">
                    {index + 1}
                  </div>
                  <div>
                    <div className="text-sm font-medium text-[var(--sm-text)]">{step.title}</div>
                    <div className="mt-1 text-xs leading-5 text-[var(--sm-text-muted)]">{step.description}</div>
                  </div>
                </div>
              ))}
            </div>
            <div className="mt-5 rounded-2xl border border-white bg-white px-4 py-3">
              <div className="text-xs font-semibold text-[var(--sm-text)]">支持模板</div>
              <div className="mt-2 flex flex-wrap gap-2">
                {MODEL_PROVIDER_CATALOG.map((provider) => (
                  <span
                    key={provider.id}
                    className="inline-flex items-center rounded-full bg-[var(--sm-primary-soft)] px-2.5 py-1 text-[11px] font-medium text-[var(--sm-primary-strong)]"
                  >
                    {provider.label}
                  </span>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
