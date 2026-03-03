import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { InstallDialog } from "../InstallDialog";

const invokeMock = vi.fn();
const openMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openMock(...args),
}));

describe("InstallDialog industry pack mode", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();
  });

  test("checks update and installs industry bundle without auto session creation", async () => {
    openMock.mockResolvedValueOnce("C:\\packs\\teacher-suite.industrypack");
    invokeMock.mockImplementation((command: string) => {
      if (command === "check_industry_bundle_update") {
        return Promise.resolve({
          pack_id: "edu-teacher-suite",
          current_version: "1.0.0",
          candidate_version: "1.2.0",
          has_update: true,
          message: "发现新版本：1.0.0 -> 1.2.0",
        });
      }
      if (command === "install_industry_bundle") {
        return Promise.resolve({
          pack_id: "edu-teacher-suite",
          version: "1.2.0",
          installed_skills: [{ id: "local-edu-teacher-suite--teacher-helper", name: "Teacher Helper" }],
          missing_mcp: [],
        });
      }
      return Promise.resolve(null);
    });

    const onInstalled = vi.fn();
    const onClose = vi.fn();
    render(<InstallDialog onInstalled={onInstalled} onClose={onClose} />);

    fireEvent.click(screen.getByRole("button", { name: "行业包" }));
    fireEvent.click(screen.getByRole("button", { name: "选择 .industrypack 文件" }));

    await waitFor(() => {
      expect(screen.getByText("teacher-suite.industrypack")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "检查更新" }));

    await waitFor(() => {
      expect(screen.getByText("发现新版本：1.0.0 -> 1.2.0")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "安装" }));
    fireEvent.click(screen.getByRole("button", { name: "确认安装" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("install_industry_bundle", {
        bundlePath: "C:\\packs\\teacher-suite.industrypack",
        installRoot: null,
      });
      expect(onInstalled).toHaveBeenCalledWith(
        "local-edu-teacher-suite--teacher-helper",
        { createSession: false }
      );
      expect(onClose).toHaveBeenCalled();
    });
  });
});
