import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { NewSessionLanding } from "../NewSessionLanding";
import type { SessionInfo } from "../../types";

function makeSession(id: string, title: string, createdAt?: Date): SessionInfo {
  return {
    id,
    title,
    created_at: (createdAt ?? new Date()).toISOString(),
    model_id: "test-model",
  };
}

describe("NewSessionLanding", () => {
  test("renders hero copy and capability intro", () => {
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={() => {}}
      />
    );

    expect(screen.getByText("你的电脑任务，交给打工虾们协作完成")).toBeInTheDocument();
    expect(
      screen.getByText(
        "一句话描述需求，它可以帮你创建和修改文件、分析本地数据、整理文件、操作浏览器，并持续反馈执行过程。"
      )
    ).toBeInTheDocument();

    expect(screen.getByText("创建/修改文件")).toBeInTheDocument();
    expect(screen.getByText("分析本地文件")).toBeInTheDocument();
    expect(screen.getByText("文件整理")).toBeInTheDocument();
    expect(screen.getByText("浏览器操作")).toBeInTheDocument();
  });

  test("submits input on button click", () => {
    const onCreate = vi.fn();
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={onCreate}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("先描述你要完成什么任务..."), {
      target: { value: "请帮我整理下载目录" },
    });
    fireEvent.click(screen.getByRole("button", { name: "开始任务" }));

    expect(onCreate).toHaveBeenCalledWith({
      initialMessage: "请帮我整理下载目录",
      attachments: [],
      workDir: "",
    });
  });

  test("submits on Enter and keeps newline on Shift+Enter", () => {
    const onCreate = vi.fn();
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={onCreate}
      />
    );

    const input = screen.getByPlaceholderText("先描述你要完成什么任务...");
    fireEvent.change(input, { target: { value: "第一行" } });

    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(onCreate).not.toHaveBeenCalled();

    fireEvent.keyDown(input, { key: "Enter", shiftKey: false });
    expect(onCreate).toHaveBeenCalledWith({
      initialMessage: "第一行",
      attachments: [],
      workDir: "",
    });
  });

  test("shows empty state when there is no recent session", () => {
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={() => {}}
      />
    );

    expect(screen.getByText("暂无会话，从上方输入任务开始")).toBeInTheDocument();
  });

  test("shows max 6 recent sessions and supports selecting session", () => {
    const sessions = Array.from({ length: 8 }, (_, i) => makeSession(`s-${i}`, `会话-${i}`));
    const onSelectSession = vi.fn();
    render(
      <NewSessionLanding
        sessions={sessions}
        teams={[]}
        creating={false}
        onSelectSession={onSelectSession}
        onCreateSessionWithInitialMessage={() => {}}
      />
    );

    expect(screen.getByText("会话-0")).toBeInTheDocument();
    expect(screen.getByText("会话-5")).toBeInTheDocument();
    expect(screen.queryByText("会话-6")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "会话-1" }));
    expect(onSelectSession).toHaveBeenCalledWith("s-1");
  });

  test("groups recent sessions by time buckets", () => {
    const now = new Date();
    const day = 24 * 60 * 60 * 1000;
    const sessions = [
      makeSession("today-1", "今天任务", new Date(now.getTime() - 2 * 60 * 60 * 1000)),
      makeSession("week-1", "周内任务", new Date(now.getTime() - 3 * day)),
      makeSession("old-1", "更早任务", new Date(now.getTime() - 12 * day)),
    ];

    render(
      <NewSessionLanding
        sessions={sessions}
        teams={[]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={() => {}}
      />
    );

    expect(screen.getByText("今天")).toBeInTheDocument();
    expect(screen.getByText("最近7天")).toBeInTheDocument();
    expect(screen.getByText("更早")).toBeInTheDocument();
    expect(screen.getByText("今天任务")).toBeInTheDocument();
    expect(screen.getByText("周内任务")).toBeInTheDocument();
    expect(screen.getByText("更早任务")).toBeInTheDocument();
  });

  test("shows creating state and error text", () => {
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={true}
        error="创建失败"
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={() => {}}
      />
    );

    expect(screen.getByRole("button", { name: "正在创建..." })).toBeDisabled();
    expect(screen.getByText("创建失败")).toBeInTheDocument();
  });

  test("renders four scenario cards", () => {
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={() => {}}
      />
    );

    expect(screen.getByRole("button", { name: /文件整理助手/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /本地数据汇总/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /浏览器信息采集/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /代码问题排查/ })).toBeInTheDocument();
  });

  test("fills textarea when scenario is selected and only submits on explicit create", () => {
    const onCreate = vi.fn();
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={onCreate}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /文件整理助手/ }));
    const input = screen.getByPlaceholderText("先描述你要完成什么任务...");
    expect(input).toHaveValue(
      "请帮我整理下载目录，把文件按类型分类到子文件夹，并按近30天和更早文件分开。先告诉我你的整理方案。"
    );
    expect(onCreate).not.toHaveBeenCalled();
    expect(screen.getByText("已填入场景示例，你可以继续修改后再开始任务")).toBeInTheDocument();

    const selected = screen.getByRole("button", { name: /文件整理助手/ });
    expect(selected).toHaveAttribute("aria-pressed", "true");

    const unselected = screen.getByRole("button", { name: /本地数据汇总/ });
    expect(unselected).toHaveAttribute("aria-pressed", "false");

    fireEvent.click(screen.getByRole("button", { name: "开始任务" }));
    expect(onCreate).toHaveBeenCalledWith({
      initialMessage: "请帮我整理下载目录，把文件按类型分类到子文件夹，并按近30天和更早文件分开。先告诉我你的整理方案。",
      attachments: [],
      workDir: "",
    });
  });

  test("renders explicit team entry section and dispatches chosen team", () => {
    const onCreateTeamEntrySession = vi.fn();
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[
          {
            id: "group-1",
            name: "默认复杂任务团队",
            description: "入口：太子 · 协调：尚书省",
            memberCount: 10,
          },
        ]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={() => {}}
        onCreateTeamEntrySession={onCreateTeamEntrySession}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("先描述你要完成什么任务..."), {
      target: { value: "请安排一份调研和执行计划" },
    });

    expect(screen.getByText("团队协作入口")).toBeInTheDocument();
    expect(screen.getByText("默认复杂任务团队")).toBeInTheDocument();
    expect(screen.getByText("入口：太子 · 协调：尚书省")).toBeInTheDocument();
    expect(screen.getByText("10 人团队")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "交给团队处理：默认复杂任务团队" }));

    expect(onCreateTeamEntrySession).toHaveBeenCalledWith({
      teamId: "group-1",
      initialMessage: "请安排一份调研和执行计划",
    });
  });

  test("renders attachment and workdir controls", () => {
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={() => {}}
      />
    );

    expect(screen.getByLabelText("添加附件")).toBeInTheDocument();
    expect(screen.getByTestId("landing-attachment-trigger")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "选择工作目录" })).toBeInTheDocument();
  });

  test("shows the default workdir path with chat-like truncation and preserves full path in title", () => {
    const fullPath = "D:\\code\\WorkClaw\\very-long-project-name";
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={false}
        defaultWorkDir={fullPath}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={() => {}}
      />
    );

    const workdirButton = screen.getByRole("button", { name: fullPath });
    expect(workdirButton).toHaveAttribute("title", fullPath);
    expect(screen.getByTestId("landing-workdir-label")).toHaveClass("max-w-[150px]", "truncate");
  });

  test("picks a workdir and includes it in submit payload", async () => {
    const onCreate = vi.fn();
    const onPickWorkDir = vi.fn().mockResolvedValue("D:\\code\\WorkClaw");
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={onCreate}
        onPickWorkDir={onPickWorkDir}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("先描述你要完成什么任务..."), {
      target: { value: "分析当前项目目录" },
    });
    fireEvent.click(screen.getByRole("button", { name: "选择工作目录" }));

    expect(onPickWorkDir).toHaveBeenCalledTimes(1);
    const workdirButton = await screen.findByRole("button", { name: "D:\\code\\WorkClaw" });
    expect(workdirButton).toHaveAttribute("title", "D:\\code\\WorkClaw");
    expect(screen.getByTestId("landing-workdir-label")).toHaveClass("max-w-[150px]", "truncate");

    fireEvent.click(screen.getByRole("button", { name: "开始任务" }));
    expect(onCreate).toHaveBeenCalledWith({
      initialMessage: "分析当前项目目录",
      attachments: [],
      workDir: "D:\\code\\WorkClaw",
    });
  });

  test("shows attachment summary and includes attachments in submit payload", async () => {
    const onCreate = vi.fn();
    render(
      <NewSessionLanding
        sessions={[]}
        teams={[]}
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={onCreate}
      />
    );

    const input = screen.getByLabelText("添加附件") as HTMLInputElement;
    const file = new File(["hello"], "需求说明.txt", { type: "text/plain" });
    fireEvent.change(input, { target: { files: [file] } });

    await waitFor(() => {
      expect(screen.getByText("已添加 1 个附件")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText("先描述你要完成什么任务..."), {
      target: { value: "请结合附件处理" },
    });
    fireEvent.click(screen.getByRole("button", { name: "开始任务" }));

    expect(onCreate).toHaveBeenCalledWith({
      initialMessage: "请结合附件处理",
      attachments: [
        expect.objectContaining({
          kind: "text-file",
          name: "需求说明.txt",
          mimeType: "text/plain",
          text: "hello",
        }),
      ],
      workDir: "",
    });
  });
});
