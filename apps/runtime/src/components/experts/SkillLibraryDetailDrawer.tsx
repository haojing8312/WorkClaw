import type { ClawhubLibraryItem, ClawhubSkillDetail } from "../../types";

interface SkillLibraryDetailViewModel {
  name: string;
  summary?: string | null;
  description?: string | null;
  tags: string[];
  stars?: number | null;
  downloads?: number | null;
  slug: string;
  author?: string | null;
  updatedAt?: string | null;
  githubUrl?: string | null;
  sourceUrl?: string | null;
}

interface SkillLibraryDetailDrawerProps {
  detailError: string;
  detailItem: ClawhubLibraryItem | null;
  detailLoading: boolean;
  detailNotFound: boolean;
  detailView: SkillLibraryDetailViewModel | null;
  installedSkillIds: Set<string>;
  installingSlug: string;
  renderDisplayText: (text: string) => string;
  onClose: () => void;
  onInstallRequest: (item: ClawhubLibraryItem) => void;
}

export function SkillLibraryDetailDrawer({
  detailError,
  detailItem,
  detailLoading,
  detailNotFound,
  detailView,
  installedSkillIds,
  installingSlug,
  renderDisplayText,
  onClose,
  onInstallRequest,
}: SkillLibraryDetailDrawerProps) {
  if (!detailItem) {
    return null;
  }

  const installed = installedSkillIds.has(`clawhub-${detailItem.slug}`);
  const isInstalling = installingSlug === detailItem.slug;

  return (
    <div className="fixed inset-0 z-40">
      <button aria-label="关闭技能详情" className="absolute inset-0 bg-black/25" onClick={onClose} />
      <aside
        className="absolute right-0 top-0 h-full w-full max-w-md bg-white border-l border-gray-200 shadow-xl p-5 overflow-y-auto"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <div className="text-base font-semibold text-gray-900">技能详情</div>
          <button
            className="h-7 px-2 rounded border border-gray-200 text-xs text-gray-600 hover:bg-gray-50"
            onClick={onClose}
          >
            关闭
          </button>
        </div>
        <div className="mt-4 space-y-3">
          {detailLoading && (
            <div className="text-xs text-gray-500 bg-gray-50 border border-gray-100 rounded px-2 py-1">
              详情加载中...
            </div>
          )}
          {detailNotFound && (
            <div className="text-xs text-amber-700 bg-amber-50 border border-amber-100 rounded px-2 py-1">
              暂无更详细信息，已展示基础信息。
            </div>
          )}
          {detailError && (
            <div className="text-xs text-red-600 bg-red-50 border border-red-100 rounded px-2 py-1">
              详情加载失败：{detailError}
            </div>
          )}
          <div>
            <div className="text-xs text-gray-500">名称</div>
            <div className="text-sm text-gray-900 mt-1">{renderDisplayText(detailView?.name ?? "")}</div>
          </div>
          <div>
            <div className="text-xs text-gray-500">简介</div>
            <div className="text-sm text-gray-700 mt-1 whitespace-pre-wrap">
              {detailView?.description ? renderDisplayText(detailView.description) : "暂无描述"}
            </div>
          </div>
          {detailView?.author && (
            <div>
              <div className="text-xs text-gray-500">作者</div>
              <div className="text-sm text-gray-800 mt-1">{detailView.author}</div>
            </div>
          )}
          {detailView?.updatedAt && (
            <div>
              <div className="text-xs text-gray-500">最近更新</div>
              <div className="text-sm text-gray-800 mt-1">{detailView.updatedAt}</div>
            </div>
          )}
          {(detailView?.githubUrl || detailView?.sourceUrl) && (
            <div className="space-y-2">
              {detailView.githubUrl && (
                <div>
                  <div className="text-xs text-gray-500">仓库链接</div>
                  <a
                    href={detailView.githubUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="text-xs text-blue-600 hover:text-blue-700 break-all"
                  >
                    {detailView.githubUrl}
                  </a>
                </div>
              )}
              {detailView.sourceUrl && (
                <div>
                  <div className="text-xs text-gray-500">来源链接</div>
                  <a
                    href={detailView.sourceUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="text-xs text-blue-600 hover:text-blue-700 break-all"
                  >
                    {detailView.sourceUrl}
                  </a>
                </div>
              )}
            </div>
          )}
          <div className="grid grid-cols-3 gap-2">
            <div className="rounded border border-gray-100 bg-gray-50 px-2 py-2">
              <div className="text-[11px] text-gray-500">下载</div>
              <div className="text-sm text-gray-800 mt-1">{detailView?.downloads ?? 0}</div>
            </div>
            <div className="rounded border border-gray-100 bg-gray-50 px-2 py-2">
              <div className="text-[11px] text-gray-500">星标</div>
              <div className="text-sm text-gray-800 mt-1">{detailView?.stars ?? 0}</div>
            </div>
            <div className="rounded border border-gray-100 bg-gray-50 px-2 py-2">
              <div className="text-[11px] text-gray-500">Slug</div>
              <div className="text-xs text-gray-800 mt-1 break-all">{detailView?.slug || ""}</div>
            </div>
          </div>
          <div>
            <div className="text-xs text-gray-500 mb-1">标签</div>
            <div className="flex flex-wrap gap-1">
              {(detailView?.tags ?? [])
                .filter((tag) => tag && tag.toLowerCase() !== "latest")
                .map((tag) => (
                  <span
                    key={`detail-${detailItem.slug}-${tag}`}
                    className="text-[10px] px-1.5 py-0.5 rounded bg-blue-50 text-blue-600 border border-blue-100"
                  >
                    {renderDisplayText(tag)}
                  </span>
                ))}
            </div>
          </div>
          <div className="pt-2">
            <button
              onClick={() => onInstallRequest(detailItem)}
              disabled={installed || isInstalling}
              className={`h-8 px-4 rounded text-xs transition-colors ${
                installed
                  ? "bg-emerald-50 text-emerald-700 border border-emerald-100"
                  : "bg-blue-500 hover:bg-blue-600 text-white"
              } disabled:opacity-80`}
            >
              {installed ? "已安装" : isInstalling ? "安装中..." : "安装该技能"}
            </button>
          </div>
        </div>
      </aside>
    </div>
  );
}
