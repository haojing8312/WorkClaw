import { EmployeeMemoryStats } from "../../../types";

interface EmployeeMemoryToolsSectionProps {
  memoryLoading: boolean;
  memoryActionLoading: "export" | "clear" | null;
  memoryScopeSkillId: string;
  memorySkillScopeOptions: string[];
  selectedEmployeeMemoryId: string;
  memoryStats: EmployeeMemoryStats | null;
  formatBytes: (bytes: number) => string;
  onMemoryScopeChange: (value: string) => void;
  onRefreshEmployeeMemoryStats: () => void;
  onExportEmployeeMemory: () => void;
  onRequestClearEmployeeMemory: () => void;
}

export function EmployeeMemoryToolsSection({
  memoryLoading,
  memoryActionLoading,
  memoryScopeSkillId,
  memorySkillScopeOptions,
  selectedEmployeeMemoryId,
  memoryStats,
  formatBytes,
  onMemoryScopeChange,
  onRefreshEmployeeMemoryStats,
  onExportEmployeeMemory,
  onRequestClearEmployeeMemory,
}: EmployeeMemoryToolsSectionProps) {
  const disabled = memoryLoading || memoryActionLoading !== null || !selectedEmployeeMemoryId;

  return (
    <div className="rounded-lg border border-indigo-200 bg-indigo-50 p-3 space-y-2">
      <div className="flex items-center justify-between gap-2">
        <div className="text-xs font-medium text-indigo-900">长期记忆管理</div>
        {memoryLoading && <div className="text-[11px] text-indigo-600">统计刷新中...</div>}
      </div>
      <div className="grid grid-cols-1 md:grid-cols-4 gap-2">
        <select
          data-testid="employee-memory-scope"
          className="border border-indigo-200 rounded px-2 py-1.5 text-xs bg-white"
          value={memoryScopeSkillId}
          onChange={(e) => onMemoryScopeChange(e.target.value)}
        >
          <option value="__all__">全部技能</option>
          {memorySkillScopeOptions.map((id) => (
            <option key={id} value={id}>
              {id}
            </option>
          ))}
        </select>
        <button
          type="button"
          data-testid="employee-memory-refresh"
          onClick={onRefreshEmployeeMemoryStats}
          disabled={disabled}
          className="h-8 rounded border border-indigo-200 hover:bg-indigo-100 disabled:bg-gray-100 text-indigo-700 text-xs"
        >
          刷新统计
        </button>
        <button
          type="button"
          data-testid="employee-memory-export"
          onClick={onExportEmployeeMemory}
          disabled={disabled}
          className="h-8 rounded border border-indigo-200 hover:bg-indigo-100 disabled:bg-gray-100 text-indigo-700 text-xs"
        >
          {memoryActionLoading === "export" ? "导出中..." : "导出 JSON"}
        </button>
        <button
          type="button"
          data-testid="employee-memory-clear"
          onClick={onRequestClearEmployeeMemory}
          disabled={disabled}
          className="h-8 rounded border border-red-200 hover:bg-red-50 disabled:bg-gray-100 text-red-600 text-xs"
        >
          清空记忆
        </button>
      </div>
      <div className="text-xs text-indigo-800 flex items-center gap-4">
        <span data-testid="employee-memory-total-files">文件数：{memoryStats?.total_files ?? 0}</span>
        <span data-testid="employee-memory-total-bytes">大小：{memoryStats?.total_bytes ?? 0}</span>
        <span>({formatBytes(memoryStats?.total_bytes ?? 0)})</span>
      </div>
      <div className="max-h-32 overflow-y-auto rounded border border-indigo-100 bg-white p-2 space-y-1">
        {(memoryStats?.skills || []).length === 0 ? (
          <div className="text-[11px] text-gray-500">暂无长期记忆文件</div>
        ) : (
          (memoryStats?.skills || []).map((item) => (
            <div
              key={item.skill_id}
              data-testid={`employee-memory-skill-${item.skill_id}`}
              className="text-[11px] text-gray-700 flex items-center justify-between"
            >
              <span>{item.skill_id}</span>
              <span>
                {item.total_files} 文件 / {formatBytes(item.total_bytes)}
              </span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
