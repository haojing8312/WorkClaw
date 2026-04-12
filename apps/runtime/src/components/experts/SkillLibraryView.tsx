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
import { SkillLibraryDetailDrawer } from "./SkillLibraryDetailDrawer";
import { SkillLibraryGrid } from "./SkillLibraryGrid";
import { SkillLibraryToolbar } from "./SkillLibraryToolbar";

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
      <SkillLibraryToolbar
        hasPendingTranslations={hasPendingTranslations}
        immersiveEnabled={immersiveEnabled}
        isTranslating={isTranslating}
        lastSyncedAtLabel={formatLastSyncedAt(lastSyncedAt)}
        refreshingCatalog={refreshingCatalog}
        selectedTag={selectedTag}
        tagOptions={tagOptions}
        translationError={translationError}
        translationFallbackActive={translationFallbackActive}
        onRefreshCatalog={handleRefreshCatalog}
        onSelectTag={setSelectedTag}
        onTranslateNow={translateNow}
      />

      {error && <div className="text-sm text-red-500">{error}</div>}

      <SkillLibraryGrid
        initialized={initialized}
        installedSkillIds={installedSkillIds}
        installingSlug={installingSlug}
        renderDisplayText={renderDisplayText}
        visibleItems={visibleItems}
        onInstallRequest={(item) => {
          setError("");
          setPendingInstall(item);
        }}
        onOpenDetail={setDetailItem}
      />

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
      <SkillLibraryDetailDrawer
        detailError={detailError}
        detailItem={detailItem}
        detailLoading={detailLoading}
        detailNotFound={detailNotFound}
        detailView={detailView}
        installedSkillIds={installedSkillIds}
        installingSlug={installingSlug}
        renderDisplayText={renderDisplayText}
        onClose={() => setDetailItem(null)}
        onInstallRequest={(item) => {
          setError("");
          setPendingInstall(item);
        }}
      />
    </div>
  );
}
