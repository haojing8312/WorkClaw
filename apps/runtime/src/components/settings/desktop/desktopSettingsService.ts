import { invoke } from "@tauri-apps/api/core";
import type { RuntimePreferences } from "../../../types";

export interface DesktopLifecyclePaths {
  runtime_root_dir: string;
  pending_runtime_root_dir: string | null;
  last_runtime_migration_status: string | null;
  last_runtime_migration_message: string | null;
}

export interface DesktopCleanupResult {
  removed_files: number;
  removed_dirs: number;
}

export interface DesktopDiagnosticsStatus {
  diagnostics_dir: string;
  logs_dir: string;
  audit_dir: string;
  crashes_dir: string;
  exports_dir: string;
  current_run_id: string;
  abnormal_previous_run: boolean;
  last_clean_exit_at: string | null;
  latest_crash: {
    timestamp: string;
    message: string;
    run_id?: string | null;
  } | null;
}

export interface RuntimeLanguagePreferencesInput {
  default_language: string;
  immersive_translation_enabled: boolean;
  immersive_translation_display: string;
  immersive_translation_trigger: string;
  translation_engine: string;
  translation_model_id: string;
}

export interface DesktopRuntimePreferencesInput {
  launch_at_login: boolean;
  launch_minimized: boolean;
  close_to_tray: boolean;
  operation_permission_mode: "standard" | "full_access";
}

export const DEFAULT_RUNTIME_PREFERENCES: RuntimePreferences = {
  default_work_dir: "",
  default_language: "zh-CN",
  immersive_translation_enabled: true,
  immersive_translation_display: "translated_only",
  immersive_translation_trigger: "auto",
  translation_engine: "model_then_free",
  translation_model_id: "",
  launch_at_login: false,
  launch_minimized: false,
  close_to_tray: true,
  operation_permission_mode: "standard",
};

export function normalizeRuntimePreferences(raw: unknown): RuntimePreferences {
  const parsed = (raw ?? {}) as Partial<RuntimePreferences>;
  const immersiveDisplay =
    parsed.immersive_translation_display === "bilingual_inline"
      ? "bilingual_inline"
      : "translated_only";
  const triggerMode = parsed.immersive_translation_trigger === "manual" ? "manual" : "auto";
  const translationEngine =
    parsed.translation_engine === "model_only" || parsed.translation_engine === "free_only"
      ? parsed.translation_engine
      : "model_then_free";
  const translationModelId = typeof parsed.translation_model_id === "string" ? parsed.translation_model_id : "";
  const operationPermissionMode =
    parsed.operation_permission_mode === "full_access" ? "full_access" : "standard";
  return {
    default_work_dir: typeof parsed.default_work_dir === "string" ? parsed.default_work_dir : "",
    default_language:
      typeof parsed.default_language === "string" && parsed.default_language ? parsed.default_language : "zh-CN",
    immersive_translation_enabled:
      typeof parsed.immersive_translation_enabled === "boolean"
        ? parsed.immersive_translation_enabled
        : true,
    immersive_translation_display: immersiveDisplay,
    immersive_translation_trigger: triggerMode,
    translation_engine: translationEngine,
    translation_model_id: translationModelId,
    launch_at_login: typeof parsed.launch_at_login === "boolean" ? parsed.launch_at_login : false,
    launch_minimized: typeof parsed.launch_minimized === "boolean" ? parsed.launch_minimized : false,
    close_to_tray: typeof parsed.close_to_tray === "boolean" ? parsed.close_to_tray : true,
    operation_permission_mode: operationPermissionMode,
  };
}

export async function getRuntimePreferences() {
  return invoke<RuntimePreferences>("get_runtime_preferences");
}

export async function saveRuntimeLanguagePreferences(input: RuntimeLanguagePreferencesInput) {
  return invoke<RuntimePreferences>("set_runtime_preferences", { input });
}

export async function saveDesktopRuntimePreferences(input: DesktopRuntimePreferencesInput) {
  return invoke<RuntimePreferences>("set_runtime_preferences", { input });
}

export async function getDesktopLifecyclePaths() {
  return invoke<DesktopLifecyclePaths>("get_desktop_lifecycle_paths");
}

export async function scheduleDesktopRuntimeRootMigration(targetRoot: string) {
  return invoke("schedule_desktop_runtime_root_migration", { targetRoot });
}

export async function getDesktopDiagnosticsStatus() {
  return invoke<DesktopDiagnosticsStatus>("get_desktop_diagnostics_status");
}

export async function openDesktopPath(path: string) {
  return invoke("open_desktop_path", { path });
}

export async function clearDesktopCacheAndLogs() {
  return invoke<DesktopCleanupResult>("clear_desktop_cache_and_logs");
}

export async function exportDesktopEnvironmentSummary() {
  return invoke<string>("export_desktop_environment_summary");
}

export async function openDesktopDiagnosticsDir() {
  return invoke("open_desktop_diagnostics_dir");
}

export async function exportDesktopDiagnosticsBundle() {
  return invoke<string>("export_desktop_diagnostics_bundle");
}
