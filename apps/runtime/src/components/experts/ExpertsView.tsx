import { useMemo, useState } from "react";
import { ClawhubInstallRequest, SkillManifest } from "../../types";
import { SkillLibraryView } from "./SkillLibraryView";
import { FindSkillsView } from "./FindSkillsView";
import { RiskConfirmDialog } from "../RiskConfirmDialog";

interface Props {
  skills: SkillManifest[];
  launchError?: string | null;
  onInstallSkill: () => void;
  onCreate: () => void;
  onOpenPackaging: () => void;
  onInstallFromLibrary: (request: ClawhubInstallRequest) => Promise<void>;
  onStartTaskWithSkill: (skillId: string) => void;
  onRefreshLocalSkill: (skillId: string) => void;
  onCheckClawhubUpdate: (skillId: string) => void;
  onUpdateClawhubSkill: (skillId: string) => void;
  onDeleteSkill: (skillId: string) => void;
  clawhubUpdateStatus?: Record<string, { hasUpdate: boolean; message: string }>;
  busySkillId?: string;
  busyAction?: "refresh" | "delete" | "check-update" | "update" | null;
}

type PendingExpertsRiskAction =
  | {
      kind: "delete";
      skillId: string;
      skillName: string;
    }
  | {
      kind: "update";
      skillId: string;
      skillName: string;
    };

export function ExpertsView({
  skills,
  launchError,
  onInstallSkill,
  onCreate,
  onOpenPackaging,
  onInstallFromLibrary,
  onStartTaskWithSkill,
  onRefreshLocalSkill,
  onCheckClawhubUpdate,
  onUpdateClawhubSkill,
  onDeleteSkill,
  clawhubUpdateStatus,
  busySkillId,
  busyAction,
}: Props) {
  const [activeTab, setActiveTab] = useState<"mine" | "library" | "finder">("mine");
  const [pendingRiskAction, setPendingRiskAction] = useState<PendingExpertsRiskAction | null>(null);
  const visibleSkills = skills.filter((skill) => skill.id !== "builtin-general");
  const totalSkills = visibleSkills.length;
  const builtinSkills = visibleSkills.filter((skill) => skill.id.startsWith("builtin-")).length;
  const localSkills = visibleSkills.filter((skill) => skill.id.startsWith("local-")).length;
  const installedSkills = visibleSkills.filter(
    (skill) => !skill.id.startsWith("local-") && !skill.id.startsWith("builtin-")
  ).length;
  const installedSkillIds = useMemo(
    () => new Set(visibleSkills.map((skill) => skill.id)),
    [visibleSkills]
  );
  const riskDialogLoading = Boolean(
    pendingRiskAction &&
      busySkillId === pendingRiskAction.skillId &&
      ((pendingRiskAction.kind === "delete" && busyAction === "delete") ||
        (pendingRiskAction.kind === "update" && busyAction === "update"))
  );
  const riskDialogMeta = pendingRiskAction
    ? pendingRiskAction.kind === "delete"
      ? {
          level: "high" as const,
          title: "移除技能",
          summary: `确定移除「${pendingRiskAction.skillName}」吗？`,
          impact: "该操作会移除技能入口，并可能影响已有工作流。",
          irreversible: true,
          confirmLabel: "确认移除",
        }
      : {
          level: "medium" as const,
          title: "更新技能",
          summary: `确定更新「${pendingRiskAction.skillName}」吗？`,
          impact: "更新后将覆盖当前版本，建议先完成正在进行的任务。",
          irreversible: false,
          confirmLabel: "确认更新",
        }
    : null;

  function requestDeleteSkill(skillId: string, skillName: string) {
    setPendingRiskAction({
      kind: "delete",
      skillId,
      skillName,
    });
  }

  function requestUpdateSkill(skillId: string, skillName: string) {
    setPendingRiskAction({
      kind: "update",
      skillId,
      skillName,
    });
  }

  function handleRiskCancel() {
    if (riskDialogLoading) return;
    setPendingRiskAction(null);
  }

  function handleRiskConfirm() {
    if (!pendingRiskAction) return;
    if (pendingRiskAction.kind === "delete") {
      onDeleteSkill(pendingRiskAction.skillId);
    } else {
      onUpdateClawhubSkill(pendingRiskAction.skillId);
    }
    setPendingRiskAction(null);
  }

  return (
    <div className="h-full overflow-y-auto bg-gray-50">
      <div className="max-w-6xl mx-auto px-8 pt-10 pb-12">
        <div className="flex items-start justify-between mb-6">
          <div>
            <h1 className="text-2xl font-semibold text-gray-900">专家技能</h1>
            <p className="text-sm text-gray-600 mt-2">
              管理你已安装和创建的技能，通过创建页持续沉淀可复用能力。
            </p>
            {launchError ? (
              <div className="mt-3 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">
                {launchError}
              </div>
            ) : null}
            <div className="flex flex-wrap items-center gap-2 mt-3">
              <span className="inline-flex items-center px-2.5 py-1 rounded-full bg-blue-50 text-blue-700 text-xs border border-blue-100">
                全部 {totalSkills}
              </span>
              <span className="inline-flex items-center px-2.5 py-1 rounded-full bg-green-50 text-green-700 text-xs border border-green-100">
                本地创建 {localSkills}
              </span>
              <span className="inline-flex items-center px-2.5 py-1 rounded-full bg-purple-50 text-purple-700 text-xs border border-purple-100">
                产品内置 {builtinSkills}
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
          <div className="inline-flex items-center">
            <button
              onClick={() => setActiveTab("mine")}
              className={`px-3 py-2 text-sm font-medium border-b-2 transition-colors ${
                activeTab === "mine"
                  ? "text-blue-600 border-blue-500"
                  : "text-gray-500 border-transparent hover:text-gray-700"
              }`}
            >
              我的技能
            </button>
            <button
              onClick={() => setActiveTab("library")}
              className={`px-3 py-2 text-sm font-medium border-b-2 transition-colors ${
                activeTab === "library"
                  ? "text-blue-600 border-blue-500"
                  : "text-gray-500 border-transparent hover:text-gray-700"
              }`}
            >
              技能库
            </button>
            <button
              onClick={() => setActiveTab("finder")}
              className={`px-3 py-2 text-sm font-medium border-b-2 transition-colors ${
                activeTab === "finder"
                  ? "text-blue-600 border-blue-500"
                  : "text-gray-500 border-transparent hover:text-gray-700"
              }`}
            >
              找技能
            </button>
          </div>
        </div>

        {activeTab === "mine" && visibleSkills.length === 0 ? (
          <div className="rounded-xl border border-dashed border-gray-200 bg-white px-4 py-10 text-center text-sm text-gray-400">
            暂无技能，点击右上角“创建”开始沉淀你的第一个专家技能。
          </div>
        ) : activeTab === "mine" ? (
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
            {visibleSkills.map((skill) => {
              const isBuiltin = skill.id.startsWith("builtin-");
              const isLocal = skill.id.startsWith("local-");
              const isClawhub = skill.id.startsWith("clawhub-");
              const source = isBuiltin ? "内置" : isLocal ? "本地" : "已安装";
              const updateState = clawhubUpdateStatus?.[skill.id];
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
                  {isClawhub && updateState && (
                    <div className={`text-[11px] mt-1 ${updateState.hasUpdate ? "text-amber-600" : "text-emerald-600"}`}>
                      {updateState.message}
                    </div>
                  )}
                  {isBuiltin && (
                    <div className="text-[11px] text-gray-400 mt-1">系统预置，不支持移除</div>
                  )}
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
                    {isClawhub && (
                      <button
                        onClick={() => onCheckClawhubUpdate(skill.id)}
                        disabled={busySkillId === skill.id && busyAction === "check-update"}
                        className="h-7 px-3 rounded bg-sky-50 hover:bg-sky-100 disabled:bg-sky-100 text-sky-700 text-xs transition-colors"
                      >
                        {busySkillId === skill.id && busyAction === "check-update" ? "检查中..." : "检查更新"}
                      </button>
                    )}
                    {isClawhub && updateState?.hasUpdate && (
                      <button
                        onClick={() => requestUpdateSkill(skill.id, skill.name)}
                        disabled={busySkillId === skill.id && busyAction === "update"}
                        className="h-7 px-3 rounded bg-emerald-50 hover:bg-emerald-100 disabled:bg-emerald-100 text-emerald-700 text-xs transition-colors"
                      >
                        {busySkillId === skill.id && busyAction === "update" ? "更新中..." : "更新"}
                      </button>
                    )}
                    {!isBuiltin && (
                      <button
                        onClick={() => requestDeleteSkill(skill.id, skill.name)}
                        disabled={busySkillId === skill.id && busyAction === "delete"}
                        className="h-7 px-3 rounded bg-red-50 hover:bg-red-100 disabled:bg-red-100 text-red-600 text-xs transition-colors"
                      >
                        {busySkillId === skill.id && busyAction === "delete" ? "移除中..." : "移除"}
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          activeTab === "library" ? (
            <SkillLibraryView
              installedSkillIds={installedSkillIds}
              onInstall={onInstallFromLibrary}
            />
          ) : (
            <FindSkillsView
              installedSkillIds={installedSkillIds}
              onInstall={onInstallFromLibrary}
            />
          )
        )}
      </div>
      <RiskConfirmDialog
        open={Boolean(riskDialogMeta)}
        level={riskDialogMeta?.level ?? "low"}
        title={riskDialogMeta?.title ?? ""}
        summary={riskDialogMeta?.summary ?? ""}
        impact={riskDialogMeta?.impact}
        irreversible={riskDialogMeta?.irreversible}
        confirmLabel={riskDialogMeta?.confirmLabel}
        cancelLabel="取消"
        loading={riskDialogLoading}
        onConfirm={handleRiskConfirm}
        onCancel={handleRiskCancel}
      />
    </div>
  );
}
