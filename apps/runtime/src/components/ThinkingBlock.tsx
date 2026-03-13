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
    <div className="mb-3 rounded-2xl border border-gray-200 bg-gray-50/90 px-4 py-3 text-sm text-gray-700">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <span
            className={
              "inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full border " +
              (status === "thinking"
                ? "border-blue-200 bg-blue-50 text-blue-500"
                : status === "interrupted"
                ? "border-amber-200 bg-amber-50 text-amber-600"
                : "border-emerald-200 bg-emerald-50 text-emerald-600")
            }
          >
            {status === "thinking" ? (
              <span className="h-2 w-2 animate-pulse rounded-full bg-current" />
            ) : (
              <span className="text-[10px] font-semibold">{status === "completed" ? "✓" : "!"}</span>
            )}
          </span>
          <span className="truncate text-xs font-medium text-gray-600">{label}</span>
        </div>
        {hasContent && onToggle && (
          <button
            type="button"
            data-testid={toggleTestId}
            onClick={onToggle}
            className="shrink-0 text-xs font-medium text-gray-500 underline underline-offset-2 hover:text-gray-700"
          >
            {expanded ? "收起" : "展开"}
          </button>
        )}
      </div>
      {expanded && hasContent && (
        <div className="mt-3 max-h-56 overflow-y-auto whitespace-pre-wrap rounded-xl border border-gray-200 bg-white/80 px-3 py-2 text-[13px] leading-6 text-gray-600">
          {content}
        </div>
      )}
    </div>
  );
}
