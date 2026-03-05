import { fireEvent, render, screen } from "@testing-library/react";
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
        creating={false}
        onSelectSession={() => {}}
        onCreateSessionWithInitialMessage={onCreate}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("先描述你要完成什么任务..."), {
      target: { value: "请帮我整理下载目录" },
    });
    fireEvent.click(screen.getByRole("button", { name: "开始任务" }));

    expect(onCreate).toHaveBeenCalledWith("请帮我整理下载目录");
  });

  test("submits on Enter and keeps newline on Shift+Enter", () => {
    const onCreate = vi.fn();
    render(
      <NewSessionLanding
        sessions={[]}
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
    expect(onCreate).toHaveBeenCalledWith("第一行");
  });

  test("shows empty state when there is no recent session", () => {
    render(
      <NewSessionLanding
        sessions={[]}
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
    expect(onCreate).toHaveBeenCalledWith(
      "请帮我整理下载目录，把文件按类型分类到子文件夹，并按近30天和更早文件分开。先告诉我你的整理方案。"
    );
  });
});
