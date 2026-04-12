import type { ModelConfig, RuntimePreferences } from "../../../types";

interface DesktopLanguageSectionProps {
  inputCls: string;
  labelCls: string;
  models: ModelConfig[];
  runtimePreferences: RuntimePreferences;
  runtimePreferencesError: string;
  runtimePreferencesSaveState: "idle" | "saving" | "saved" | "error";
  onRuntimePreferencesChange: (
    updater: (prev: RuntimePreferences) => RuntimePreferences,
  ) => void;
  onSaveRuntimePreferences: () => void | Promise<void>;
}

export function DesktopLanguageSection({
  inputCls,
  labelCls,
  models,
  runtimePreferences,
  runtimePreferencesError,
  runtimePreferencesSaveState,
  onRuntimePreferencesChange,
  onSaveRuntimePreferences,
}: DesktopLanguageSectionProps) {
  return (
    <div className="bg-white rounded-lg p-4 space-y-3">
      <div className="text-xs font-medium text-gray-500">语言与沉浸式翻译</div>
      <div>
        <label className={labelCls}>默认语言</label>
        <select
          aria-label="默认语言"
          className={inputCls}
          value={runtimePreferences.default_language}
          onChange={(e) =>
            onRuntimePreferencesChange((prev) => ({ ...prev, default_language: e.target.value }))
          }
        >
          <option value="zh-CN">简体中文 (zh-CN)</option>
          <option value="en-US">English (en-US)</option>
        </select>
      </div>
      <label className="flex items-center gap-2 text-xs text-gray-600">
        <input
          aria-label="启用沉浸式翻译"
          type="checkbox"
          checked={runtimePreferences.immersive_translation_enabled}
          onChange={(e) =>
            onRuntimePreferencesChange((prev) => ({
              ...prev,
              immersive_translation_enabled: e.target.checked,
            }))
          }
        />
        启用沉浸式翻译
      </label>
      <div>
        <label className={labelCls}>显示模式</label>
        <select
          aria-label="翻译显示模式"
          className={inputCls}
          value={runtimePreferences.immersive_translation_display}
          onChange={(e) =>
            onRuntimePreferencesChange((prev) => ({
              ...prev,
              immersive_translation_display:
                e.target.value === "bilingual_inline" ? "bilingual_inline" : "translated_only",
            }))
          }
        >
          <option value="translated_only">仅译文</option>
          <option value="bilingual_inline">双语对照</option>
        </select>
      </div>
      <div>
        <label className={labelCls}>翻译触发方式</label>
        <select
          aria-label="翻译触发方式"
          className={inputCls}
          value={runtimePreferences.immersive_translation_trigger}
          onChange={(e) =>
            onRuntimePreferencesChange((prev) => ({
              ...prev,
              immersive_translation_trigger: e.target.value === "manual" ? "manual" : "auto",
            }))
          }
        >
          <option value="auto">自动翻译（默认）</option>
          <option value="manual">手动触发</option>
        </select>
      </div>
      <div>
        <label className={labelCls}>翻译引擎策略</label>
        <select
          aria-label="翻译引擎策略"
          className={inputCls}
          value={runtimePreferences.translation_engine}
          onChange={(e) =>
            onRuntimePreferencesChange((prev) => ({
              ...prev,
              translation_engine:
                e.target.value === "model_only" || e.target.value === "free_only"
                  ? e.target.value
                  : "model_then_free",
              translation_model_id: e.target.value === "free_only" ? "" : prev.translation_model_id,
            }))
          }
        >
          <option value="model_then_free">优先模型，失败回退免费翻译（推荐）</option>
          <option value="model_only">仅使用翻译模型</option>
          <option value="free_only">仅使用免费翻译</option>
        </select>
      </div>
      <div>
        <label className={labelCls}>翻译模型</label>
        <select
          aria-label="翻译模型"
          className={inputCls}
          disabled={runtimePreferences.translation_engine === "free_only"}
          value={runtimePreferences.translation_model_id}
          onChange={(e) =>
            onRuntimePreferencesChange((prev) => ({
              ...prev,
              translation_model_id: e.target.value,
            }))
          }
        >
          <option value="">跟随默认模型</option>
          {models.map((model) => (
            <option key={model.id} value={model.id}>
              {model.name || model.model_name || model.id}
            </option>
          ))}
        </select>
      </div>
      {runtimePreferences.translation_engine !== "free_only" && models.length === 0 && (
        <div className="bg-amber-50 text-amber-700 text-xs px-2 py-1 rounded">
          当前未配置可用模型。翻译会尝试免费翻译接口；若策略为“仅使用翻译模型”则可能失败。
        </div>
      )}
      {runtimePreferences.translation_engine === "model_only" && models.length === 0 && (
        <div className="bg-red-50 text-red-700 text-xs px-2 py-1 rounded">
          已选择仅模型翻译，但当前无可用模型配置。建议切换到“优先模型，失败回退免费翻译”。
        </div>
      )}
      {runtimePreferences.translation_model_id &&
        !models.some((model) => model.id === runtimePreferences.translation_model_id) && (
          <div className="bg-amber-50 text-amber-700 text-xs px-2 py-1 rounded">
            选中的翻译模型不存在，将自动跟随默认模型或回退免费翻译。
          </div>
        )}
      {runtimePreferencesError && (
        <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{runtimePreferencesError}</div>
      )}
      {runtimePreferencesSaveState === "saved" && (
        <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">已保存</div>
      )}
      <button
        onClick={onSaveRuntimePreferences}
        disabled={runtimePreferencesSaveState === "saving"}
        className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
      >
        {runtimePreferencesSaveState === "saving" ? "保存中..." : "保存语言与翻译设置"}
      </button>
    </div>
  );
}
