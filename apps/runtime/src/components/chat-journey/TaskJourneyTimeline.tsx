import type { TaskJourneyViewModel } from "../chat-side-panel/view-model";

interface TaskJourneyTimelineProps {
  model: TaskJourneyViewModel;
}

const STATUS_LABELS: Record<TaskJourneyViewModel["status"], string> = {
  running: "进行中",
  completed: "已完成",
  failed: "失败",
  partial: "部分完成",
};

export function TaskJourneyTimeline({ model }: TaskJourneyTimelineProps) {
  if (model.steps.length === 0 && !model.currentTaskTitle) {
    return null;
  }

  return (
    <div className="max-w-[80%] rounded-2xl border border-sky-200 bg-sky-50/80 px-5 py-4 text-sm text-sky-950 shadow-sm">
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="text-xs font-medium text-sky-700">任务进度</div>
          <div className="mt-1 text-base font-semibold">{model.currentTaskTitle}</div>
        </div>
        <div className="rounded-full bg-white px-3 py-1 text-xs font-medium text-sky-700">
          {STATUS_LABELS[model.status]}
        </div>
      </div>

      {model.steps.length > 0 && (
        <div className="mt-4 space-y-2">
          {model.steps.map((step) => (
            <div
              key={step.id}
              className={`rounded-xl border px-3 py-2 ${
                step.kind === "error"
                  ? "border-rose-200 bg-rose-50 text-rose-900"
                  : "border-sky-100 bg-white/80 text-slate-800"
              }`}
            >
              <div className="text-sm font-medium">{step.title}</div>
              {step.detail && <div className="mt-1 text-xs opacity-80">{step.detail}</div>}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
