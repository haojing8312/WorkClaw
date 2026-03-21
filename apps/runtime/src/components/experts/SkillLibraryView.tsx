import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ClawhubInstallRequest,
  ClawhubLibraryItem,
  ClawhubLibraryResponse,
  ClawhubSkillDetail,
  SkillhubCatalogSyncStatus,
} from "../../types";
import { RiskConfirmDialog } from "../RiskConfirmDialog";
import { useImmersiveTranslation } from "../../hooks/useImmersiveTranslation";

interface Props {
  installedSkillIds: Set<string>;
  onInstall: (request: ClawhubInstallRequest) => Promise<void>;
}

function formatLastSyncedAt(value?: string | null): string {
  if (!value) return "最近同步：尚未完成";
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return `最近同步：${value}`;
  return `最近同步：${parsed.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  })}`;
}

function isDetailNotFoundError(raw: string): boolean {
  const normalized = (raw || "").toLowerCase();
  return normalized.includes("404") || normalized.includes("not found");
}

export function SkillLibraryView({ installedSkillIds, onInstall }: Props) {
  const [items, setItems] = useState<ClawhubLibraryItem[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [initialized, setInitialized] = useState(false);
  const [error, setError] = useState<string>("");
  const [selectedTag, setSelectedTag] = useState<string>("全部");
  const [installingSlug, setInstallingSlug] = useState<string>("");
  const [pendingInstall, setPendingInstall] = useState<ClawhubLibraryItem | null>(null);
  const [detailItem, setDetailItem] = useState<ClawhubLibraryItem | null>(null);
  const [detailData, setDetailData] = useState<ClawhubSkillDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState("");
  const [detailNotFound, setDetailNotFound] = useState(false);
  const [lastSyncedAt, setLastSyncedAt] = useState<string | null>(null);
  const [refreshingCatalog, setRefreshingCatalog] = useState(false);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  async function loadMore(reset = false) {
    if (loading) return;
    if (!reset && initialized && nextCursor === null) return;
    setLoading(true);
    setError("");
    try {
      const result = await invoke<ClawhubLibraryResponse>("list_clawhub_library", {
        cursor: reset ? null : nextCursor,
        limit: 20,
        sort: "downloads",
      });
      setItems((prev) => {
        if (reset) return result.items ?? [];
        const existing = new Set(prev.map((i) => i.slug));
        const merged = [...prev];
        for (const item of result.items ?? []) {
          if (!existing.has(item.slug)) merged.push(item);
        }
        return merged;
      });
      setNextCursor(result.next_cursor ?? null);
      setLastSyncedAt(result.last_synced_at ?? null);
      setInitialized(true);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadMore(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const node = sentinelRef.current;
    if (!node) return;
    const observer = new IntersectionObserver(
      (entries) => {
        const hit = entries.some((entry) => entry.isIntersecting);
        if (hit) {
          void loadMore(false);
        }
      },
      { rootMargin: "300px 0px" }
    );
    observer.observe(node);
    return () => observer.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sentinelRef.current, nextCursor, loading, initialized]);

  useEffect(() => {
    if (!detailItem) {
      setDetailData(null);
      setDetailLoading(false);
      setDetailError("");
      setDetailNotFound(false);
      return;
    }
    let cancelled = false;
    setDetailLoading(true);
    setDetailError("");
    setDetailNotFound(false);
    setDetailData(null);
    void invoke<ClawhubSkillDetail>("get_clawhub_skill_detail", {
      slug: detailItem.slug,
    })
      .then((result) => {
        if (cancelled) return;
        setDetailData(result);
      })
      .catch((err) => {
        if (cancelled) return;
        const message = err instanceof Error ? err.message : String(err);
        if (isDetailNotFoundError(message)) {
          setDetailNotFound(true);
          setDetailError("");
          return;
        }
        setDetailError(message);
      })
      .finally(() => {
        if (cancelled) return;
        setDetailLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [detailItem?.slug]);

  const translationTexts = useMemo(
    () =>
      items.flatMap((item) => [item.name, item.summary, ...(item.tags ?? [])]),
    [items],
  );
  const {
    renderDisplayText,
    immersiveEnabled,
    isTranslating,
    translationFallbackActive,
    translationError,
    hasPendingTranslations,
    translateNow,
  } =
    useImmersiveTranslation(translationTexts, {
      scene: "experts-library",
      batchSize: 40,
    });

  const tagOptions = useMemo(() => {
    const counter = new Map<string, number>();
    for (const item of items) {
      for (const tag of item.tags ?? []) {
        if (!tag || tag.toLowerCase() === "latest") continue;
        counter.set(tag, (counter.get(tag) ?? 0) + 1);
      }
    }
    return [
      "全部",
      ...Array.from(counter.entries())
        .sort((a, b) => b[1] - a[1])
        .map(([tag]) => tag),
    ];
  }, [items]);

  const visibleItems = useMemo(() => {
    if (selectedTag === "全部") return items;
    return items.filter((item) => (item.tags ?? []).includes(selectedTag));
  }, [items, selectedTag]);

  const detailView = useMemo(() => {
    if (!detailItem) return null;
    return {
      name: detailData?.name || detailItem.name,
      summary: detailData?.summary || detailItem.summary,
      description: detailData?.description || detailData?.summary || detailItem.summary,
      tags: detailData && detailData.tags.length > 0 ? detailData.tags : detailItem.tags ?? [],
      stars: detailData?.stars ?? detailItem.stars,
      downloads: detailData?.downloads ?? detailItem.downloads,
      slug: detailData?.slug || detailItem.slug,
      author: detailData?.author,
      updatedAt: detailData?.updated_at,
      githubUrl: detailData?.github_url,
      sourceUrl: detailData?.source_url,
    };
  }, [detailData, detailItem]);

  async function handleConfirmInstall() {
    if (!pendingInstall || installingSlug) return;
    setInstallingSlug(pendingInstall.slug);
    setError("");
    try {
      await onInstall({
        slug: pendingInstall.slug,
        githubUrl:
          pendingInstall.github_url ??
          (detailView?.slug === pendingInstall.slug ? detailView.githubUrl ?? null : null),
        sourceUrl:
          pendingInstall.source_url ??
          (detailView?.slug === pendingInstall.slug ? detailView.sourceUrl ?? null : null),
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setInstallingSlug("");
      setPendingInstall(null);
    }
  }

  function handleCancelInstall() {
    if (installingSlug) return;
    setPendingInstall(null);
  }

  async function handleRefreshCatalog() {
    if (refreshingCatalog) return;
    setRefreshingCatalog(true);
    setError("");
    try {
      const result = await invoke<SkillhubCatalogSyncStatus>("sync_skillhub_catalog", {
        force: true,
      });
      setLastSyncedAt(result.last_synced_at ?? null);
      await loadMore(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshingCatalog(false);
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3 flex-wrap">
        <div className="flex flex-wrap items-center gap-2">
          {tagOptions.map((tag) => (
            <button
              key={tag}
              onClick={() => setSelectedTag(tag)}
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
          <div className="text-[11px] text-gray-500">{formatLastSyncedAt(lastSyncedAt)}</div>
          <button
            onClick={() => {
              void handleRefreshCatalog();
            }}
            disabled={refreshingCatalog}
            className="h-7 px-3 rounded border border-gray-200 bg-white text-gray-700 text-xs hover:bg-gray-50 disabled:opacity-60"
          >
            {refreshingCatalog ? "刷新中..." : "刷新技能库"}
          </button>
          <button
            onClick={() => {
              void translateNow();
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

      {error && <div className="text-sm text-red-500">{error}</div>}

      {visibleItems.length === 0 && initialized ? (
        <div className="rounded-xl border border-dashed border-gray-200 bg-white px-4 py-10 text-center text-sm text-gray-400">
          当前分类暂无技能
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
          {visibleItems.map((item) => {
            const installed = installedSkillIds.has(`clawhub-${item.slug}`);
            const isInstalling = installingSlug === item.slug;
            return (
              <div
                key={item.slug}
                role="button"
                tabIndex={0}
                onClick={() => setDetailItem(item)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    setDetailItem(item);
                  }
                }}
                className="bg-white border border-gray-200 rounded-xl p-4 cursor-pointer hover:border-blue-300 transition-colors"
              >
                <div className="flex items-center justify-between gap-2 mb-2">
                  <div className="text-sm font-medium text-gray-800 truncate">
                    {renderDisplayText(item.name)}
                  </div>
                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-50 text-gray-500 border border-gray-100">
                    ★ {item.stars}
                  </span>
                </div>
                <div className="text-xs text-gray-500 line-clamp-3 min-h-[48px]">
                  {item.summary ? renderDisplayText(item.summary) : "暂无描述"}
                </div>
                <div className="mt-2 flex flex-wrap gap-1 min-h-[20px]">
                  {(item.tags ?? [])
                    .filter((t) => t && t.toLowerCase() !== "latest")
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
                      setError("");
                      setPendingInstall(item);
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
      )}

      <div ref={sentinelRef} className="h-8 flex items-center justify-center text-xs text-gray-400">
        {loading ? "加载中..." : nextCursor ? "下拉加载更多" : initialized ? "已加载全部" : ""}
      </div>
      <RiskConfirmDialog
        open={Boolean(pendingInstall)}
        level="medium"
        title="安装技能"
        summary={pendingInstall ? `确定安装「${renderDisplayText(pendingInstall.name)}」吗？` : "确定安装该技能吗？"}
        impact={pendingInstall ? `slug: ${pendingInstall.slug}` : undefined}
        irreversible={false}
        confirmLabel="确认安装"
        cancelLabel="取消"
        loading={Boolean(installingSlug)}
        onConfirm={handleConfirmInstall}
        onCancel={handleCancelInstall}
      />
      {detailItem && (
        <div className="fixed inset-0 z-40">
          <button
            aria-label="关闭技能详情"
            className="absolute inset-0 bg-black/25"
            onClick={() => setDetailItem(null)}
          />
          <aside
            className="absolute right-0 top-0 h-full w-full max-w-md bg-white border-l border-gray-200 shadow-xl p-5 overflow-y-auto"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="flex items-center justify-between">
              <div className="text-base font-semibold text-gray-900">技能详情</div>
              <button
                className="h-7 px-2 rounded border border-gray-200 text-xs text-gray-600 hover:bg-gray-50"
                onClick={() => setDetailItem(null)}
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
                  {detailView?.githubUrl && (
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
                  {detailView?.sourceUrl && (
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
                  onClick={() => {
                    setError("");
                    setPendingInstall(detailItem);
                  }}
                  disabled={
                    installedSkillIds.has(`clawhub-${detailItem.slug}`) || installingSlug === detailItem.slug
                  }
                  className={`h-8 px-4 rounded text-xs transition-colors ${
                    installedSkillIds.has(`clawhub-${detailItem.slug}`)
                      ? "bg-emerald-50 text-emerald-700 border border-emerald-100"
                      : "bg-blue-500 hover:bg-blue-600 text-white"
                  } disabled:opacity-80`}
                >
                  {installedSkillIds.has(`clawhub-${detailItem.slug}`)
                    ? "已安装"
                    : installingSlug === detailItem.slug
                      ? "安装中..."
                      : "安装该技能"}
                </button>
              </div>
            </div>
          </aside>
        </div>
      )}
    </div>
  );
}
