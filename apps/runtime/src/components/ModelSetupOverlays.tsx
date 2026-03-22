import type { Ref } from "react";
import {
  BadgeCheck,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Eye,
  EyeOff,
  KeyRound,
  Sparkles,
  Wand2,
  X,
} from "lucide-react";
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
      className="fixed inset-0 z-40 flex items-start justify-center overflow-y-auto bg-slate-950/30 px-4 py-4 backdrop-blur-sm sm:py-6"
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
        className="h-[calc(100vh-2rem)] w-full max-w-[1120px] max-h-[960px] overflow-hidden rounded-[28px] border border-white/80 bg-white shadow-[0_36px_120px_rgba(15,23,42,0.24)]"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="flex h-full min-h-0 flex-col lg:grid lg:grid-cols-[0.9fr_1.1fr]">
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
                    ? "先完成模型接入，保存后自动进入搜索引擎配置。"
                    : quickSetupStep === "search"
                      ? "补齐搜索配置后，智能体即可在首次使用时直接联网检索。"
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
                    onSecondaryAction={!isBlockingInitialModelSetup ? onSkipQuickSearchSetup : undefined}
                    secondaryActionLabel={!isBlockingInitialModelSetup ? "跳过搜索，稍后再配" : undefined}
                  />
                </div>
              )}
              {quickSetupStep === "feishu" && (
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
              )}
              {quickSetupStep === "model" && (
                <div>
                  <div className="mt-6">
                    <div className="flex items-center justify-between gap-3">
                      <div className="sm-field-label mb-0">推荐模板</div>
                      <div className="text-[11px] text-[var(--sm-text-muted)]">先选模板，再补 API Key</div>
                    </div>
                    <div className="mt-2 grid grid-cols-1 gap-2 sm:grid-cols-2">
                      {MODEL_PROVIDER_CATALOG.map((provider) => {
                        const isActive = quickModelPresetKey === provider.id;
                        return (
                          <button
                            key={provider.id}
                            type="button"
                            data-testid={`quick-model-setup-provider-${provider.id}`}
                            onClick={() => onApplyQuickModelPreset(provider.id)}
                            className={`text-left rounded-2xl border px-3 py-3 transition-colors ${
                              isActive
                                ? "border-[var(--sm-primary)] bg-[var(--sm-primary-soft)] shadow-[var(--sm-shadow-sm)]"
                                : "border-[var(--sm-border)] bg-white hover:border-[var(--sm-primary)] hover:bg-[var(--sm-surface-soft)]"
                            }`}
                          >
                            <div className="flex items-start justify-between gap-3">
                              <div>
                                <div className="text-[11px] font-semibold text-[var(--sm-primary-strong)]">{provider.badge}</div>
                                <div className="mt-1 text-sm font-medium text-[var(--sm-text)]">{provider.label}</div>
                              </div>
                              {provider.id === DEFAULT_MODEL_PROVIDER_ID ? (
                                <span className="sm-badge-info">推荐</span>
                              ) : null}
                            </div>
                            <div className="mt-2 text-xs leading-5 text-[var(--sm-text-muted)]">{provider.helper}</div>
                          </button>
                        );
                      })}
                    </div>
                  </div>

                  <div className="mt-6 grid grid-cols-1 gap-3 sm:grid-cols-2">
                    <div>
                      <label className="sm-field-label">连接名称</label>
                      <input
                        value={quickModelForm.name}
                        onChange={(event) => {
                          onQuickModelFormChange((state) => ({ ...state, name: event.target.value }));
                          onQuickModelTestResultChange(null);
                        }}
                        className="sm-input h-11 px-3 text-sm"
                      />
                    </div>
                    <div>
                      <label className="sm-field-label">Base URL</label>
                      <input
                        data-testid="quick-model-setup-base-url"
                        value={quickModelForm.base_url}
                        onChange={(event) => {
                          onQuickModelFormChange((state) => ({ ...state, base_url: event.target.value }));
                          onQuickModelTestResultChange(null);
                        }}
                        className="sm-input h-11 px-3 text-sm"
                        placeholder={selectedQuickModelProvider.baseUrlPlaceholder}
                      />
                    </div>
                    <div>
                      <label className="sm-field-label">模型名</label>
                      <input
                        data-testid="quick-model-setup-model-name"
                        value={quickModelForm.model_name}
                        onChange={(event) => {
                          onQuickModelFormChange((state) => ({ ...state, model_name: event.target.value }));
                          onQuickModelTestResultChange(null);
                        }}
                        className="sm-input h-11 px-3 text-sm"
                        placeholder={selectedQuickModelProvider.modelNamePlaceholder}
                      />
                    </div>
                  </div>

                  <div className="mt-4 rounded-2xl border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] px-4 py-4">
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                      <div>
                        <div className="flex flex-wrap items-center gap-2">
                          <div className="text-sm font-medium text-[var(--sm-text)]">
                            {selectedQuickModelProvider.label}
                          </div>
                          <span className="inline-flex items-center rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-[var(--sm-primary-strong)]">
                            {selectedQuickModelProvider.protocolLabel}
                          </span>
                        </div>
                        <div className="mt-2 text-xs leading-5 text-[var(--sm-text-muted)]">
                          {selectedQuickModelProvider.helper}
                        </div>
                      </div>
                      {selectedQuickModelProvider.officialConsoleUrl ? (
                        <div className="flex flex-wrap gap-2">
                          <button
                            type="button"
                            onClick={() => onOpenExternalLink(selectedQuickModelProvider.officialConsoleUrl ?? "")}
                            className="sm-btn sm-btn-secondary min-h-10 rounded-xl px-4 text-sm"
                          >
                            {selectedQuickModelProvider.officialConsoleLabel ?? "获取 API Key"}
                          </button>
                          {selectedQuickModelProvider.officialDocsUrl ? (
                            <button
                              type="button"
                              onClick={() => onOpenExternalLink(selectedQuickModelProvider.officialDocsUrl ?? "")}
                              className="sm-btn sm-btn-ghost min-h-10 rounded-xl px-4 text-sm"
                            >
                              {selectedQuickModelProvider.officialDocsLabel ?? "查看文档"}
                            </button>
                          ) : null}
                        </div>
                      ) : null}
                    </div>
                    {selectedQuickModelProvider.isCustom ? (
                      <div
                        data-testid="quick-model-setup-custom-guidance"
                        className="mt-3 rounded-2xl border border-dashed border-[var(--sm-border)] bg-white px-3 py-3"
                      >
                        <div className="text-xs font-semibold text-[var(--sm-text)]">
                          {selectedQuickModelProvider.customGuidanceTitle}
                        </div>
                        <div className="mt-2 space-y-1.5 text-[12px] leading-5 text-[var(--sm-text-muted)]">
                          {selectedQuickModelProvider.customGuidanceLines?.map((line) => (
                            <div key={line}>{line}</div>
                          ))}
                        </div>
                      </div>
                    ) : null}
                  </div>

                  <div className="mt-3">
                    <label className="sm-field-label">API Key</label>
                    <div className="relative">
                      <input
                        ref={quickModelApiKeyInputRef}
                        data-testid="quick-model-setup-api-key"
                        type={quickModelApiKeyVisible ? "text" : "password"}
                        value={quickModelForm.api_key}
                        onChange={(event) => {
                          onQuickModelFormChange((state) => ({ ...state, api_key: event.target.value }));
                          onQuickModelTestResultChange(null);
                        }}
                        className="sm-input h-11 px-3 pr-12 text-sm"
                        placeholder="请输入 API Key"
                      />
                      <button
                        type="button"
                        data-testid="quick-model-setup-toggle-api-key-visibility"
                        onClick={onQuickModelApiKeyVisibilityToggle}
                        aria-label={quickModelApiKeyVisible ? "隐藏 API Key" : "显示 API Key"}
                        className="sm-btn sm-btn-ghost absolute right-1 top-1/2 h-9 w-9 -translate-y-1/2 rounded-lg"
                      >
                        {quickModelApiKeyVisible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                      </button>
                    </div>
                    <div className="mt-2 flex items-start gap-2 rounded-2xl border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] px-3 py-3 text-[12px] leading-5 text-[var(--sm-text-muted)]">
                      <KeyRound className="mt-0.5 h-4 w-4 flex-shrink-0 text-[var(--sm-primary)]" />
                      API Key 仅用于当前模型连接。先完成这里的配置并验证连接，后续再按需去设置页调整高级参数。
                    </div>
                  </div>

                  {quickModelError && (
                    <div className="mt-4 flex items-start gap-2 rounded-2xl border border-red-200 bg-red-50 px-3 py-3 text-xs text-red-700">
                      <CircleAlert className="mt-0.5 h-4 w-4 flex-shrink-0" />
                      <span>{quickModelError}</span>
                    </div>
                  )}
                </div>
              )}
            </div>

            <div className="mt-6 border-t border-[var(--sm-border)] pt-4">
              <div className="text-xs leading-5 text-[var(--sm-text-muted)]">
                {isBlockingInitialModelSetup
                  ? "首次使用至少完成模型与搜索配置后，才能关闭这个引导。"
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
                        {quickModelTestResult.ok
                          ? "连接成功，可直接保存并开始"
                          : quickModelTestDisplay?.title}
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
      className="fixed inset-0 z-30 flex items-center justify-center bg-[radial-gradient(circle_at_top,_rgba(37,99,235,0.16),_rgba(241,245,249,0.92)_46%,_rgba(241,245,249,0.98)_100%)] px-4 py-6 backdrop-blur-sm"
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
