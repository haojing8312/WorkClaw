import type {
  CapabilityRouteTemplateInfo,
  CapabilityRoutingPolicy,
  ProviderConfig,
} from "../../types";

interface RouteLogRow {
  provider_id: string;
  model: string;
}

interface CapabilityRoutingSectionProps {
  capabilities: Array<{ label: string; value: string }>;
  chatFallbackRows: RouteLogRow[];
  chatPrimaryModels: string[];
  chatRoutingPolicy: CapabilityRoutingPolicy;
  inputCls: string;
  labelCls: string;
  policyError: string;
  policySaveState: "idle" | "saving" | "saved" | "error";
  providers: ProviderConfig[];
  routeTemplates: CapabilityRouteTemplateInfo[];
  selectedCapability: string;
  selectedRouteTemplateId: string;
  onAddFallbackRow: () => void;
  onApplyRecommendedDefaults: () => void;
  onApplyRouteTemplate: () => void | Promise<void>;
  onCapabilityChange: (capability: string) => void;
  onPrimaryModelChange: (model: string) => void;
  onPrimaryProviderChange: (providerId: string) => void;
  onRemoveFallbackRow: (index: number) => void;
  onSaveChatPolicy: () => void | Promise<void>;
  onSelectedRouteTemplateIdChange: (templateId: string) => void;
  onTimeoutChange: (timeoutMs: number) => void;
  onRetryCountChange: (retryCount: number) => void;
  onToggleEnabled: (enabled: boolean) => void;
  onUpdateFallbackRow: (index: number, patch: Partial<RouteLogRow>) => void;
}

export function CapabilityRoutingSection({
  capabilities,
  chatFallbackRows,
  chatPrimaryModels,
  chatRoutingPolicy,
  inputCls,
  labelCls,
  policyError,
  policySaveState,
  providers,
  routeTemplates,
  selectedCapability,
  selectedRouteTemplateId,
  onAddFallbackRow,
  onApplyRecommendedDefaults,
  onApplyRouteTemplate,
  onCapabilityChange,
  onPrimaryModelChange,
  onPrimaryProviderChange,
  onRemoveFallbackRow,
  onSaveChatPolicy,
  onSelectedRouteTemplateIdChange,
  onTimeoutChange,
  onRetryCountChange,
  onToggleEnabled,
  onUpdateFallbackRow,
}: CapabilityRoutingSectionProps) {
  return (
    <div className="bg-white rounded-lg p-4 space-y-3">
      <div className="text-xs font-medium text-gray-500 mb-2">能力路由</div>
      <div>
        <label className={labelCls}>能力类型</label>
        <select
          className={inputCls}
          value={selectedCapability}
          onChange={(e) => onCapabilityChange(e.target.value)}
        >
          {capabilities.map((capability) => (
            <option key={capability.value} value={capability.value}>
              {capability.label}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label className={labelCls}>主连接</label>
        <select
          className={inputCls}
          value={chatRoutingPolicy.primary_provider_id}
          onChange={(e) => onPrimaryProviderChange(e.target.value)}
        >
          <option value="">请选择</option>
          {providers.map((provider) => (
            <option key={provider.id} value={provider.id}>
              {provider.display_name}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label className={labelCls}>主模型</label>
        <input
          className={inputCls}
          list="chat-primary-models"
          value={chatRoutingPolicy.primary_model}
          onChange={(e) => onPrimaryModelChange(e.target.value)}
          placeholder="例如: deepseek-chat / qwen3.5-plus / kimi-k2"
        />
        {chatPrimaryModels.length > 0 && (
          <datalist id="chat-primary-models">
            {chatPrimaryModels.map((model) => (
              <option key={model} value={model} />
            ))}
          </datalist>
        )}
      </div>
      <div>
        <label className={labelCls}>Fallback 链</label>
        <div className="space-y-2">
          {chatFallbackRows.map((row, index) => (
            <div key={index} className="grid grid-cols-[1fr_1fr_auto] gap-2">
              <select
                className={inputCls}
                value={row.provider_id}
                onChange={(e) => onUpdateFallbackRow(index, { provider_id: e.target.value })}
              >
                <option value="">选择连接</option>
                {providers.map((provider) => (
                  <option key={provider.id} value={provider.id}>
                    {provider.display_name}
                  </option>
                ))}
              </select>
              <input
                className={inputCls}
                value={row.model}
                onChange={(e) => onUpdateFallbackRow(index, { model: e.target.value })}
                placeholder="模型名"
              />
              <button
                onClick={() => onRemoveFallbackRow(index)}
                className="px-2 text-xs text-red-500 hover:text-red-600"
              >
                删除
              </button>
            </div>
          ))}
          <button onClick={onAddFallbackRow} className="text-xs text-blue-500 hover:text-blue-600">
            + 添加回退节点
          </button>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div>
          <label className={labelCls}>超时(ms)</label>
          <input
            className={inputCls}
            type="number"
            value={chatRoutingPolicy.timeout_ms}
            onChange={(e) => onTimeoutChange(Number(e.target.value || 60000))}
          />
        </div>
        <div>
          <label className={labelCls}>重试次数</label>
          <input
            className={inputCls}
            type="number"
            value={chatRoutingPolicy.retry_count}
            onChange={(e) => onRetryCountChange(Number(e.target.value || 0))}
          />
        </div>
      </div>
      <button
        onClick={onApplyRecommendedDefaults}
        className="w-full bg-gray-100 hover:bg-gray-200 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
      >
        应用推荐超时/重试配置
      </button>
      <div className="grid grid-cols-[1fr_auto] gap-2">
        <select
          className={inputCls}
          value={selectedRouteTemplateId}
          onChange={(e) => onSelectedRouteTemplateIdChange(e.target.value)}
        >
          {routeTemplates.length === 0 && <option value="">暂无模板</option>}
          {routeTemplates.map((template) => (
            <option key={`${template.template_id}-${template.capability}`} value={template.template_id}>
              {template.name}
            </option>
          ))}
        </select>
        <button
          onClick={onApplyRouteTemplate}
          disabled={!selectedRouteTemplateId}
          className="bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm px-3 py-1.5 rounded-lg transition-all active:scale-[0.97]"
        >
          应用模板
        </button>
      </div>
      <label className="flex items-center gap-2 text-xs text-gray-600">
        <input
          type="checkbox"
          checked={chatRoutingPolicy.enabled}
          onChange={(e) => onToggleEnabled(e.target.checked)}
        />
        启用当前能力路由
      </label>
      {policyError && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{policyError}</div>}
      {policySaveState === "saved" && (
        <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">已保存</div>
      )}
      <button
        onClick={onSaveChatPolicy}
        disabled={policySaveState === "saving"}
        className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
      >
        {policySaveState === "saving" ? "保存中..." : "保存能力路由策略"}
      </button>
    </div>
  );
}
