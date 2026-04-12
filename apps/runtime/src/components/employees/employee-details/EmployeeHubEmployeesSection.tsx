import type {
  AgentEmployee,
  AgentProfileFilesView,
  EmployeeMemoryStats,
  ImRoutingBinding,
  OpenClawPluginFeishuRuntimeStatus,
} from "../../../types";
import { EmployeeFeishuAssociationSection } from "../EmployeeFeishuAssociationSection";
import type { EmployeeHubEmployeeFilter } from "../employeeHubOverview";
import { toEmployeeHubFeishuRuntimeStatus } from "../hooks/useEmployeeHubFeishu";
import { EmployeeMemoryToolsSection } from "../tools/EmployeeMemoryToolsSection";
import { EmployeeProfileFilesSection } from "../tools/EmployeeProfileFilesSection";

type EmployeeHubFeishuRuntimeStatus = ReturnType<typeof toEmployeeHubFeishuRuntimeStatus>;
type FeishuAssociationSavePayload = {
  enabled: boolean;
  mode: "default" | "scoped";
  peerKind: "group" | "channel" | "direct";
  peerId: string;
  priority: number;
};

interface EmployeeHubEmployeesSectionProps {
  employeeFilter: EmployeeHubEmployeeFilter;
  employeeFilterLabel: string;
  employees: AgentEmployee[];
  filteredEmployees: AgentEmployee[];
  formatBytes: (bytes: number) => string;
  globalDefaultWorkDir: string;
  highlightEmployeeId?: string | null;
  memoryActionLoading: "export" | "clear" | null;
  memoryLoading: boolean;
  memoryScopeSkillId: string;
  memorySkillScopeOptions: string[];
  memoryStats: EmployeeMemoryStats | null;
  officialFeishuRuntimeStatus: OpenClawPluginFeishuRuntimeStatus | null;
  onClearEmployeeFilter: () => void;
  onExportEmployeeMemory: () => void | Promise<void>;
  onMemoryScopeChange: (value: string) => void;
  onOpenEmployeeCreatorSkill?: (options?: { mode?: "create" | "update"; employeeId?: string }) => void | Promise<void>;
  onOpenFeishuSettings?: () => void;
  onOpenTeamsTab: () => void;
  onRefreshEmployeeMemoryStats: () => void | Promise<void>;
  onRequestClearEmployeeMemory: () => void;
  onRequestRemoveCurrent: () => void;
  onSelectEmployee: (employeeId: string) => void;
  onSetAsMainAndEnter: (employeeId: string) => void;
  onStartTaskWithEmployee: (employeeId: string) => void | Promise<void>;
  profileLoading: boolean;
  profileView: AgentProfileFilesView | null;
  resolveFeishuStatus: (
    employee: AgentEmployee,
    runtimeStatus: OpenClawPluginFeishuRuntimeStatus | null,
  ) => {
    dotClass: string;
    label: string;
    detail: string;
    error: string;
  };
  routingBindings: ImRoutingBinding[];
  saveFeishuAssociation: (payload: FeishuAssociationSavePayload) => Promise<void>;
  saving: boolean;
  savingFeishuAssociation: boolean;
  selectedEmployee: AgentEmployee | null;
  selectedEmployeeAuthorizedSkills: Array<{ id: string; name: string }>;
  selectedEmployeeFeishuRuntimeStatus: EmployeeHubFeishuRuntimeStatus;
  selectedEmployeeFeishuStatus: unknown;
  selectedEmployeeId: string | null;
  selectedEmployeeMemoryId: string;
  setMessage: (message: string) => void;
  skillNameById: Map<string, string>;
}

export function EmployeeHubEmployeesSection({
  employeeFilter,
  employeeFilterLabel,
  employees,
  filteredEmployees,
  formatBytes,
  globalDefaultWorkDir,
  highlightEmployeeId,
  memoryActionLoading,
  memoryLoading,
  memoryScopeSkillId,
  memorySkillScopeOptions,
  memoryStats,
  officialFeishuRuntimeStatus,
  onClearEmployeeFilter,
  onExportEmployeeMemory,
  onMemoryScopeChange,
  onOpenEmployeeCreatorSkill,
  onOpenFeishuSettings,
  onOpenTeamsTab,
  onRefreshEmployeeMemoryStats,
  onRequestClearEmployeeMemory,
  onRequestRemoveCurrent,
  onSelectEmployee,
  onSetAsMainAndEnter,
  onStartTaskWithEmployee,
  profileLoading,
  profileView,
  resolveFeishuStatus,
  routingBindings,
  saveFeishuAssociation,
  saving,
  savingFeishuAssociation,
  selectedEmployee,
  selectedEmployeeAuthorizedSkills,
  selectedEmployeeFeishuRuntimeStatus,
  selectedEmployeeFeishuStatus,
  selectedEmployeeId,
  selectedEmployeeMemoryId,
  setMessage,
  skillNameById,
}: EmployeeHubEmployeesSectionProps) {
  return (
    <div
      id="employee-hub-panel-employees"
      role="tabpanel"
      aria-labelledby="employee-hub-tab-employees"
      className="space-y-4"
    >
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-white border border-gray-200 rounded-xl p-3 max-h-[640px] overflow-y-auto">
          <div className="text-xs text-gray-500 mb-2">员工列表</div>
          <div className="mb-2">
            <button
              type="button"
              onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "create" })}
              className="h-8 w-full rounded bg-blue-600 hover:bg-blue-700 text-white text-xs"
            >
              新建员工
            </button>
          </div>
          {employeeFilter !== "all" && (
            <div className="mb-2 flex items-center justify-between rounded border border-blue-100 bg-blue-50 px-2 py-1.5 text-[11px] text-blue-700">
              <span>当前筛选：{employeeFilterLabel}</span>
              <button type="button" onClick={onClearEmployeeFilter} className="text-blue-700 hover:text-blue-800">
                清除筛选
              </button>
            </div>
          )}
          <div className="space-y-2">
            {filteredEmployees.length === 0 ? (
              <div className="rounded border border-dashed border-gray-300 px-3 py-4 text-xs text-gray-500">
                当前筛选下暂无员工。
              </div>
            ) : (
              filteredEmployees.map((employee) => {
                const status = resolveFeishuStatus(employee, officialFeishuRuntimeStatus);
                const isSelected = selectedEmployeeId === employee.id;
                const isHighlighted = highlightEmployeeId === employee.id;
                return (
                  <button
                    key={employee.id}
                    data-testid={`employee-item-${employee.id}`}
                    onClick={() => {
                      onSelectEmployee(employee.id);
                      setMessage("");
                    }}
                    className={
                      "w-full text-left border rounded p-2 text-xs " +
                      (isHighlighted
                        ? "border-emerald-300 bg-emerald-50 ring-1 ring-emerald-200"
                        : isSelected
                          ? "border-blue-300 bg-blue-50"
                          : "border-gray-200 bg-white")
                    }
                  >
                    <div className="flex items-center gap-2">
                      <span
                        data-testid={`employee-connection-dot-${employee.id}`}
                        className={`inline-block h-2.5 w-2.5 rounded-full ${status.dotClass}`}
                        title={status.label}
                      />
                      <div className="font-medium text-gray-800 truncate">
                        {employee.name} {employee.is_default ? "· 主员工" : ""}
                      </div>
                      {isHighlighted && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-100 text-emerald-700 border border-emerald-200">
                          新建
                        </span>
                      )}
                    </div>
                    <div className="text-gray-500 truncate">{employee.employee_id || employee.role_id}</div>
                  </button>
                );
              })
            )}
          </div>
        </div>

        <div className="md:col-span-2 bg-white border border-gray-200 rounded-xl p-4 space-y-3">
          <div className="text-xs text-gray-500">员工详情</div>
          {selectedEmployee ? (
            <>
              <div className="rounded-lg border border-gray-200 p-3 space-y-2">
                <div className="flex items-center justify-between gap-2">
                  <div>
                    <div className="text-sm font-semibold text-gray-900">{selectedEmployee.name}</div>
                    <div className="text-xs text-gray-500">{selectedEmployeeMemoryId || "未设置员工编号"}</div>
                  </div>
                  <button
                    type="button"
                    onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "update", employeeId: selectedEmployee.id })}
                    className="h-8 px-3 rounded border border-blue-200 hover:bg-blue-50 text-blue-700 text-xs"
                  >
                    调整员工
                  </button>
                </div>
                <div className="text-[11px] text-gray-500">角色职责</div>
                <div className="text-xs text-gray-700 whitespace-pre-wrap">
                  {selectedEmployee.persona?.trim() || "暂无职责描述，可通过智能体员工助手补充。"}
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                  <div className="rounded border border-gray-100 p-2">
                    <div className="text-[11px] text-gray-500">主技能</div>
                    <div className="text-xs text-gray-700">
                      {selectedEmployee.primary_skill_id
                        ? (skillNameById.get(selectedEmployee.primary_skill_id) || selectedEmployee.primary_skill_id)
                        : "通用助手（系统默认）"}
                    </div>
                  </div>
                  <div className="rounded border border-gray-100 p-2">
                    <div className="text-[11px] text-gray-500">默认工作目录</div>
                    <div className="text-xs text-gray-700 break-all">
                      {selectedEmployee.default_work_dir?.trim() || globalDefaultWorkDir.trim() || "跟随系统默认目录"}
                    </div>
                  </div>
                </div>
                <div className="text-[11px] text-gray-500">技能合集</div>
                <div className="flex flex-wrap gap-1">
                  {selectedEmployeeAuthorizedSkills.length === 0 ? (
                    <span className="text-[11px] px-2 py-0.5 rounded border border-gray-200 text-gray-500">
                      未配置，默认按主技能执行
                    </span>
                  ) : (
                    selectedEmployeeAuthorizedSkills.map((item) => (
                      <span
                        key={item.id}
                        className="text-[11px] px-2 py-0.5 rounded border border-blue-100 bg-blue-50 text-blue-700"
                      >
                        {item.name}
                      </span>
                    ))
                  )}
                </div>
              </div>

              {selectedEmployeeFeishuStatus && (
                <EmployeeFeishuAssociationSection
                  employee={selectedEmployee}
                  employees={employees}
                  bindings={routingBindings}
                  saving={savingFeishuAssociation}
                  runtimeStatus={selectedEmployeeFeishuRuntimeStatus}
                  onSave={saveFeishuAssociation}
                  onOpenFeishuSettings={onOpenFeishuSettings}
                />
              )}

              <EmployeeProfileFilesSection
                profileLoading={profileLoading}
                profileView={profileView}
                onOpenEmployeeCreatorSkill={() =>
                  onOpenEmployeeCreatorSkill?.({ mode: "update", employeeId: selectedEmployee.id })
                }
              />

              <div className="flex items-center gap-2 pt-1">
                <button
                  disabled={saving || !selectedEmployeeId}
                  onClick={onRequestRemoveCurrent}
                  className="h-8 px-3 rounded bg-red-50 hover:bg-red-100 disabled:bg-gray-100 text-red-600 text-xs"
                >
                  删除员工
                </button>
                <button
                  disabled={!selectedEmployeeId}
                  onClick={() => selectedEmployeeId && onSetAsMainAndEnter(selectedEmployeeId)}
                  className="h-8 px-3 rounded bg-emerald-50 hover:bg-emerald-100 disabled:bg-gray-100 text-emerald-700 text-xs"
                >
                  设为主员工并进入首页
                </button>
                <button
                  disabled={!selectedEmployeeId || saving}
                  onClick={() => selectedEmployeeId && onStartTaskWithEmployee(selectedEmployeeId)}
                  className="h-8 px-3 rounded bg-indigo-50 hover:bg-indigo-100 disabled:bg-gray-100 text-indigo-700 text-xs"
                >
                  与该员工开始对话
                </button>
                <button
                  type="button"
                  onClick={onOpenTeamsTab}
                  className="h-8 px-3 rounded bg-violet-50 hover:bg-violet-100 text-violet-700 text-xs"
                >
                  以团队模式发起任务
                </button>
              </div>
            </>
          ) : (
            <div className="rounded-lg border border-dashed border-gray-300 p-4 space-y-2">
              <div className="text-sm font-medium text-gray-800">请选择一个员工或直接创建</div>
              <div className="text-xs text-gray-600">
                已移除手动创建流程，请通过「智能体员工助手」对话式完成创建与配置。
              </div>
              <button
                type="button"
                onClick={() => onOpenEmployeeCreatorSkill?.({ mode: "create" })}
                className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 text-white text-xs"
              >
                创建员工
              </button>
            </div>
          )}

          <EmployeeMemoryToolsSection
            memoryLoading={memoryLoading}
            memoryActionLoading={memoryActionLoading}
            memoryScopeSkillId={memoryScopeSkillId}
            memorySkillScopeOptions={memorySkillScopeOptions}
            selectedEmployeeMemoryId={selectedEmployeeMemoryId}
            memoryStats={memoryStats}
            formatBytes={formatBytes}
            onMemoryScopeChange={onMemoryScopeChange}
            onRefreshEmployeeMemoryStats={onRefreshEmployeeMemoryStats}
            onExportEmployeeMemory={onExportEmployeeMemory}
            onRequestClearEmployeeMemory={onRequestClearEmployeeMemory}
          />
        </div>
      </div>
    </div>
  );
}
