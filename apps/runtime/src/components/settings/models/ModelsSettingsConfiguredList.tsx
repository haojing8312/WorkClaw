import type { ModelConfig } from "../../../types";

type ModelsSettingsConfiguredListProps = {
  models: ModelConfig[];
  editingModelId: string | null;
  onSetDefault: (id: string) => void;
  onEdit: (model: ModelConfig) => void;
  onDelete: (id: string) => void;
};

export function ModelsSettingsConfiguredList({
  models,
  editingModelId,
  onSetDefault,
  onEdit,
  onDelete,
}: ModelsSettingsConfiguredListProps) {
  if (models.length === 0) return null;

  return (
    <div className="mb-6 space-y-2">
      <div className="text-xs text-gray-500 mb-2">已配置模型</div>
      {models.map((model) => (
        <div
          key={model.id}
          className={
            "flex items-center justify-between bg-white rounded-lg px-4 py-2.5 text-sm border transition-colors " +
            (editingModelId === model.id ? "border-blue-400 ring-1 ring-blue-400" : "border-transparent hover:border-gray-200")
          }
        >
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="font-medium text-gray-800">{model.name}</span>
              {model.is_default && (
                <span className="text-[10px] bg-blue-500 text-white px-1.5 py-0.5 rounded">默认</span>
              )}
              {model.supports_vision && (
                <span className="text-[10px] bg-emerald-100 text-emerald-700 px-1.5 py-0.5 rounded">图片理解</span>
              )}
            </div>
            <div className="text-xs text-gray-400 mt-0.5 truncate">
              {model.model_name} · {model.api_format === "anthropic" ? "Anthropic" : "OpenAI 兼容"} · {model.base_url}
            </div>
          </div>
          <div className="flex items-center gap-2 flex-shrink-0 ml-3">
            {!model.is_default && (
              <button onClick={() => onSetDefault(model.id)} className="text-blue-400 hover:text-blue-500 text-xs">
                设为默认
              </button>
            )}
            <button onClick={() => onEdit(model)} className="text-blue-500 hover:text-blue-600 text-xs">
              编辑
            </button>
            <button onClick={() => onDelete(model.id)} className="text-red-400 hover:text-red-500 text-xs">
              删除
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
