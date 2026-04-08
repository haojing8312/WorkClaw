import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve(null)),
}));

function createRuntimePreferences() {
  return {
    default_work_dir: "E:\\workspace",
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
}

describe("SettingsView data retention", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_model_configs") return Promise.resolve([]);
      if (command === "list_mcp_servers") return Promise.resolve([]);
      if (command === "list_search_configs") return Promise.resolve([]);
      if (command === "list_provider_configs") return Promise.resolve([]);
      if (command === "get_runtime_preferences") {
        return Promise.resolve(createRuntimePreferences());
      }
      if (command === "get_desktop_lifecycle_paths") {
        return Promise.resolve({
          runtime_root_dir: "C:\\Users\\me\\.workclaw",
          pending_runtime_root_dir: null,
          last_runtime_migration_status: null,
          last_runtime_migration_message: null,
        });
      }
      if (command === "get_desktop_diagnostics_status") {
        return Promise.resolve({
          diagnostics_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics",
          logs_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics\\logs",
          audit_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics\\audit",
          crashes_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics\\crashes",
          exports_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics\\exports",
          current_run_id: "run-1",
          abnormal_previous_run: true,
          last_clean_exit_at: "2026-03-13T09:59:00Z",
          latest_crash: {
            timestamp: "2026-03-13T10:00:00Z",
            message: "panic occurred",
            run_id: "run-0",
          },
        });
      }
      if (command === "clear_desktop_cache_and_logs") {
        return Promise.resolve({
          removed_files: 12,
          removed_dirs: 3,
        });
      }
      if (command === "export_desktop_environment_summary") {
        return Promise.resolve("# Environment Summary");
      }
      if (command === "open_desktop_path") {
        return Promise.resolve(null);
      }
      if (command === "open_desktop_diagnostics_dir") {
        return Promise.resolve(null);
      }
      if (command === "export_desktop_diagnostics_bundle") {
        return Promise.resolve(
          "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics\\exports\\diagnostics-run-1.zip",
        );
      }
      return Promise.resolve(null);
    });
  });

  test("shows the unified runtime root, uninstall guidance and maintenance actions", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(await screen.findByRole("button", { name: "桌面 / 系统" }));

    expect(await screen.findByText("数据根目录")).toBeInTheDocument();
    expect(screen.getByText("C:\\Users\\me\\.workclaw")).toBeInTheDocument();
    expect(screen.queryByText("应用数据目录")).not.toBeInTheDocument();
    expect(screen.queryByText("缓存目录")).not.toBeInTheDocument();
    expect(screen.queryByText("默认工作目录")).not.toBeInTheDocument();
    expect(screen.getByText("检测到上次运行可能异常退出")).toBeInTheDocument();
    expect(screen.getByText(/panic occurred/)).toBeInTheDocument();
    expect(screen.getByText("卸载程序不会删除你的数据根目录。")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    fireEvent.click(screen.getByRole("button", { name: "清理缓存与日志" }));
    fireEvent.click(screen.getByRole("button", { name: "导出诊断包" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_desktop_diagnostics_bundle");
    });

    fireEvent.click(screen.getByRole("button", { name: "导出环境摘要" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_desktop_path", {
        path: "C:\\Users\\me\\.workclaw",
      });
      expect(invokeMock).toHaveBeenCalledWith("clear_desktop_cache_and_logs");
      expect(invokeMock).toHaveBeenCalledWith("export_desktop_environment_summary");
      expect(invokeMock).toHaveBeenCalledWith("export_desktop_diagnostics_bundle");
    });
  });
});
