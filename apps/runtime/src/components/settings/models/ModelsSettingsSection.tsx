import { useEffect, useState } from "react";
import { Eye, EyeOff } from "lucide-react";
import { DEFAULT_MODEL_PROVIDER_ID, MODEL_PROVIDER_CATALOG, getModelProviderCatalogItem } from "../../../model-provider-catalog";
import { getModelErrorDisplay } from "../../../lib/model-error-display";
import { openExternalUrl } from "../../../utils/openExternalUrl";
import type { ModelConfig, ProviderConfig } from "../../../types";
import { ModelSetupDevTools } from "./ModelSetupDevTools";
import { ModelsSettingsConfiguredList } from "./ModelsSettingsConfiguredList";
import {
  deleteModelConfig,
  listModelConfigs,
  listProviderConfigs,
  getDefaultModelForm,
  getModelApiKey,
  resolveModelProviderForEdit,
  saveModelConfig,
  setDefaultModel,
  syncCapabilityRouteToConnection,
  syncConnectionToRouting,
  syncModelConnections,
  testModelConnection,
  validateModelForm,
  type ModelFormState,
} from "./modelSettingsService";

interface ModelsSettingsSectionProps {
  models: ModelConfig[];
  providers: ProviderConfig[];
  onModelsChange: (models: ModelConfig[]) => void;
  onProvidersChange: (providers: ProviderConfig[]) => void;
  showDevModelSetupTools?: boolean;
  onDevResetFirstUseOnboarding?: () => void;
  onDevOpenQuickModelSetup?: () => void;
  onOpenAdvancedRouting?: () => void;
}

export function ModelsSettingsSection({
  models,
  providers,
  onModelsChange,
  onProvidersChange,
  showDevModelSetupTools = false,
  onDevResetFirstUseOnboarding,
  onDevOpenQuickModelSetup,
  onOpenAdvancedRouting,
}: ModelsSettingsSectionProps) {
  const [selectedModelProviderId, setSelectedModelProviderId] = useState(DEFAULT_MODEL_PROVIDER_ID);
  const [form, setForm] = useState<ModelFormState>(() => ({
    ...getDefaultModelForm(),
    is_default: models.length === 0,
  }));
  const [error, setError] = useState("");
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<Awaited<ReturnType<typeof testModelConnection>> | null>(null);
  const [modelSuggestions, setModelSuggestions] = useState<string[]>(getModelProviderCatalogItem("zhipu").models);
  const [modelSaveMessage, setModelSaveMessage] = useState("");
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [showApiKey, setShowApiKey] = useState(false);

  const selectedModelProvider = getModelProviderCatalogItem(selectedModelProviderId);
  const connectionTestDisplay = testResult ? getModelErrorDisplay(testResult) : null;
  const shouldShowConnectionRawMessage = Boolean(
    connectionTestDisplay?.rawMessage &&
      connectionTestDisplay.rawMessage !== connectionTestDisplay.title &&
      connectionTestDisplay.rawMessage !== connectionTestDisplay.message,
  );

  useEffect(() => {
    if (!modelSaveMessage) return;
    const timer = window.setTimeout(() => setModelSaveMessage(""), 1200);
    return () => window.clearTimeout(timer);
  }, [modelSaveMessage]);

  useEffect(() => {
    if (editingModelId) return;
    const defaultForm = getDefaultModelForm(selectedModelProviderId);
    const isPristineForm =
      form.name === defaultForm.name &&
      form.api_format === defaultForm.api_format &&
      form.base_url === defaultForm.base_url &&
      form.model_name === defaultForm.model_name &&
      form.api_key === "";
    if (!isPristineForm) return;
    const nextIsDefault = models.length === 0;
    if (form.is_default === nextIsDefault) return;
    setForm((current) => ({ ...current, is_default: nextIsDefault }));
  }, [editingModelId, form, models.length, selectedModelProviderId]);

  async function refreshModelData() {
    try {
      const list = await listModelConfigs();
      await syncModelConnections(list);
      onModelsChange(list);
      const providerList = await listProviderConfigs();
      const ids = new Set(list.map((model) => model.id));
      const alignedProviders = providerList.filter((provider) => ids.has(provider.id));
      onProvidersChange(alignedProviders);
    } catch (cause) {
      setError("加载模型连接失败: " + String(cause));
    }
  }

  function resetModelForm(providerId = DEFAULT_MODEL_PROVIDER_ID) {
    const nextForm = getDefaultModelForm(providerId);
    setSelectedModelProviderId(providerId);
    setForm({
      ...nextForm,
      is_default: models.length === 0,
    });
    setModelSuggestions(getModelProviderCatalogItem(providerId).models);
    setEditingModelId(null);
    setShowApiKey(false);
    setError("");
    setTestResult(null);
    setModelSaveMessage("");
  }

  function handleEditModel(model: ModelConfig) {
    void (async () => {
      try {
        const apiKey = await getModelApiKey(model.id);
        const provider = resolveModelProviderForEdit(model, providers);
        setForm({
          name: model.name,
          api_format: model.api_format === "anthropic" ? "anthropic" : "openai",
          base_url: model.base_url,
          model_name: model.model_name,
          api_key: apiKey,
          is_default: Boolean(model.is_default),
          supports_vision: Boolean(model.supports_vision),
        });
        setSelectedModelProviderId(provider.id);
        setEditingModelId(model.id);
        setShowApiKey(false);
        setError("");
        setTestResult(null);
        setModelSuggestions(provider.models);
      } catch (cause) {
        setError("加载配置失败: " + String(cause));
      }
    })();
  }

  function applyPreset(value: string) {
    const preset = getModelProviderCatalogItem(value);
    setForm((current) => ({
      ...current,
      ...getDefaultModelForm(preset.id),
      api_key: current.api_key,
      is_default: current.is_default,
    }));
    setSelectedModelProviderId(preset.id);
    setModelSuggestions(preset.models);
    setError("");
    setTestResult(null);
  }

  async function handleSave() {
    const validationError = validateModelForm(form);
    if (validationError) {
      setError(validationError);
      setTestResult(null);
      return;
    }

    setError("");
    setModelSaveMessage("");
    try {
      const isCreateMode = !editingModelId;
      const nextModelDefault = form.is_default || models.length === 0;
      const savedModelId = await saveModelConfig({
        id: editingModelId || undefined,
        isDefault: nextModelDefault,
        form,
      });
      const preferredProviderKey = getModelProviderCatalogItem(selectedModelProviderId).providerKey;
      await syncConnectionToRouting(
        {
          id: savedModelId,
          name: form.name.trim(),
          api_format: form.api_format,
          base_url: form.base_url.trim(),
          model_name: form.model_name.trim(),
          is_default: isCreateMode ? true : nextModelDefault,
          supports_vision: form.supports_vision,
        },
        form.api_key.trim(),
        preferredProviderKey,
      );
      if (form.supports_vision) {
        await syncCapabilityRouteToConnection("vision", {
          id: savedModelId,
          name: form.name.trim(),
          api_format: form.api_format,
          base_url: form.base_url.trim(),
          model_name: form.model_name.trim(),
          is_default: isCreateMode ? true : nextModelDefault,
          supports_vision: true,
        });
      }
      if (isCreateMode && nextModelDefault) {
        await setDefaultModel(savedModelId);
        setModelSaveMessage("已保存，并切换为默认模型");
      } else if (!isCreateMode && nextModelDefault) {
        await setDefaultModel(savedModelId);
        setModelSaveMessage("已保存，并设为默认模型");
      } else {
        setModelSaveMessage("已保存");
      }
      resetModelForm();
      await refreshModelData();
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function handleTest() {
    const validationError = validateModelForm(form);
    if (validationError) {
      setError(validationError);
      setTestResult(null);
      return;
    }
    setError("");
    setTesting(true);
    setTestResult(null);
    try {
      const result = await testModelConnection(form);
      setTestResult(result);
    } catch (cause) {
      setError(String(cause));
      setTestResult(null);
    } finally {
      setTesting(false);
    }
  }

  async function handleDelete(id: string) {
    await deleteModelConfig(id);
    if (editingModelId === id) {
      resetModelForm();
    }
    await refreshModelData();
  }

  async function handleSetDefaultModel(id: string) {
    await setDefaultModel(id);
    await refreshModelData();
  }

  return (
    <>
      <ModelsSettingsConfiguredList
        models={models}
        editingModelId={editingModelId}
        onSetDefault={(id) => void handleSetDefaultModel(id)}
        onEdit={handleEditModel}
        onDelete={(id) => void handleDelete(id)}
      />

      <div className="bg-white rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between mb-2">
          <div className="text-xs font-medium text-gray-500">{editingModelId ? "编辑模型" : "添加模型"}</div>
          {editingModelId && (
            <button onClick={() => resetModelForm()} className="text-xs text-gray-400 hover:text-gray-600">
              取消编辑
            </button>
          )}
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">快速选择模型服务</label>
          <select
            data-testid="settings-model-provider-preset"
            className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-400"
            value={selectedModelProviderId}
            onChange={(event) => applyPreset(event.target.value)}
          >
            {MODEL_PROVIDER_CATALOG.map((provider) => (
              <option key={provider.id} value={provider.id}>
                {provider.label}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">名称</label>
          <input
            data-testid="settings-model-provider-name"
            className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-400"
            value={form.name}
            onChange={(event) => setForm({ ...form, name: event.target.value })}
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">API 格式</label>
          <select className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm outline-none" value={form.api_format} disabled>
            <option value="openai">OpenAI 兼容</option>
            <option value="anthropic">Anthropic (Claude)</option>
          </select>
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Base URL</label>
          <input
            data-testid="settings-model-provider-base-url"
            className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-400"
            value={form.base_url}
            placeholder={selectedModelProvider.baseUrlPlaceholder}
            onChange={(event) => setForm({ ...form, base_url: event.target.value })}
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">模型名称</label>
          <input
            data-testid="settings-model-provider-model-name"
            className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-400"
            list="model-suggestions"
            value={form.model_name}
            placeholder={selectedModelProvider.modelNamePlaceholder}
            onChange={(event) => setForm({ ...form, model_name: event.target.value })}
          />
          {modelSuggestions.length > 0 && (
            <datalist id="model-suggestions">
              {modelSuggestions.map((model) => (
                <option key={model} value={model} />
              ))}
            </datalist>
          )}
        </div>
        <label className="flex items-start gap-3 rounded-xl border border-gray-200 bg-gray-50 px-3 py-3 text-sm text-gray-700">
          <input
            data-testid="settings-model-provider-is-default"
            type="checkbox"
            className="mt-0.5"
            checked={form.is_default}
            onChange={(event) => setForm({ ...form, is_default: event.target.checked })}
          />
          <span>
            <span className="block font-medium text-gray-800">设为默认对话模型</span>
            <span className="mt-1 block text-xs leading-5 text-gray-500">
              普通文字对话会优先使用这个模型。建议只保留一个默认对话模型。
            </span>
          </span>
        </label>
        <label className="flex items-start gap-3 rounded-xl border border-gray-200 bg-gray-50 px-3 py-3 text-sm text-gray-700">
          <input
            data-testid="settings-model-provider-supports-vision"
            type="checkbox"
            className="mt-0.5"
            checked={form.supports_vision}
            onChange={(event) => setForm({ ...form, supports_vision: event.target.checked })}
          />
          <span>
            <span className="block font-medium text-gray-800">用于图片理解</span>
            <span className="mt-1 block text-xs leading-5 text-gray-500">
              勾选后，保存时会自动同步为当前默认的“图片理解”模型，用于图片、截图和视觉类请求。
            </span>
          </span>
        </label>
        <div className="flex items-center justify-between rounded-xl border border-dashed border-gray-200 bg-white px-3 py-2 text-xs text-gray-500">
          <span>普通用户一般只需要在这里配置模型用途；超时、回退链等细项放在高级配置里。</span>
          {onOpenAdvancedRouting ? (
            <button
              type="button"
              onClick={onOpenAdvancedRouting}
              className="sm-btn rounded-lg border border-gray-200 bg-gray-50 px-3 py-1 text-xs text-gray-700 hover:bg-gray-100"
              aria-label="高级配置：图片理解路由"
            >
              高级配置：图片理解路由
            </button>
          ) : null}
        </div>
        <div className="rounded-2xl border border-gray-200 bg-gray-50 px-4 py-4">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <div className="text-sm font-medium text-gray-800">{selectedModelProvider.label}</div>
                <span className="inline-flex items-center rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-blue-700">
                  {selectedModelProvider.protocolLabel}
                </span>
              </div>
              <div className="mt-2 text-xs leading-5 text-gray-500">{selectedModelProvider.helper}</div>
            </div>
            {selectedModelProvider.officialConsoleUrl ? (
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() =>
                    openExternalUrl(selectedModelProvider.officialConsoleUrl ?? "").catch((cause) => {
                      setError("打开外部链接失败: " + String(cause));
                    })
                  }
                  className="sm-btn rounded-xl border border-gray-200 bg-white px-4 py-2 text-sm text-gray-700 hover:bg-gray-100"
                >
                  {selectedModelProvider.officialConsoleLabel ?? "获取 API Key"}
                </button>
                {selectedModelProvider.officialDocsUrl ? (
                  <button
                    type="button"
                    onClick={() =>
                      openExternalUrl(selectedModelProvider.officialDocsUrl ?? "").catch((cause) => {
                        setError("打开外部链接失败: " + String(cause));
                      })
                    }
                    className="sm-btn rounded-xl border border-transparent px-4 py-2 text-sm text-gray-500 hover:bg-white hover:text-gray-700"
                  >
                    {selectedModelProvider.officialDocsLabel ?? "查看文档"}
                  </button>
                ) : null}
              </div>
            ) : null}
          </div>
          {selectedModelProvider.isCustom ? (
            <div
              data-testid="settings-model-provider-custom-guidance"
              className="mt-3 rounded-2xl border border-dashed border-gray-200 bg-white px-3 py-3"
            >
              <div className="text-xs font-semibold text-gray-800">{selectedModelProvider.customGuidanceTitle}</div>
              <div className="mt-2 space-y-1.5 text-[12px] leading-5 text-gray-500">
                {selectedModelProvider.customGuidanceLines?.map((line) => (
                  <div key={line}>{line}</div>
                ))}
              </div>
            </div>
          ) : null}
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">API Key</label>
          <div className="relative">
            <input
              data-testid="settings-model-provider-api-key"
              className="w-full rounded-lg border border-gray-200 px-3 py-2 pr-10 text-sm outline-none focus:ring-2 focus:ring-blue-400"
              type={showApiKey ? "text" : "password"}
              value={form.api_key}
              onChange={(event) => setForm({ ...form, api_key: event.target.value })}
            />
            <button
              type="button"
              onClick={() => setShowApiKey((current) => !current)}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 p-1"
              title={showApiKey ? "隐藏" : "显示"}
            >
              {showApiKey ? <EyeOff className="h-4 w-4" aria-hidden="true" /> : <Eye className="h-4 w-4" aria-hidden="true" />}
            </button>
          </div>
        </div>
        {error && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{error}</div>}
        {testResult !== null && (
          <div
            className={
              "space-y-1 rounded px-2 py-2 text-xs " +
              (testResult.ok ? "bg-green-50 text-green-600" : "bg-red-50 text-red-600")
            }
          >
            <div className="font-medium">{testResult.ok ? "连接成功" : connectionTestDisplay?.title}</div>
            {!testResult.ok && connectionTestDisplay?.message ? <div>{connectionTestDisplay.message}</div> : null}
            {!testResult.ok && shouldShowConnectionRawMessage ? (
              <div className="whitespace-pre-wrap break-all rounded border border-red-200/80 bg-white/70 px-2 py-2 font-mono text-[11px] text-red-700/90">
                {connectionTestDisplay?.rawMessage}
              </div>
            ) : null}
          </div>
        )}
        {modelSaveMessage && (
          <div data-testid="settings-model-provider-save-hint" className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">
            {modelSaveMessage}
          </div>
        )}
        <div className="flex gap-2 pt-1">
          <button
            onClick={() => void handleTest()}
            disabled={testing}
            className="flex-1 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {testing ? "测试中..." : "测试连接"}
          </button>
          <button
            data-testid="settings-model-provider-save"
            onClick={() => void handleSave()}
            className="flex-1 bg-blue-500 hover:bg-blue-600 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {editingModelId ? "保存修改" : "保存"}
          </button>
        </div>
        <div className="text-xs text-gray-400">保存后会自动同步到默认路由和健康检查，无需重复配置。</div>
      </div>

      <ModelSetupDevTools
        show={showDevModelSetupTools}
        onResetFirstUseOnboarding={onDevResetFirstUseOnboarding}
        onOpenQuickModelSetup={onDevOpenQuickModelSetup}
      />
    </>
  );
}
