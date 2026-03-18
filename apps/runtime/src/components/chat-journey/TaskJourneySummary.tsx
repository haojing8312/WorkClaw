import type { TaskJourneyViewModel } from "../chat-side-panel/view-model";

interface TaskJourneySummaryProps {
  model: TaskJourneyViewModel;
  onViewFiles?: () => void;
}

export function TaskJourneySummary({
  model,
  onViewFiles,
}: TaskJourneySummaryProps) {
  if (model.deliverables.length === 0 || !onViewFiles) {
    return null;
  }

  const deliverableCount = model.deliverables.length;
  const countLabel = `共 ${deliverableCount} 个文件`;

  return (
    <div className="mr-auto mt-3 max-w-[80%]">
      <button
        type="button"
        onClick={onViewFiles}
        aria-label="查看此任务中的所有文件"
        className="group flex w-full items-center gap-4 rounded-2xl border border-slate-200/80 bg-white/92 px-5 py-4 text-left text-slate-800 shadow-[0_12px_28px_-24px_rgba(15,23,42,0.22)] transition-all duration-200 hover:-translate-y-0.5 hover:border-slate-300 hover:shadow-[0_16px_34px_-26px_rgba(15,23,42,0.24)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 focus-visible:ring-offset-2 active:translate-y-0"
      >
        <span className="flex h-11 w-11 flex-none items-center justify-center rounded-2xl bg-slate-100 text-slate-600 shadow-[inset_0_1px_0_rgba(255,255,255,0.7)] transition-colors duration-200 group-hover:text-slate-700">
          <svg
            aria-hidden="true"
            className="h-6 w-6"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={1.8}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M3 7.5A2.25 2.25 0 015.25 5.25H9l2.25 2.25h7.5A2.25 2.25 0 0121 9.75v8.5A2.25 2.25 0 0118.75 20.5H5.25A2.25 2.25 0 013 18.25V7.5z"
            />
          </svg>
        </span>
        <span className="min-w-0 flex-1">
          <span className="block text-lg font-semibold tracking-tight text-slate-900 sm:text-xl">
            查看此任务中的所有文件
          </span>
          <span className="mt-1 block text-sm leading-5 text-slate-500">
            任务已完成，点击查看本次产出文件
          </span>
          <span className="mt-2 inline-flex rounded-full border border-slate-200 bg-slate-50 px-2.5 py-1 text-xs font-medium text-slate-600">
            {countLabel}
          </span>
        </span>
        <span className="hidden h-9 w-9 flex-none items-center justify-center rounded-full border border-slate-200 bg-slate-50 text-slate-400 shadow-[0_1px_2px_rgba(15,23,42,0.04)] transition-all duration-200 group-hover:translate-x-0.5 group-hover:border-slate-300 group-hover:text-slate-600 sm:inline-flex">
          <svg
            aria-hidden="true"
            className="h-4 w-4"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
          </svg>
        </span>
      </button>
    </div>
  );
}
