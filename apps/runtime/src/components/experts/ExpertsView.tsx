import { SkillManifest } from "../../types";

interface Props {
  skills: SkillManifest[];
  onInstallSkill: () => void;
  onCreate: () => void;
  onOpenPackaging: () => void;
  onStartTaskWithSkill: (skillId: string) => void;
  onRefreshLocalSkill: (skillId: string) => void;
  onDeleteSkill: (skillId: string) => void;
  busySkillId?: string;
  busyAction?: "refresh" | "delete" | null;
}

export function ExpertsView({
  skills,
  onInstallSkill,
  onCreate,
  onOpenPackaging,
  onStartTaskWithSkill,
  onRefreshLocalSkill,
  onDeleteSkill,
  busySkillId,
  busyAction,
}: Props) {
  const visibleSkills = skills.filter((skill) => skill.id !== "builtin-general");
  const totalSkills = visibleSkills.length;
  const localSkills = visibleSkills.filter((skill) => skill.id.startsWith("local-")).length;
  const installedSkills = visibleSkills.filter((skill) => !skill.id.startsWith("local-")).length;

  return (
    <div className="h-full overflow-y-auto bg-gray-50">
      <div className="max-w-6xl mx-auto px-8 pt-10 pb-12">
        <div className="flex items-start justify-between mb-6">
          <div>
            <h1 className="text-2xl font-semibold text-gray-900">专家技能</h1>
            <p className="text-sm text-gray-600 mt-2">
              管理你已安装和创建的技能，通过创建页持续沉淀可复用能力。
            </p>
            <div className="flex flex-wrap items-center gap-2 mt-3">
              <span className="inline-flex items-center px-2.5 py-1 rounded-full bg-blue-50 text-blue-700 text-xs border border-blue-100">
                全部 {totalSkills}
              </span>
              <span className="inline-flex items-center px-2.5 py-1 rounded-full bg-green-50 text-green-700 text-xs border border-green-100">
                本地创建 {localSkills}
              </span>
              <span className="inline-flex items-center px-2.5 py-1 rounded-full bg-amber-50 text-amber-700 text-xs border border-amber-100">
                外部安装 {installedSkills}
              </span>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={onInstallSkill}
              className="h-9 px-4 rounded-lg bg-blue-50 hover:bg-blue-100 text-blue-700 text-sm transition-colors"
            >
              安装技能
            </button>
            <button
              onClick={onOpenPackaging}
              className="h-9 px-4 rounded-lg bg-blue-500 hover:bg-blue-600 text-white text-sm transition-colors"
            >
              技能打包
            </button>
            <button
              onClick={onCreate}
              className="h-9 px-4 rounded-lg bg-blue-500 hover:bg-blue-600 text-white text-sm transition-colors"
            >
              创建
            </button>
          </div>
        </div>

        <div className="border-b border-gray-200 mb-5">
          <div className="inline-flex items-center px-2 py-2 text-sm font-medium text-blue-600 border-b-2 border-blue-500">
            我的技能
          </div>
        </div>

        {visibleSkills.length === 0 ? (
          <div className="rounded-xl border border-dashed border-gray-200 bg-white px-4 py-10 text-center text-sm text-gray-400">
            暂无技能，点击右上角“创建”开始沉淀你的第一个专家技能。
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
            {visibleSkills.map((skill) => {
              const isLocal = skill.id.startsWith("local-");
              const source = isLocal ? "本地" : "已安装";
              return (
                <div key={skill.id} className="bg-white border border-gray-200 rounded-xl p-4">
                  <div className="flex items-center justify-between gap-2 mb-2">
                    <div className="text-sm font-medium text-gray-800 truncate">{skill.name}</div>
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-50 text-blue-600 border border-blue-100">
                      {source}
                    </span>
                  </div>
                  <div className="text-xs text-gray-500 line-clamp-2 min-h-[32px]">{skill.description || "暂无描述"}</div>
                  <div className="text-[11px] text-gray-400 mt-2">版本 {skill.version}</div>
                  <div className="mt-3 flex items-center gap-2 flex-wrap">
                    <button
                      onClick={() => onStartTaskWithSkill(skill.id)}
                      className="h-7 px-3 rounded bg-blue-500 hover:bg-blue-600 text-white text-xs transition-colors"
                    >
                      开始任务
                    </button>
                    {isLocal && (
                      <button
                        onClick={() => onRefreshLocalSkill(skill.id)}
                        disabled={busySkillId === skill.id && busyAction === "refresh"}
                        className="h-7 px-3 rounded bg-blue-50 hover:bg-blue-100 disabled:bg-blue-100 text-blue-700 text-xs transition-colors"
                      >
                        {busySkillId === skill.id && busyAction === "refresh" ? "刷新中..." : "刷新"}
                      </button>
                    )}
                    <button
                      onClick={() => onDeleteSkill(skill.id)}
                      disabled={busySkillId === skill.id && busyAction === "delete"}
                      className="h-7 px-3 rounded bg-red-50 hover:bg-red-100 disabled:bg-red-100 text-red-600 text-xs transition-colors"
                    >
                      {busySkillId === skill.id && busyAction === "delete" ? "移除中..." : "移除"}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
