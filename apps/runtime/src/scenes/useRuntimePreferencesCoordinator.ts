import { invoke } from "@tauri-apps/api/core";
import { useCallback, useState } from "react";
import type { RuntimePreferences } from "../types";

const DEFAULT_OPERATION_PERMISSION_MODE: "standard" | "full_access" =
  "standard";

export function useRuntimePreferencesCoordinator() {
  const [defaultWorkDir, setDefaultWorkDir] = useState("");
  const [operationPermissionMode, setOperationPermissionMode] = useState<
    "standard" | "full_access"
  >(DEFAULT_OPERATION_PERMISSION_MODE);

  const loadRuntimePreferences = useCallback(async () => {
    try {
      const prefs = await invoke<RuntimePreferences>("get_runtime_preferences");
      if (!prefs || typeof prefs !== "object") {
        setDefaultWorkDir("");
        setOperationPermissionMode(DEFAULT_OPERATION_PERMISSION_MODE);
        return;
      }
      setDefaultWorkDir(
        typeof prefs.default_work_dir === "string"
          ? prefs.default_work_dir.trim()
          : "",
      );
      setOperationPermissionMode(
        prefs.operation_permission_mode === "full_access"
          ? "full_access"
          : "standard",
      );
    } catch (error) {
      console.warn("加载运行时偏好失败:", error);
      setDefaultWorkDir("");
      setOperationPermissionMode(DEFAULT_OPERATION_PERMISSION_MODE);
    }
  }, []);

  const resolveSessionLaunchWorkDir = useCallback(
    async (preferredWorkDir?: string): Promise<string> => {
      const normalizedPreferred = (preferredWorkDir || "").trim();
      if (normalizedPreferred) {
        return normalizedPreferred;
      }
      const normalizedDefault = defaultWorkDir.trim();
      if (normalizedDefault) {
        return normalizedDefault;
      }
      try {
        const prefs = await invoke<RuntimePreferences>("get_runtime_preferences");
        const resolvedDefault =
          prefs &&
          typeof prefs === "object" &&
          typeof prefs.default_work_dir === "string"
            ? prefs.default_work_dir.trim()
            : "";
        if (resolvedDefault) {
          setDefaultWorkDir(resolvedDefault);
        }
        return resolvedDefault;
      } catch (error) {
        console.warn("加载默认工作目录失败:", error);
        return "";
      }
    },
    [defaultWorkDir],
  );

  return {
    defaultWorkDir,
    loadRuntimePreferences,
    operationPermissionMode,
    resolveSessionLaunchWorkDir,
    setDefaultWorkDir,
    setOperationPermissionMode,
  };
}
