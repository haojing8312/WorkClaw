import type { ClawhubLibraryItem } from "../../types";

interface SkillLibraryGridProps {
  initialized: boolean;
  installedSkillIds: Set<string>;
  installingSlug: string;
  renderDisplayText: (text: string) => string;
  visibleItems: ClawhubLibraryItem[];
  onInstallRequest: (item: ClawhubLibraryItem) => void;
  onOpenDetail: (item: ClawhubLibraryItem) => void;
}

export function SkillLibraryGrid({
  initialized,
  installedSkillIds,
  installingSlug,
  renderDisplayText,
  visibleItems,
  onInstallRequest,
  onOpenDetail,
}: SkillLibraryGridProps) {
  if (visibleItems.length === 0 && initialized) {
    return (
      <div className="rounded-xl border border-dashed border-gray-200 bg-white px-4 py-10 text-center text-sm text-gray-400">
        当前分类暂无技能
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
      {visibleItems.map((item) => {
        const installed = installedSkillIds.has(`clawhub-${item.slug}`);
        const isInstalling = installingSlug === item.slug;
        return (
          <div
            key={item.slug}
            role="button"
            tabIndex={0}
            onClick={() => onOpenDetail(item)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onOpenDetail(item);
              }
            }}
            className="bg-white border border-gray-200 rounded-xl p-4 cursor-pointer hover:border-blue-300 transition-colors"
          >
            <div className="flex items-center justify-between gap-2 mb-2">
              <div className="text-sm font-medium text-gray-800 truncate">{renderDisplayText(item.name)}</div>
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-50 text-gray-500 border border-gray-100">
                ★ {item.stars}
              </span>
            </div>
            <div className="text-xs text-gray-500 line-clamp-3 min-h-[48px]">
              {item.summary ? renderDisplayText(item.summary) : "暂无描述"}
            </div>
            <div className="mt-2 flex flex-wrap gap-1 min-h-[20px]">
              {(item.tags ?? [])
                .filter((tag) => tag && tag.toLowerCase() !== "latest")
                .slice(0, 4)
                .map((tag) => (
                  <span
                    key={`${item.slug}-${tag}`}
                    className="text-[10px] px-1.5 py-0.5 rounded bg-blue-50 text-blue-600 border border-blue-100"
                  >
                    {renderDisplayText(tag)}
                  </span>
                ))}
            </div>
            <div className="text-[11px] text-gray-400 mt-2">下载 {item.downloads}</div>
            <div className="mt-3">
              <button
                onClick={(event) => {
                  event.stopPropagation();
                  onInstallRequest(item);
                }}
                disabled={installed || isInstalling}
                className={`h-7 px-3 rounded text-xs transition-colors ${
                  installed
                    ? "bg-emerald-50 text-emerald-700 border border-emerald-100"
                    : "bg-blue-500 hover:bg-blue-600 text-white"
                } disabled:opacity-80`}
              >
                {installed ? "已安装" : isInstalling ? "安装中..." : "安装"}
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
