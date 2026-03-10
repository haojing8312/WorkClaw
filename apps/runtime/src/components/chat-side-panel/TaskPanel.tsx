import type { TaskPanelViewModel } from "./view-model";

interface TaskPanelProps {
  model: TaskPanelViewModel;
}

export function TaskPanel({ model }: TaskPanelProps) {
  return (
    <div className="space-y-4">
      <div className="rounded-2xl border border-gray-200 bg-white p-4 shadow-sm">
        <div className="text-xs font-medium text-gray-500">当前任务</div>
        {model.hasTodoList ? (
          <>
            <div className="mt-2 text-sm text-gray-500">当前执行项</div>
            <div className="text-base font-semibold text-gray-900">{model.currentTaskTitle || "暂无进行中的任务"}</div>
          </>
        ) : (
          <>
            <div className="mt-2 text-base font-semibold text-gray-900">未创建任务清单</div>
            <div className="mt-1 text-sm text-gray-500">本轮会话没有使用 TodoWrite 创建结构化任务。</div>
          </>
        )}

        <div className="mt-4 grid grid-cols-3 gap-2 text-xs">
          <div className="rounded-xl bg-gray-50 p-3">
            <div className="text-gray-500">总任务数</div>
            <div className="mt-1 text-lg font-semibold text-gray-900">{model.totalTasks}</div>
          </div>
          <div className="rounded-xl bg-emerald-50 p-3">
            <div className="text-emerald-700">已完成</div>
            <div className="mt-1 text-lg font-semibold text-emerald-700">{model.completedTasks}</div>
          </div>
          <div className="rounded-xl bg-blue-50 p-3">
            <div className="text-blue-700">进行中</div>
            <div className="mt-1 text-lg font-semibold text-blue-700">{model.inProgressTasks}</div>
          </div>
        </div>
      </div>

      <div className="rounded-2xl border border-gray-200 bg-white p-4 shadow-sm">
        <div className="flex items-center justify-between">
          <div className="text-xs font-medium text-gray-500">本轮动作摘要</div>
        </div>
        <div className="mt-3 flex flex-wrap gap-2">
          <span className="rounded-full bg-gray-100 px-3 py-1 text-xs text-gray-700">
            本轮生成文件 {model.touchedFileCount}
          </span>
          <span className="rounded-full bg-gray-100 px-3 py-1 text-xs text-gray-700">
            本轮 Web 搜索 {model.webSearchCount}
          </span>
        </div>
        {(model.latestTouchedFile || model.latestSearchQuery) && (
          <div className="mt-3 space-y-1 text-xs text-gray-500">
            {model.latestTouchedFile && <div>最近文件：{model.latestTouchedFile}</div>}
            {model.latestSearchQuery && <div>最近搜索：{model.latestSearchQuery}</div>}
          </div>
        )}
      </div>

      <div className="rounded-2xl border border-gray-200 bg-white p-4 shadow-sm">
        <div className="text-xs font-medium text-gray-500">任务清单</div>
        {model.items.length === 0 ? (
          <div className="mt-3 text-sm text-gray-400">暂无任务项</div>
        ) : (
          <div className="mt-3 space-y-2">
            {model.items.map((item) => (
              <div
                key={item.id}
                className={`flex items-center gap-3 rounded-xl border px-3 py-2 ${
                  item.status === "in_progress"
                    ? "border-blue-200 bg-blue-50"
                    : item.status === "completed"
                    ? "border-emerald-200 bg-emerald-50"
                    : "border-gray-200 bg-gray-50"
                }`}
              >
                <span
                  className={`inline-flex h-5 w-5 items-center justify-center rounded-full text-[11px] ${
                    item.status === "completed"
                      ? "bg-emerald-500 text-white"
                      : item.status === "in_progress"
                      ? "bg-blue-500 text-white"
                      : "bg-gray-300 text-white"
                  }`}
                >
                  {item.status === "completed" ? "✓" : item.status === "in_progress" ? "•" : "○"}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium text-gray-800">{item.title}</div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
