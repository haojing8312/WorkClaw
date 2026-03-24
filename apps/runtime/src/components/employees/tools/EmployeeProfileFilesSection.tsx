import { AgentProfileFilesView } from "../../../types";

interface EmployeeProfileFilesSectionProps {
  profileLoading: boolean;
  profileView: AgentProfileFilesView | null;
  onOpenEmployeeCreatorSkill?: () => void;
}

export function EmployeeProfileFilesSection({
  profileLoading,
  profileView,
  onOpenEmployeeCreatorSkill,
}: EmployeeProfileFilesSectionProps) {
  return (
    <div className="rounded-lg border border-gray-200 p-3 space-y-2">
      <div className="flex items-center justify-between gap-2">
        <div className="text-xs font-medium text-gray-700">AGENTS / SOUL / USER（只读）</div>
        <button
          type="button"
          onClick={onOpenEmployeeCreatorSkill}
          className="h-7 px-2.5 rounded border border-blue-200 hover:bg-blue-50 text-blue-700 text-xs"
        >
          更新画像
        </button>
      </div>
      {profileLoading ? (
        <div className="text-xs text-gray-500">正在加载配置文件...</div>
      ) : profileView ? (
        <>
          <div className="text-[11px] text-gray-500 break-all">目录：{profileView.profile_dir}</div>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
            {profileView.files.map((file) => (
              <div key={file.name} className="border border-gray-100 rounded p-2 space-y-1">
                <div className="text-xs font-medium text-gray-700">
                  {file.name} {file.exists ? "" : "（未生成）"}
                </div>
                {file.error ? (
                  <div className="text-[11px] text-red-600">读取失败：{file.error}</div>
                ) : file.exists ? (
                  <pre className="text-[11px] text-gray-600 whitespace-pre-wrap max-h-56 overflow-y-auto">{file.content}</pre>
                ) : (
                  <div className="text-[11px] text-gray-500">尚未生成。可使用“智能体员工助手”补齐配置。</div>
                )}
              </div>
            ))}
          </div>
        </>
      ) : (
        <div className="text-xs text-gray-500">暂无可展示的配置文件。</div>
      )}
    </div>
  );
}
