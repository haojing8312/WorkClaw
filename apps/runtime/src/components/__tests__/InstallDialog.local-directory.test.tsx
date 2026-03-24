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

describe("InstallDialog local directory mode", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();
  });

  test("shows local root directory copy and invokes local import", async () => {
    openMock.mockResolvedValueOnce("C:\\skills");
    invokeMock.mockResolvedValue({
      installed: [
        {
          dir_path: "C:\\skills\\writer",
          manifest: {
            id: "local-writer",
            name: "Writer",
          },
        },
      ],
      failed: [],
      missing_mcp: [],
    });

    const onInstalled = vi.fn();
    const onClose = vi.fn();
    render(<InstallDialog onInstalled={onInstalled} onClose={onClose} />);

    fireEvent.click(screen.getByRole("button", { name: "本地目录" }));
    expect(screen.getByRole("button", { name: "选择 Skill 目录或 skills 根目录" })).toBeInTheDocument();
    expect(screen.getByText(/最多向下扫描一层/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "选择 Skill 目录或 skills 根目录" }));

    await waitFor(() => {
      expect(screen.getByText("skills")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "安装" }));
    fireEvent.click(screen.getByRole("button", { name: "确认安装" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("import_local_skill", {
        dirPath: "C:\\skills",
      });
      expect(onInstalled).toHaveBeenCalledWith("local-writer");
      expect(onClose).toHaveBeenCalled();
    });
  });

  test("closes after importing multiple skills and uses the first installed id", async () => {
    openMock.mockResolvedValueOnce("C:\\skills");
    invokeMock.mockResolvedValue({
      installed: [
        {
          dir_path: "C:\\skills\\planner",
          manifest: {
            id: "local-planner",
            name: "Planner",
          },
        },
        {
          dir_path: "C:\\skills\\writer",
          manifest: {
            id: "local-writer",
            name: "Writer",
          },
        },
      ],
      failed: [],
      missing_mcp: [],
    });

    const onInstalled = vi.fn();
    const onClose = vi.fn();
    render(<InstallDialog onInstalled={onInstalled} onClose={onClose} />);

    fireEvent.click(screen.getByRole("button", { name: "本地目录" }));
    fireEvent.click(screen.getByRole("button", { name: "选择 Skill 目录或 skills 根目录" }));

    await waitFor(() => {
      expect(screen.getByText("skills")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "安装" }));
    fireEvent.click(screen.getByRole("button", { name: "确认安装" }));

    await waitFor(() => {
      expect(onInstalled).toHaveBeenCalledWith("local-planner");
      expect(onClose).toHaveBeenCalled();
    });
  });

  test("keeps the dialog open to show missing MCP warnings after partial success", async () => {
    openMock.mockResolvedValueOnce("C:\\skills");
    invokeMock.mockResolvedValue({
      installed: [
        {
          dir_path: "C:\\skills\\writer",
          manifest: {
            id: "local-writer",
            name: "Writer",
          },
        },
      ],
      failed: [
        {
          dir_path: "C:\\skills\\broken",
          name_hint: "broken",
          error: "DUPLICATE_SKILL_NAME:Writer",
        },
      ],
      missing_mcp: ["filesystem", "browser"],
    });

    const onInstalled = vi.fn();
    const onClose = vi.fn();
    render(<InstallDialog onInstalled={onInstalled} onClose={onClose} />);

    fireEvent.click(screen.getByRole("button", { name: "本地目录" }));
    fireEvent.click(screen.getByRole("button", { name: "选择 Skill 目录或 skills 根目录" }));

    await waitFor(() => {
      expect(screen.getByText("skills")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "安装" }));
    fireEvent.click(screen.getByRole("button", { name: "确认安装" }));

    await waitFor(() => {
      expect(screen.getByText("filesystem")).toBeInTheDocument();
      expect(screen.getByText("browser")).toBeInTheDocument();
      expect(onInstalled).toHaveBeenCalledWith("local-writer");
      expect(onClose).not.toHaveBeenCalled();
    });
  });
});
