interface EmployeeHubSettingsSectionProps {
  defaultWorkspacePathExample: string;
  globalDefaultWorkDir: string;
  savingGlobalWorkDir: boolean;
  onGlobalDefaultWorkDirChange: (value: string) => void;
  onSaveGlobalDefaultWorkDir: () => void | Promise<void>;
}

export function EmployeeHubSettingsSection({
  defaultWorkspacePathExample,
  globalDefaultWorkDir,
  savingGlobalWorkDir,
  onGlobalDefaultWorkDirChange,
  onSaveGlobalDefaultWorkDir,
}: EmployeeHubSettingsSectionProps) {
  return (
    <div
      id="employee-hub-panel-settings"
      role="tabpanel"
      aria-labelledby="employee-hub-tab-settings"
      className="space-y-4"
    >
      <div className="bg-white border border-gray-200 rounded-xl p-4 space-y-2">
        <div className="text-xs text-gray-500">全局默认工作目录（新建会话默认使用）</div>
        <input
          className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
          placeholder={`例如 ${defaultWorkspacePathExample}`}
          value={globalDefaultWorkDir}
          onChange={(event) => onGlobalDefaultWorkDirChange(event.target.value)}
        />
        <div className="text-[11px] text-gray-500">
          默认：{defaultWorkspacePathExample}。支持 C/D/E 盘路径，目录不存在会自动创建。
        </div>
        <button
          disabled={savingGlobalWorkDir}
          onClick={onSaveGlobalDefaultWorkDir}
          className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-xs"
        >
          保存默认目录
        </button>
      </div>
    </div>
  );
}
