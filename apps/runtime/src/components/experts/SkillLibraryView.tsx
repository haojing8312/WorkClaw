import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ClawhubLibraryItem } from "../../types";
import { RiskConfirmDialog } from "../RiskConfirmDialog";

interface Props {
  installedSkillIds: Set<string>;
  onInstall: (slug: string) => Promise<void>;
}

interface LibraryResponse {
  items: ClawhubLibraryItem[];
  next_cursor?: string | null;
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
  const [textZhMap, setTextZhMap] = useState<Record<string, string>>({});
  const [translating, setTranslating] = useState(false);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  async function loadMore(reset = false) {
    if (loading) return;
    if (!reset && initialized && nextCursor === null) return;
    setLoading(true);
    setError("");
    try {
      const result = await invoke<LibraryResponse>("list_clawhub_library", {
        cursor: reset ? null : nextCursor,
        limit: 20,
        sort: "updated",
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
    const candidates = Array.from(
      new Set(
        items
          .flatMap((item) => [item.name, item.summary, ...(item.tags ?? [])])
          .map((v) => v?.trim())
          .filter((v): v is string => Boolean(v) && !textZhMap[v!])
      )
    );

    if (candidates.length === 0 || translating) return;
    let cancelled = false;
    setTranslating(true);
    void (async () => {
      try {
        const limited = candidates.slice(0, 80);
        const translated = await invoke<string[]>("translate_clawhub_texts", {
          texts: limited,
        });
        if (cancelled) return;
        const next: Record<string, string> = {};
        for (let i = 0; i < limited.length; i += 1) {
          next[limited[i]] = translated[i] ?? limited[i];
        }
        setTextZhMap((prev) => ({ ...prev, ...next }));
      } catch {
        // silently fallback to original text
      } finally {
        if (!cancelled) setTranslating(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [items, textZhMap, translating]);

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

  async function handleConfirmInstall() {
    if (!pendingInstall || installingSlug) return;
    setInstallingSlug(pendingInstall.slug);
    setError("");
    try {
      await onInstall(pendingInstall.slug);
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

  return (
    <div className="space-y-4">
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
              <div key={item.slug} className="bg-white border border-gray-200 rounded-xl p-4">
                <div className="flex items-center justify-between gap-2 mb-2">
                  <div className="text-sm font-medium text-gray-800 truncate">
                    {textZhMap[item.name] ?? item.name}
                  </div>
                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-50 text-gray-500 border border-gray-100">
                    ★ {item.stars}
                  </span>
                </div>
                <div className="text-xs text-gray-500 line-clamp-3 min-h-[48px]">
                  {item.summary ? textZhMap[item.summary] ?? item.summary : "暂无描述"}
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
                        {textZhMap[tag] ?? tag}
                      </span>
                    ))}
                </div>
                <div className="text-[11px] text-gray-400 mt-2">下载 {item.downloads}</div>
                <div className="mt-3">
                  <button
                    onClick={() => {
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
        summary={pendingInstall ? `确定安装「${textZhMap[pendingInstall.name] ?? pendingInstall.name}」吗？` : "确定安装该技能吗？"}
        impact={pendingInstall ? `slug: ${pendingInstall.slug}` : undefined}
        irreversible={false}
        confirmLabel="确认安装"
        cancelLabel="取消"
        loading={Boolean(installingSlug)}
        onConfirm={handleConfirmInstall}
        onCancel={handleCancelInstall}
      />
    </div>
  );
}
