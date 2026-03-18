import { Plus, X } from "lucide-react";

export type TaskTabStripItem = {
  id: string;
  kind: "start-task" | "session";
  title: string;
  runtimeStatus?: string | null;
};

type Props = {
  tabs: TaskTabStripItem[];
  activeTabId: string | null;
  onSelectTab: (tabId: string) => void;
  onCreateTab: () => void;
  onCloseTab: (tabId: string) => void;
};

function getRuntimeBadge(status?: string | null) {
  const normalized = (status || "").trim().toLowerCase();
  if (normalized === "thinking") {
    return {
      label: "思考中",
      dotClassName: "bg-violet-500",
    };
  }
  if (normalized === "running") {
    return {
      label: "执行中",
      dotClassName: "bg-sky-500",
    };
  }
  if (normalized === "tool_calling") {
    return {
      label: "处理中",
      dotClassName: "bg-cyan-500",
    };
  }
  if (normalized === "waiting_approval") {
    return {
      label: "待确认",
      dotClassName: "bg-amber-500",
    };
  }
  if (normalized === "completed" || normalized === "done") {
    return {
      label: "已完成",
      dotClassName: "bg-emerald-500",
    };
  }
  if (normalized === "failed" || normalized === "error" || normalized === "cancelled") {
    return {
      label: "异常",
      dotClassName: "bg-rose-500",
    };
  }
  return null;
}

export function TaskTabStrip({
  tabs,
  activeTabId,
  onSelectTab,
  onCreateTab,
  onCloseTab,
}: Props) {
  return (
    <div className="overflow-hidden border-b border-[var(--sm-border)] bg-[color-mix(in_srgb,var(--sm-surface-muted)_88%,white_12%)] px-3 pt-2">
      <div className="flex items-end gap-1 overflow-x-auto overflow-y-hidden pb-px" role="tablist" aria-label="任务标签">
        {tabs.map((tab) => {
          const active = tab.id === activeTabId;
          const runtimeBadge = getRuntimeBadge(tab.runtimeStatus);
          return (
            <div
              key={tab.id}
              className={
                "group -mb-px inline-flex min-w-0 max-w-[240px] items-center rounded-t-[18px] border border-b-transparent text-sm transition-colors " +
                (active
                  ? "border-[var(--sm-border)] bg-[var(--sm-surface)] text-[var(--sm-text)]"
                  : "border-transparent bg-[color-mix(in_srgb,var(--sm-surface-muted)_78%,transparent_22%)] text-[var(--sm-text-muted)] hover:border-[var(--sm-border)] hover:bg-[color-mix(in_srgb,var(--sm-surface)_82%,var(--sm-surface-muted)_18%)] hover:text-[var(--sm-text)]")
              }
            >
              <button
                type="button"
                role="tab"
                aria-label={tab.title}
                aria-selected={active}
                className="flex min-w-0 flex-1 items-center gap-2 px-4 py-2 text-left outline-none"
                onClick={() => onSelectTab(tab.id)}
              >
                <span
                  aria-hidden="true"
                  className={
                    "h-2.5 w-2.5 flex-shrink-0 rounded-full transition-colors " +
                    (runtimeBadge?.dotClassName || (active ? "bg-[var(--sm-primary)]" : "bg-[var(--sm-border)]")) +
                    (runtimeBadge && ["thinking", "running", "tool_calling"].includes((tab.runtimeStatus || "").trim().toLowerCase())
                      ? " animate-pulse"
                      : "")
                  }
                />
                <span className="truncate font-medium">{tab.title}</span>
                {runtimeBadge ? <span className="sr-only">{runtimeBadge.label}</span> : null}
              </button>
              <button
                type="button"
                className={
                  "sm-btn sm-btn-ghost mr-2 h-6 w-6 flex-shrink-0 rounded-md text-[var(--sm-text-muted)] transition-all hover:bg-[var(--sm-surface-soft)] hover:text-[var(--sm-text)] focus-visible:text-[var(--sm-text)] " +
                  (active ? "opacity-70" : "opacity-0 group-hover:opacity-55 group-focus-within:opacity-55")
                }
                aria-label={`关闭标签 ${tab.title}`}
                onClick={(event) => {
                  event.stopPropagation();
                  onCloseTab(tab.id);
                }}
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </div>
          );
        })}
        <button
          type="button"
          className="sm-btn sm-btn-ghost -mb-px h-10 min-w-[46px] flex-shrink-0 rounded-t-[16px] border border-transparent border-b-transparent px-3 text-[var(--sm-text-muted)] transition-colors hover:border-[var(--sm-border)] hover:bg-[var(--sm-surface)] hover:text-[var(--sm-text)]"
          aria-label="新建任务标签"
          onClick={onCreateTab}
        >
          <Plus className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
