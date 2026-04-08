import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();
const saveMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: (...args: unknown[]) => saveMock(...args),
}));

describe("EmployeeHubView memory governance", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    saveMock.mockReset();
    saveMock.mockResolvedValue("D:\\exports\\sales-memory.json");
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\.workclaw\\workspace" });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") {
        return Promise.resolve("C:\\Users\\test\\.workclaw\\workspace");
      }
      if (command === "get_openclaw_plugin_feishu_runtime_status") {
        return Promise.resolve({
          plugin_id: "@larksuite/openclaw-lark",
          account_id: "default",
          running: true,
          started_at: "2026-03-04T00:00:00Z",
          last_error: null,
          last_event_at: null,
          recent_logs: [],
        });
      }
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({
          relay: { running: true, generation: 1, interval_ms: 1500, total_accepted: 0, last_error: null },
          sidecar: { running: true, started_at: null, queued_events: 0, running_count: 0, items: [] },
        });
      }
      if (command === "get_employee_memory_stats") {
        if (payload?.skillId === "skill-sales") {
          return Promise.resolve({
            employee_id: "sales_lead",
            total_files: 1,
            total_bytes: 120,
            skills: [{ skill_id: "skill-sales", total_files: 1, total_bytes: 120 }],
          });
        }
        return Promise.resolve({
          employee_id: "sales_lead",
          total_files: 3,
          total_bytes: 300,
          skills: [
            { skill_id: "skill-sales", total_files: 2, total_bytes: 220 },
            { skill_id: "skill-support", total_files: 1, total_bytes: 80 },
          ],
        });
      }
      if (command === "export_employee_memory") {
        return Promise.resolve({
          employee_id: "sales_lead",
          skill_id: payload?.skillId ?? null,
          exported_at: "2026-03-04T00:00:00Z",
          total_files: 3,
          total_bytes: 300,
          files: [],
        });
      }
      if (command === "write_export_file") return Promise.resolve(null);
      if (command === "clear_employee_memory") {
        return Promise.resolve({
          employee_id: "sales_lead",
          total_files: 0,
          total_bytes: 0,
          skills: [],
        });
      }
      return Promise.resolve(null);
    });
  });

  test("supports refresh export and clear for selected employee memory", async () => {
    render(
      <EmployeeHubView
        employees={[
          {
            id: "emp-sales",
            employee_id: "sales_lead",
            name: "销售主管",
            role_id: "sales_lead",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "skill-sales",
            default_work_dir: "",
            openclaw_agent_id: "sales_lead",
            routing_priority: 100,
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: false,
            skill_ids: ["skill-sales", "skill-support"],
            created_at: "2026-03-01T00:00:00Z",
            updated_at: "2026-03-01T00:00:00Z",
          },
        ]}
        skills={[
          {
            id: "builtin-general",
            name: "通用助手",
            description: "",
            version: "1.0.0",
            author: "",
            recommended_model: "",
            tags: [],
            created_at: "2026-03-01T00:00:00Z",
          },
          {
            id: "skill-sales",
            name: "销售助手",
            description: "",
            version: "1.0.0",
            author: "",
            recommended_model: "",
            tags: [],
            created_at: "2026-03-01T00:00:00Z",
          },
        ]}
        selectedEmployeeId="emp-sales"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_employee_memory_stats", {
        employeeId: "sales_lead",
        skillId: null,
      });
    });

    expect(screen.getByTestId("employee-memory-total-files")).toHaveTextContent("3");
    expect(screen.getByTestId("employee-memory-total-bytes")).toHaveTextContent("300");

    fireEvent.click(screen.getByTestId("employee-memory-export"));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_employee_memory", {
        employeeId: "sales_lead",
        skillId: null,
      });
      expect(saveMock).toHaveBeenCalled();
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("write_export_file", {
        path: "D:\\exports\\sales-memory.json",
        content: expect.any(String),
      });
    });

    fireEvent.click(screen.getByTestId("employee-memory-clear"));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "确认清空" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("clear_employee_memory", {
        employeeId: "sales_lead",
        skillId: null,
      });
    });
  });
});
