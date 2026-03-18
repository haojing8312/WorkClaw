import type { TaskJourneyViewModel } from "../chat-side-panel/view-model";

interface DeliverySummaryCardProps {
  model: TaskJourneyViewModel;
  workspace?: string;
  onViewFiles?: () => void;
  onOpenWorkspace?: () => void;
  onResumeFailedWork?: () => void;
}

export function DeliverySummaryCard({
  model,
  workspace,
  onViewFiles,
  onOpenWorkspace,
  onResumeFailedWork,
}: DeliverySummaryCardProps) {
  if (model.deliverables.length === 0 && model.warnings.length === 0) {
    return null;
  }

  const summaryText =
    model.status === "partial"
      ? `已生成 ${model.deliverables.length} 个文件，仍有 ${model.warnings.length} 个问题待补做`
      : model.status === "failed"
      ? "本轮交付未完成，建议先处理失败步骤后继续生成"
      : `已生成 ${model.deliverables.length} 个交付文件`;

  return (
    <div className="max-w-[80%] rounded-2xl border border-slate-200/85 bg-white/92 px-5 py-4 text-sm text-slate-900 shadow-[0_10px_24px_-22px_rgba(15,23,42,0.24)]">
      <div className="text-xs font-medium text-slate-500">交付结果</div>
      <div className="mt-1 text-base font-semibold">
        {model.status === "partial" ? "已生成部分产物" : model.status === "failed" ? "交付失败" : "已生成交付产物"}
      </div>
      <div className="mt-2 text-xs text-slate-600">{summaryText}</div>

      <div className="mt-4 flex flex-wrap gap-2">
        {model.deliverables.length > 0 && onViewFiles && (
          <button
            type="button"
            onClick={onViewFiles}
            className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-xs font-medium text-slate-700 hover:bg-slate-100"
          >
            查看文件
          </button>
        )}
        {workspace && onOpenWorkspace && (
          <button
            type="button"
            onClick={onOpenWorkspace}
            className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-xs font-medium text-slate-700 hover:bg-slate-100"
          >
            打开工作区
          </button>
        )}
        {model.warnings.length > 0 && onResumeFailedWork && (
          <button
            type="button"
            onClick={onResumeFailedWork}
            className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-1.5 text-xs font-medium text-amber-900 hover:bg-amber-100"
          >
            继续补做失败项
          </button>
        )}
      </div>

      {model.deliverables.length > 0 && (
        <div className="mt-4 space-y-2">
          {model.deliverables.map((item) => (
            <div key={item.path} className="rounded-xl border border-slate-200/80 bg-slate-50/60 px-3 py-2">
              <div className="text-sm font-medium text-slate-900">{item.path}</div>
              <div className="mt-1 text-xs text-slate-500">
                {item.category === "primary" ? "主产物" : "辅助产物"} · {item.tool === "write_file" ? "文件写入" : "文件编辑"}
              </div>
            </div>
          ))}
        </div>
      )}

      {model.warnings.length > 0 && (
        <div className="mt-4 space-y-2">
          {model.warnings.map((warning) => (
            <div key={warning} className="rounded-xl border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-900">
              {warning}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
