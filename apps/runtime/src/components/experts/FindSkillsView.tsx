import { FormEvent, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ClawhubSkillRecommendation } from "../../types";

interface Props {
  installedSkillIds: Set<string>;
  onInstall: (slug: string) => Promise<void>;
}

interface Turn {
  id: string;
  query: string;
  recommendations: ClawhubSkillRecommendation[];
  error?: string;
}

export function FindSkillsView({ installedSkillIds, onInstall }: Props) {
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [installingSlug, setInstallingSlug] = useState("");
  const [turns, setTurns] = useState<Turn[]>([]);

  const empty = useMemo(() => turns.length === 0, [turns.length]);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    const q = query.trim();
    if (!q || loading) return;
    setLoading(true);
    setQuery("");
    try {
      const recommendations = await invoke<ClawhubSkillRecommendation[]>(
        "recommend_clawhub_skills",
        { query: q, limit: 5 }
      );
      setTurns((prev) => [
        {
          id: `${Date.now()}`,
          query: q,
          recommendations,
        },
        ...prev,
      ]);
    } catch (err: unknown) {
      setTurns((prev) => [
        {
          id: `${Date.now()}`,
          query: q,
          recommendations: [],
          error: String(err),
        },
        ...prev,
      ]);
    } finally {
      setLoading(false);
    }
  }

  async function handleInstall(slug: string) {
    setInstallingSlug(slug);
    try {
      await onInstall(slug);
    } finally {
      setInstallingSlug("");
    }
  }

  return (
    <div className="space-y-4">
      <div className="rounded-xl border border-blue-100 bg-blue-50 p-4">
        <div className="text-sm font-medium text-blue-800">找技能助手</div>
        <div className="text-xs text-blue-700 mt-1">
          直接描述你的需求，我会推荐可安装的开源技能。
        </div>
        <form onSubmit={handleSubmit} className="mt-3 flex gap-2">
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="例如：我想做短视频脚本，最好中文场景可用"
            className="flex-1 bg-white border border-blue-200 rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-400 focus:ring-1 focus:ring-blue-400"
          />
          <button
            type="submit"
            disabled={loading}
            className="px-4 rounded bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-sm"
          >
            {loading ? "匹配中..." : "找技能"}
          </button>
        </form>
      </div>

      {empty ? (
        <div className="rounded-xl border border-dashed border-gray-200 bg-white px-4 py-10 text-center text-sm text-gray-400">
          还没有检索记录，先输入你的需求试试。
        </div>
      ) : (
        <div className="space-y-3">
          {turns.map((turn) => (
            <div key={turn.id} className="rounded-xl border border-gray-200 bg-white p-4">
              <div className="text-xs text-gray-500">你的需求</div>
              <div className="text-sm text-gray-800 mt-1">{turn.query}</div>

              {turn.error ? (
                <div className="text-sm text-red-500 mt-3">检索失败：{turn.error}</div>
              ) : turn.recommendations.length === 0 ? (
                <div className="text-sm text-gray-500 mt-3">未找到匹配技能，建议换个关键词重试。</div>
              ) : (
                <div className="mt-3 grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
                  {turn.recommendations.map((item) => {
                    const installed = installedSkillIds.has(`clawhub-${item.slug}`);
                    const isInstalling = installingSlug === item.slug;
                    return (
                      <div key={`${turn.id}-${item.slug}`} className="border border-gray-100 rounded-lg p-3">
                        <div className="flex items-center justify-between gap-2">
                          <div className="text-sm font-medium text-gray-800 truncate">{item.name}</div>
                          <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-50 text-gray-500 border border-gray-100">
                            ★ {item.stars}
                          </span>
                        </div>
                        <div className="text-xs text-gray-500 mt-1 line-clamp-2 min-h-[32px]">
                          {item.description || "暂无描述"}
                        </div>
                        <div className="text-[11px] text-blue-700 mt-2">{item.reason}</div>
                        <div className="mt-3">
                          <button
                            onClick={() => void handleInstall(item.slug)}
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
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
