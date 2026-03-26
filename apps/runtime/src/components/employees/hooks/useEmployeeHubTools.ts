import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  AgentEmployee,
  AgentProfileFilesView,
  EmployeeMemoryExport,
  EmployeeMemoryStats,
} from "../../../types";

export interface UseEmployeeHubToolsArgs {
  selectedEmployee: AgentEmployee | null;
  selectedEmployeeId: string | null;
  selectedEmployeeMemoryId: string | null;
  setMessage: (message: string) => void;
}

export function useEmployeeHubTools({
  selectedEmployee,
  selectedEmployeeId,
  selectedEmployeeMemoryId,
  setMessage,
}: UseEmployeeHubToolsArgs) {
  const [memoryScopeSkillId, setMemoryScopeSkillId] = useState("__all__");
  const [memoryStats, setMemoryStats] = useState<EmployeeMemoryStats | null>(null);
  const [memoryLoading, setMemoryLoading] = useState(false);
  const [memoryActionLoading, setMemoryActionLoading] = useState<"export" | "clear" | null>(null);
  const [pendingClearMemory, setPendingClearMemory] = useState(false);
  const [profileView, setProfileView] = useState<AgentProfileFilesView | null>(null);
  const [profileLoading, setProfileLoading] = useState(false);

  useEffect(() => {
    setMemoryScopeSkillId("__all__");
    setMemoryStats(null);
    setPendingClearMemory(false);
  }, [selectedEmployeeId]);

  useEffect(() => {
    if (!selectedEmployee) {
      setProfileView(null);
      setProfileLoading(false);
      return;
    }

    let disposed = false;
    setProfileLoading(true);
    invoke<AgentProfileFilesView>("get_agent_profile_files", { employeeDbId: selectedEmployee.id })
      .then((view) => {
        if (!disposed) setProfileView(view);
      })
      .catch(() => {
        if (!disposed) setProfileView(null);
      })
      .finally(() => {
        if (!disposed) setProfileLoading(false);
      });

    return () => {
      disposed = true;
    };
  }, [selectedEmployee]);

  async function refreshEmployeeMemoryStats(scopeSkillId?: string) {
    if (!selectedEmployeeMemoryId) {
      setMemoryStats(null);
      return;
    }
    const normalizedSkillId = (scopeSkillId ?? memoryScopeSkillId) === "__all__" ? null : (scopeSkillId ?? memoryScopeSkillId);
    setMemoryLoading(true);
    try {
      const stats = await invoke<EmployeeMemoryStats>("get_employee_memory_stats", {
        employeeId: selectedEmployeeMemoryId,
        skillId: normalizedSkillId,
      });
      setMemoryStats(stats);
    } catch (e) {
      setMessage(`加载长期记忆统计失败: ${String(e)}`);
    } finally {
      setMemoryLoading(false);
    }
  }

  useEffect(() => {
    if (!selectedEmployeeMemoryId) {
      setMemoryStats(null);
      return;
    }
    void refreshEmployeeMemoryStats();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedEmployeeMemoryId, memoryScopeSkillId]);

  async function exportEmployeeMemory() {
    if (!selectedEmployeeMemoryId || memoryActionLoading) return;
    setMemoryActionLoading("export");
    try {
      const skillId = memoryScopeSkillId === "__all__" ? null : memoryScopeSkillId;
      const payload = await invoke<EmployeeMemoryExport>("export_employee_memory", {
        employeeId: selectedEmployeeMemoryId,
        skillId,
      });
      const filePath = await saveDialog({
        defaultPath: `employee-memory-${selectedEmployeeMemoryId}-${skillId || "all"}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!filePath) return;
      await invoke("write_export_file", {
        path: filePath,
        content: JSON.stringify(payload, null, 2),
      });
      setMessage("长期记忆已导出");
    } catch (e) {
      setMessage(`导出长期记忆失败: ${String(e)}`);
    } finally {
      setMemoryActionLoading(null);
    }
  }

  async function confirmClearEmployeeMemory() {
    if (!selectedEmployeeMemoryId || memoryActionLoading) return;
    setMemoryActionLoading("clear");
    try {
      const skillId = memoryScopeSkillId === "__all__" ? null : memoryScopeSkillId;
      const stats = await invoke<EmployeeMemoryStats>("clear_employee_memory", {
        employeeId: selectedEmployeeMemoryId,
        skillId,
      });
      setMemoryStats(stats);
      setMessage("长期记忆已清空");
    } catch (e) {
      setMessage(`清空长期记忆失败: ${String(e)}`);
    } finally {
      setMemoryActionLoading(null);
      setPendingClearMemory(false);
    }
  }

  return {
    memoryScopeSkillId,
    setMemoryScopeSkillId,
    memoryStats,
    memoryLoading,
    memoryActionLoading,
    pendingClearMemory,
    setPendingClearMemory,
    profileView,
    profileLoading,
    refreshEmployeeMemoryStats,
    exportEmployeeMemory,
    confirmClearEmployeeMemory,
  };
}
