import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AgentEmployee,
  EmployeeGroup,
  EmployeeGroupRule,
  EmployeeGroupRunResult,
  EmployeeGroupRunSummary,
} from "../../../types";
import {
  EmployeeGroupExecutionMode,
  EmployeeGroupReviewMode,
  EmployeeGroupVisibilityMode,
  EmployeeHubRunFilter,
  EmployeeHubTeamFilter,
  matchesEmployeeHubRunFilter,
  matchesEmployeeHubTeamFilter,
} from "../employeeHubOverview";

export interface UseEmployeeHubGroupsArgs {
  employees: AgentEmployee[];
  onOpenGroupRunSession?: (sessionId: string, skillId: string) => Promise<void> | void;
  onEmployeeGroupsChanged?: () => Promise<void> | void;
  setMessage: (message: string) => void;
}

function employeeKey(employee: AgentEmployee): string {
  return (employee.employee_id || employee.role_id || "").trim();
}

export function useEmployeeHubGroups({
  employees,
  onOpenGroupRunSession,
  onEmployeeGroupsChanged,
  setMessage,
}: UseEmployeeHubGroupsArgs) {
  const [groupName, setGroupName] = useState("");
  const [groupCoordinatorId, setGroupCoordinatorId] = useState("");
  const [groupMemberIds, setGroupMemberIds] = useState<string[]>([]);
  const [groupEntryId, setGroupEntryId] = useState("");
  const [groupPlannerId, setGroupPlannerId] = useState("");
  const [groupReviewerId, setGroupReviewerId] = useState("");
  const [groupReviewMode, setGroupReviewMode] = useState<EmployeeGroupReviewMode>("none");
  const [groupExecutionMode, setGroupExecutionMode] = useState<EmployeeGroupExecutionMode>("sequential");
  const [groupVisibilityMode, setGroupVisibilityMode] = useState<EmployeeGroupVisibilityMode>("internal");
  const [employeeGroups, setEmployeeGroups] = useState<EmployeeGroup[]>([]);
  const [recentRuns, setRecentRuns] = useState<EmployeeGroupRunSummary[]>([]);
  const [teamFilter, setTeamFilter] = useState<EmployeeHubTeamFilter>("all");
  const [runFilter, setRunFilter] = useState<EmployeeHubRunFilter>("all");
  const [groupSubmitting, setGroupSubmitting] = useState(false);
  const [groupDeletingId, setGroupDeletingId] = useState<string | null>(null);
  const [groupRunGoalById, setGroupRunGoalById] = useState<Record<string, string>>({});
  const [groupRunSubmittingId, setGroupRunSubmittingId] = useState<string | null>(null);
  const [groupRunReportById, setGroupRunReportById] = useState<Record<string, string>>({});
  const [groupRulesById, setGroupRulesById] = useState<Record<string, EmployeeGroupRule[]>>({});
  const [cloningGroupId, setCloningGroupId] = useState<string | null>(null);

  useEffect(() => {
    const normalized = employees
      .map((item) => employeeKey(item))
      .filter((item) => item.length > 0);
    setGroupMemberIds((prev) => prev.filter((id) => normalized.includes(id)));
    setGroupCoordinatorId((prev) => (normalized.includes(prev) ? prev : normalized[0] || ""));
  }, [employees]);

  async function loadEmployeeGroups() {
    try {
      const groups = await invoke<EmployeeGroup[]>("list_employee_groups");
      const normalizedGroups = Array.isArray(groups) ? groups : [];
      setEmployeeGroups(normalizedGroups);
      if (normalizedGroups.length === 0) {
        setGroupRulesById({});
        return;
      }
      const entries = await Promise.all(
        normalizedGroups.map(async (group) => {
          try {
            const rules = await invoke<EmployeeGroupRule[]>("list_employee_group_rules", {
              groupId: group.id,
            });
            return [group.id, Array.isArray(rules) ? rules : []] as const;
          } catch {
            return [group.id, []] as const;
          }
        }),
      );
      setGroupRulesById(Object.fromEntries(entries));
    } catch {
      setEmployeeGroups([]);
      setGroupRulesById({});
    }
  }

  async function loadRecentRuns() {
    try {
      const runs = await invoke<EmployeeGroupRunSummary[]>("list_employee_group_runs", { limit: 10 });
      setRecentRuns(Array.isArray(runs) ? runs : []);
    } catch {
      setRecentRuns([]);
    }
  }

  useEffect(() => {
    void loadEmployeeGroups();
    void loadRecentRuns();
  }, []);

  const filteredGroups = useMemo(
    () => employeeGroups.filter((group) => matchesEmployeeHubTeamFilter(group, teamFilter)),
    [employeeGroups, teamFilter],
  );
  const filteredRuns = useMemo(
    () => recentRuns.filter((run) => matchesEmployeeHubRunFilter(run, runFilter)),
    [recentRuns, runFilter],
  );

  async function reloadGroupsAndRuns() {
    await Promise.all([loadEmployeeGroups(), loadRecentRuns()]);
  }

  async function createEmployeeGroup() {
    const name = groupName.trim();
    const coordinator = groupCoordinatorId.trim();
    const members = Array.from(new Set(groupMemberIds.map((item) => item.trim()).filter((item) => item.length > 0)));
    const entryEmployeeId = (groupEntryId.trim() || coordinator).trim();
    const plannerEmployeeId = (groupPlannerId.trim() || entryEmployeeId || coordinator).trim();
    const reviewerEmployeeId = groupReviewerId.trim();
    const reviewMode = groupReviewMode.trim() || "none";
    const executionMode = groupExecutionMode.trim() || "sequential";
    const visibilityMode = groupVisibilityMode.trim() || "internal";

    if (!name) {
      setMessage("请填写群组名称");
      return;
    }
    if (!coordinator) {
      setMessage("请先选择协调员");
      return;
    }
    if (members.length === 0) {
      setMessage("请至少选择 1 个成员");
      return;
    }
    if (members.length > 10) {
      setMessage("群组成员最多 10 人");
      return;
    }
    if (!members.includes(coordinator)) {
      setMessage("协调员必须包含在群组成员中");
      return;
    }
    if (entryEmployeeId && !members.includes(entryEmployeeId)) {
      setMessage("入口员工必须包含在团队成员中");
      return;
    }
    if (plannerEmployeeId && !members.includes(plannerEmployeeId)) {
      setMessage("规划员工必须包含在团队成员中");
      return;
    }
    if (reviewMode !== "none" && !reviewerEmployeeId) {
      setMessage("开启审核后必须选择审核员工");
      return;
    }
    if (reviewerEmployeeId && !members.includes(reviewerEmployeeId)) {
      setMessage("审核员工必须包含在团队成员中");
      return;
    }

    setGroupSubmitting(true);
    setMessage("");
    try {
      await invoke<string>("create_employee_team", {
        input: {
          name,
          coordinator_employee_id: coordinator,
          member_employee_ids: members,
          entry_employee_id: entryEmployeeId,
          planner_employee_id: plannerEmployeeId,
          reviewer_employee_id: reviewerEmployeeId,
          review_mode: reviewMode,
          execution_mode: executionMode,
          visibility_mode: visibilityMode,
        },
      });
      setGroupName("");
      setGroupCoordinatorId("");
      setGroupMemberIds([]);
      setGroupEntryId("");
      setGroupPlannerId("");
      setGroupReviewerId("");
      setGroupReviewMode("none");
      setGroupExecutionMode("sequential");
      setGroupVisibilityMode("internal");
      await reloadGroupsAndRuns();
      await onEmployeeGroupsChanged?.();
      setMessage("协作团队已创建");
    } catch (e) {
      setMessage(`创建群组失败: ${String(e)}`);
    } finally {
      setGroupSubmitting(false);
    }
  }

  async function deleteEmployeeGroup(groupId: string) {
    if (!groupId) return;
    setGroupDeletingId(groupId);
    try {
      await invoke("delete_employee_group", { groupId });
      setGroupRunGoalById((prev) => {
        const next = { ...prev };
        delete next[groupId];
        return next;
      });
      setGroupRunReportById((prev) => {
        const next = { ...prev };
        delete next[groupId];
        return next;
      });
      setGroupRulesById((prev) => {
        const next = { ...prev };
        delete next[groupId];
        return next;
      });
      await reloadGroupsAndRuns();
      await onEmployeeGroupsChanged?.();
      setMessage("协作群组已删除");
    } catch (e) {
      setMessage(`删除群组失败: ${String(e)}`);
    } finally {
      setGroupDeletingId(null);
    }
  }

  async function startEmployeeGroupRun(groupId: string) {
    if (!groupId || groupRunSubmittingId) return;
    const userGoal = (groupRunGoalById[groupId] || "").trim();
    if (!userGoal) {
      setMessage("请先填写协作指令");
      return;
    }
    setGroupRunSubmittingId(groupId);
    setMessage("");
    try {
      const result = await invoke<EmployeeGroupRunResult>("start_employee_group_run", {
        input: {
          group_id: groupId,
          user_goal: userGoal,
          execution_window: 3,
          max_retry_per_step: 1,
          timeout_employee_ids: [],
        },
      });
      setGroupRunReportById((prev) => ({
        ...prev,
        [groupId]: result.final_report || "",
      }));
      await loadRecentRuns();
      if (result.session_id && result.session_skill_id) {
        await onOpenGroupRunSession?.(result.session_id, result.session_skill_id);
      }
      if ((result.state || "").trim().toLowerCase() === "waiting_review") {
        setMessage("协作任务已启动，等待审核");
      } else if ((result.state || "").trim().toLowerCase() === "done") {
        setMessage(`协作任务已完成（第 ${result.current_round || 1} 轮）`);
      } else {
        setMessage(`协作任务已启动，当前状态：${result.state || "执行"}`);
      }
    } catch (e) {
      setMessage(`发起协作失败: ${String(e)}`);
    } finally {
      setGroupRunSubmittingId(null);
    }
  }

  async function cloneEmployeeGroup(group: EmployeeGroup) {
    if (!group.id || cloningGroupId) return;
    const cloneName = `${group.name}（副本）`;
    setCloningGroupId(group.id);
    setMessage("");
    try {
      await invoke<string>("clone_employee_group_template", {
        input: {
          source_group_id: group.id,
          name: cloneName,
        },
      });
      await reloadGroupsAndRuns();
      await onEmployeeGroupsChanged?.();
      setMessage(`已复制团队：${cloneName}`);
    } catch (e) {
      setMessage(`复制团队失败: ${String(e)}`);
    } finally {
      setCloningGroupId(null);
    }
  }

  function handleGroupMemberToggle(employeeCode: string, checked: boolean) {
    if (checked) {
      setGroupMemberIds((prev) => {
        if (prev.includes(employeeCode)) return prev;
        if (prev.length >= 10) {
          setMessage("群组成员最多 10 人");
          return prev;
        }
        return [...prev, employeeCode];
      });
      return;
    }
    setGroupMemberIds((prev) => prev.filter((id) => id !== employeeCode));
  }

  function handleGroupRunGoalChange(groupId: string, value: string) {
    setGroupRunGoalById((prev) => ({ ...prev, [groupId]: value }));
  }

  return {
    groupName,
    setGroupName,
    groupCoordinatorId,
    setGroupCoordinatorId,
    groupMemberIds,
    groupEntryId,
    setGroupEntryId,
    groupPlannerId,
    setGroupPlannerId,
    groupReviewerId,
    setGroupReviewerId,
    groupReviewMode,
    setGroupReviewMode,
    groupExecutionMode,
    setGroupExecutionMode,
    groupVisibilityMode,
    setGroupVisibilityMode,
    employeeGroups,
    recentRuns,
    teamFilter,
    setTeamFilter,
    runFilter,
    setRunFilter,
    groupSubmitting,
    groupDeletingId,
    groupRunGoalById,
    groupRunSubmittingId,
    groupRunReportById,
    groupRulesById,
    cloningGroupId,
    filteredGroups,
    filteredRuns,
    createEmployeeGroup,
    deleteEmployeeGroup,
    startEmployeeGroupRun,
    cloneEmployeeGroup,
    handleGroupMemberToggle,
    handleGroupRunGoalChange,
  };
}
