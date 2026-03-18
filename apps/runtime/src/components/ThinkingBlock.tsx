type ThinkingBlockProps = {
  status: "thinking" | "completed" | "interrupted";
  content?: string;
  durationMs?: number;
  expanded: boolean;
  onToggle?: () => void;
  toggleTestId?: string;
};

function formatThinkingLabel(status: ThinkingBlockProps["status"], durationMs?: number) {
  if (status === "completed" && typeof durationMs === "number" && durationMs >= 0) {
    return `已思考 ${(durationMs / 1000).toFixed(1)}s`;
  }
  if (status === "interrupted") return "思考中断";
  return "思考中";
}

export function ThinkingBlock({
  status,
  content,
  durationMs,
  expanded,
  onToggle,
  toggleTestId = "thinking-block-toggle",
}: ThinkingBlockProps) {
  const hasContent = Boolean(content?.trim());
  const label = formatThinkingLabel(status, durationMs);

  return (
    <div className="mb-3 w-full border-b border-slate-200/80 pb-2 text-[12px] text-slate-400">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <span
            className={
              "inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-full border " +
              (status === "thinking"
                ? "border-slate-200 bg-slate-100 text-slate-500"
                : status === "interrupted"
                ? "border-amber-200 bg-amber-50 text-amber-600"
                : "border-slate-200 bg-slate-100 text-slate-500")
            }
          >
            {status === "thinking" ? (
              <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-current" />
            ) : (
              <span className="text-[10px] font-semibold">{status === "completed" ? "✓" : "!"}</span>
            )}
          </span>
          <span className="truncate text-[12px] font-medium text-slate-500">{label}</span>
        </div>
        {hasContent && onToggle && (
          <button
            type="button"
            data-testid={toggleTestId}
            onClick={onToggle}
            className="shrink-0 text-[12px] font-medium text-slate-400 transition-colors hover:text-slate-600"
          >
            {expanded ? "收起" : "展开"}
          </button>
        )}
      </div>
      {expanded && hasContent && (
        <div
          data-testid="thinking-block-detail"
          className="mt-2 max-h-56 overflow-y-auto whitespace-pre-wrap border-l border-slate-200 pl-3 pt-1 text-[13px] leading-6 text-slate-600"
        >
          {content}
        </div>
      )}
    </div>
  );
}
