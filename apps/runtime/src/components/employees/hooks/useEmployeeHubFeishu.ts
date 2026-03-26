import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AgentEmployee,
  ImRoutingBinding,
  OpenClawPluginFeishuRuntimeStatus,
  SaveFeishuEmployeeAssociationInput,
} from "../../../types";

export interface UseEmployeeHubFeishuArgs {
  selectedEmployee: AgentEmployee | null;
  onRefreshEmployees?: () => Promise<AgentEmployee[] | void> | AgentEmployee[] | void;
  setMessage: (message: string) => void;
  setEmployeeScopeOverrides: Dispatch<SetStateAction<Record<string, string[]>>>;
}

export function toEmployeeHubFeishuRuntimeStatus(
  status: OpenClawPluginFeishuRuntimeStatus | null,
): {
  queued_events: number;
  reconnect_attempts: number;
  last_event_at: string | null;
  last_error: string | null;
} | null {
  if (!status) return null;
  return {
    queued_events: 0,
    reconnect_attempts: 0,
    last_event_at: status.last_event_at ?? null,
    last_error: status.last_error ?? null,
  };
}

export function useEmployeeHubFeishu({
  selectedEmployee,
  onRefreshEmployees,
  setMessage,
  setEmployeeScopeOverrides,
}: UseEmployeeHubFeishuArgs) {
  const [routingBindings, setRoutingBindings] = useState<ImRoutingBinding[]>([]);
  const [savingFeishuAssociation, setSavingFeishuAssociation] = useState(false);

  useEffect(() => {
    let disposed = false;
    const loadBindings = async () => {
      try {
        const bindings = await invoke<ImRoutingBinding[]>("list_im_routing_bindings");
        if (!disposed) {
          setRoutingBindings(Array.isArray(bindings) ? bindings : []);
        }
      } catch {
        if (!disposed) {
          setRoutingBindings([]);
        }
      }
    };
    void loadBindings();
    return () => {
      disposed = true;
    };
  }, []);

  function resolveFeishuStatus(
    employee: AgentEmployee,
    officialFeishuRuntimeStatus: OpenClawPluginFeishuRuntimeStatus | null,
  ): { dotClass: string; label: string; detail: string; error: string } {
    const enabled = !!employee.enabled;
    const agentId = (employee.openclaw_agent_id || employee.employee_id || employee.role_id || "").trim().toLowerCase();
    const hasFeishuBinding = routingBindings.some(
      (binding) =>
        binding.enabled &&
        binding.channel === "feishu" &&
        binding.agent_id.trim().toLowerCase() === agentId,
    );
    const receivesFeishu = employee.enabled_scopes.includes("feishu") || hasFeishuBinding;
    if (!enabled) {
      return { dotClass: "bg-gray-300", label: "未启用飞书消息", detail: "该员工已停用，不接收飞书事件。", error: "" };
    }
    if (!receivesFeishu) {
      return { dotClass: "bg-gray-300", label: "未关联飞书接待", detail: "请在员工详情中启用飞书接待。", error: "" };
    }
    const running = officialFeishuRuntimeStatus?.running === true;
    if (running && !officialFeishuRuntimeStatus?.last_error?.trim()) {
      return {
        dotClass: "bg-emerald-500",
        label: "飞书接入正常",
        detail: "官方插件宿主已运行，飞书接待规则已生效。",
        error: "",
      };
    }
    const error =
      officialFeishuRuntimeStatus?.last_error?.trim() ||
      (!running ? "官方插件宿主未运行" : "飞书消息桥接未运行");
    if (!running) {
      return {
        dotClass: "bg-amber-500",
        label: "待启动飞书接入",
        detail: "请前往设置中心中的飞书连接页面检查官方插件状态。",
        error,
      };
    }
    return {
      dotClass: "bg-red-500",
      label: "飞书接入异常",
      detail: "请检查官方插件运行状态或员工接待规则。",
      error,
    };
  }

  async function saveFeishuAssociation(input: {
    enabled: boolean;
    mode: "default" | "scoped";
    peerKind: "group" | "channel" | "direct";
    peerId: string;
    priority: number;
  }) {
    if (!selectedEmployee) return;
    if (!selectedEmployee.id.trim()) {
      setMessage("员工编号缺失，无法保存飞书接待");
      return;
    }
    setSavingFeishuAssociation(true);
    setMessage("");
    try {
      const scopes = new Set(selectedEmployee.enabled_scopes?.length ? selectedEmployee.enabled_scopes : ["app"]);
      if (input.enabled) {
        scopes.add("feishu");
      } else {
        scopes.delete("feishu");
      }
      if (scopes.size === 0) {
        scopes.add("app");
      }
      const nextScopes = Array.from(scopes.values());
      const payload: SaveFeishuEmployeeAssociationInput = {
        employee_db_id: selectedEmployee.id,
        enabled: input.enabled,
        mode: input.mode,
        peer_kind: input.mode === "default" ? "group" : input.peerKind,
        peer_id: input.mode === "default" ? "" : input.peerId.trim(),
        priority: input.priority,
      };
      await invoke("save_feishu_employee_association", { input: payload });

      const latestBindings = await invoke<ImRoutingBinding[]>("list_im_routing_bindings");
      setRoutingBindings(Array.isArray(latestBindings) ? latestBindings : []);
      setEmployeeScopeOverrides((current) => ({
        ...current,
        [selectedEmployee.id]: nextScopes,
      }));
      let refreshWarning = "";
      if (onRefreshEmployees) {
        try {
          await onRefreshEmployees();
          setEmployeeScopeOverrides((current) => {
            if (!(selectedEmployee.id in current)) return current;
            const next = { ...current };
            delete next[selectedEmployee.id];
            return next;
          });
        } catch (refreshError) {
          refreshWarning = `，员工列表刷新失败: ${String(refreshError)}`;
        }
      }
      setMessage(
        input.enabled
          ? `飞书接待已保存${refreshWarning}`
          : `已关闭该员工的飞书接待${refreshWarning}`,
      );
    } catch (e) {
      setMessage(`保存飞书接待失败: ${String(e)}`);
    } finally {
      setSavingFeishuAssociation(false);
    }
  }

  return {
    routingBindings,
    savingFeishuAssociation,
    resolveFeishuStatus,
    saveFeishuAssociation,
  };
}
