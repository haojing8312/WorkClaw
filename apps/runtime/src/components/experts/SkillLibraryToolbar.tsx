interface SkillLibraryToolbarProps {
  hasPendingTranslations: boolean;
  immersiveEnabled: boolean;
  isTranslating: boolean;
  lastSyncedAtLabel: string;
  refreshingCatalog: boolean;
  selectedTag: string;
  tagOptions: string[];
  translationError: string | null;
  translationFallbackActive: boolean;
  onRefreshCatalog: () => void | Promise<void>;
  onSelectTag: (tag: string) => void;
  onTranslateNow: () => void | Promise<void>;
}

export function SkillLibraryToolbar({
  hasPendingTranslations,
  immersiveEnabled,
  isTranslating,
  lastSyncedAtLabel,
  refreshingCatalog,
  selectedTag,
  tagOptions,
  translationError,
  translationFallbackActive,
  onRefreshCatalog,
  onSelectTag,
  onTranslateNow,
}: SkillLibraryToolbarProps) {
  return (
    <>
      <div className="flex items-center justify-between gap-3 flex-wrap">
        <div className="flex flex-wrap items-center gap-2">
          {tagOptions.map((tag) => (
            <button
              key={tag}
              onClick={() => onSelectTag(tag)}
              className={`px-3 h-7 rounded-full text-xs border transition-colors ${
                selectedTag === tag
                  ? "bg-blue-500 text-white border-blue-500"
                  : "bg-white text-gray-600 border-gray-200 hover:border-blue-300"
              }`}
            >
              {tag}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-2 flex-wrap justify-end">
          <div className="text-[11px] text-gray-500">{lastSyncedAtLabel}</div>
          <button
            onClick={() => {
              void onRefreshCatalog();
            }}
            disabled={refreshingCatalog}
            className="h-7 px-3 rounded border border-gray-200 bg-white text-gray-700 text-xs hover:bg-gray-50 disabled:opacity-60"
          >
            {refreshingCatalog ? "刷新中..." : "刷新技能库"}
          </button>
          <button
            onClick={() => {
              void onTranslateNow();
            }}
            disabled={!immersiveEnabled || isTranslating || !hasPendingTranslations}
            className="h-7 px-3 rounded border border-blue-200 bg-blue-50 text-blue-700 text-xs hover:bg-blue-100 disabled:opacity-60"
          >
            {isTranslating
              ? "翻译中..."
              : translationFallbackActive || translationError
                ? "重试翻译"
                : "翻译本页"}
          </button>
        </div>
      </div>

      {immersiveEnabled && !isTranslating && translationFallbackActive && (
        <div className="text-xs text-amber-700 bg-amber-50 border border-amber-100 rounded-lg px-3 py-2">
          未命中可用翻译服务，当前展示原文。请检查默认模型与网络。
        </div>
      )}

      {translationError && (
        <div className="text-xs text-red-700 bg-red-50 border border-red-100 rounded-lg px-3 py-2">
          翻译失败：{translationError}
        </div>
      )}
    </>
  );
}
