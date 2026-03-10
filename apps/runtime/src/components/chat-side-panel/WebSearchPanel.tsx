import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RiskConfirmDialog } from "../RiskConfirmDialog";
import type { WebSearchEntryView } from "./view-model";

interface WebSearchPanelProps {
  entries: WebSearchEntryView[];
}

export function WebSearchPanel({ entries }: WebSearchPanelProps) {
  const [selectedId, setSelectedId] = useState("");
  const [pendingUrl, setPendingUrl] = useState("");

  useEffect(() => {
    if (!selectedId && entries.length > 0) {
      setSelectedId(entries[0].id);
    }
  }, [entries, selectedId]);

  const selected = entries.find((item) => item.id === selectedId) || entries[0] || null;

  return (
    <>
      <div className="flex h-full min-h-[640px] overflow-hidden rounded-2xl border border-gray-200 bg-white">
        <div className="flex w-[38%] min-w-[260px] flex-col border-r border-gray-200">
          <div className="px-4 py-4 text-2xl font-semibold text-gray-900">Web 搜索</div>
          <div className="min-h-0 flex-1 overflow-auto px-3 pb-3">
            <div className="space-y-2">
              {entries.map((entry) => (
                <button
                  key={entry.id}
                  type="button"
                  onClick={() => setSelectedId(entry.id)}
                  className={`w-full rounded-2xl px-4 py-3 text-left ${selected?.id === entry.id ? "bg-blue-50" : "hover:bg-gray-50"}`}
                >
                  <div className="truncate text-sm font-medium text-gray-800">{entry.query}</div>
                  <div className="mt-1 text-xs text-gray-500">
                    {entry.status === "completed" ? "已完成 Web 搜索" : entry.status} · {entry.results.length} 条结果
                  </div>
                </button>
              ))}
            </div>
          </div>
        </div>
        <div className="min-w-0 flex-1 overflow-auto p-4">
          {!selected ? (
            <div className="flex h-full items-center justify-center text-3xl font-semibold text-gray-300">选择要查看的搜索</div>
          ) : (
            <div>
              <div className="rounded-2xl border border-gray-200 bg-gray-50 p-4">
                <div className="text-xs font-medium text-gray-500">已完成 Web 搜索</div>
                <div className="mt-1 text-2xl font-semibold text-gray-900">{selected.query}</div>
              </div>
              <div className="mt-4 space-y-3">
                {selected.results.map((result) => (
                  <button
                    key={`${selected.id}-${result.url || result.title}`}
                    type="button"
                    className="block w-full rounded-2xl border border-gray-200 bg-white p-4 text-left shadow-sm hover:border-blue-200"
                    onClick={() => setPendingUrl(result.url)}
                  >
                    <div className="text-2xl font-semibold text-gray-900">{result.title}</div>
                    {result.domain && <div className="mt-1 text-sm text-gray-500">{result.domain}</div>}
                    {result.snippet && <div className="mt-2 text-sm leading-6 text-gray-600">{result.snippet}</div>}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
      <RiskConfirmDialog
        open={Boolean(pendingUrl)}
        level="low"
        title="打开搜索结果"
        summary="将在系统浏览器打开此链接，是否继续？"
        impact={pendingUrl || undefined}
        irreversible={false}
        confirmLabel="继续打开"
        cancelLabel="取消"
        loading={false}
        onCancel={() => setPendingUrl("")}
        onConfirm={() => {
          if (pendingUrl) {
            void invoke("open_external_url", { url: pendingUrl });
          }
          setPendingUrl("");
        }}
      />
    </>
  );
}
