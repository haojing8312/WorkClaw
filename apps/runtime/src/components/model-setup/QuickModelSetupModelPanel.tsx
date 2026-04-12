import type { Ref } from "react";
import { CircleAlert, Eye, EyeOff, KeyRound } from "lucide-react";
import {
  DEFAULT_MODEL_PROVIDER_ID,
  MODEL_PROVIDER_CATALOG,
  type ModelProviderCatalogItem,
} from "../../model-provider-catalog";
import type { QuickModelFormState } from "../../scenes/useQuickSetupCoordinator";
import type { ModelConnectionTestResult } from "../../types";

interface QuickModelSetupModelPanelProps {
  quickModelApiKeyInputRef: Ref<HTMLInputElement>;
  quickModelApiKeyVisible: boolean;
  quickModelError: string;
  quickModelForm: QuickModelFormState;
  quickModelPresetKey: string;
  selectedQuickModelProvider: ModelProviderCatalogItem;
  onApplyQuickModelPreset: (providerId: string) => void;
  onOpenExternalLink: (url: string) => void;
  onQuickModelApiKeyVisibilityToggle: () => void;
  onQuickModelFormChange: (
    updater: (previous: QuickModelFormState) => QuickModelFormState,
  ) => void;
  onQuickModelTestResultChange: (value: ModelConnectionTestResult | null) => void;
}

export function QuickModelSetupModelPanel({
  quickModelApiKeyInputRef,
  quickModelApiKeyVisible,
  quickModelError,
  quickModelForm,
  quickModelPresetKey,
  selectedQuickModelProvider,
  onApplyQuickModelPreset,
  onOpenExternalLink,
  onQuickModelApiKeyVisibilityToggle,
  onQuickModelFormChange,
  onQuickModelTestResultChange,
}: QuickModelSetupModelPanelProps) {
  return (
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
              <div className="text-sm font-medium text-[var(--sm-text)]">{selectedQuickModelProvider.label}</div>
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
  );
}
