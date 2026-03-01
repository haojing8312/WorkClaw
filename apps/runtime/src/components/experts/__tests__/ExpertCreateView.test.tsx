import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ExpertCreateView } from "../ExpertCreateView";

async function answer(content: string) {
  fireEvent.change(screen.getByPlaceholderText("输入你的回答..."), {
    target: { value: content },
  });
  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  await waitFor(() => {
    expect(screen.getByPlaceholderText("输入你的回答...")).toBeInTheDocument();
  });
}

function renderPreviewMock() {
  return vi.fn().mockImplementation(async (payload: any) => {
    const name = payload.name || "expert-skill";
    const desc = payload.description || "Use when users need a reusable expert workflow.";
    const normalizedDescription = desc.toLowerCase().startsWith("use when")
      ? desc
      : `Use when ${desc}`;
    const when = payload.whenToUse || "需要在特定任务场景中提供稳定执行能力";
    const targetDir = payload.targetDir || "~/.skillmint/skills/";

    return {
      markdown: `---\nname: ${name}\ndescription: ${normalizedDescription}\n---\n\n## When to Use\n- ${when}\n`,
      savePath: `${targetDir}/generated-skill`,
    };
  });
}

describe("ExpertCreateView", () => {
  test("guides user step by step and updates preview", async () => {
    const onRenderPreview = renderPreviewMock();
    render(
      <ExpertCreateView
        saving={false}
        onBack={() => {}}
        onOpenPackaging={() => {}}
        onPickDirectory={async () => null}
        onSave={async () => {}}
        onRenderPreview={onRenderPreview}
      />
    );

    expect(screen.getByText("创建专家技能")).toBeInTheDocument();
    expect(screen.getByText("我会用对话方式帮你创建专家技能。先告诉我技能名称。")).toBeInTheDocument();
    await waitFor(() => {
      expect(onRenderPreview).toHaveBeenCalled();
    });

    await answer("我的测试技能");
    await waitFor(() => {
      expect(onRenderPreview).toHaveBeenCalledTimes(2);
    });
    await answer("用于整理本地文件");
    await waitFor(() => {
      expect(onRenderPreview).toHaveBeenCalledTimes(3);
    });
    await answer("需要整理大量文件时");
    await waitFor(() => {
      expect(onRenderPreview).toHaveBeenCalledTimes(4);
    });

    expect(screen.getByText("请输入保存目录，或点击下方按钮选择目录。你也可以使用默认目录。")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText(/name: 我的测试技能/)).toBeInTheDocument();
      expect(screen.getByText(/## When to Use/)).toBeInTheDocument();
    });
    expect(screen.getAllByText(/需要整理大量文件时/).length).toBeGreaterThan(0);
  });

  test("saves with default directory through dialog flow", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const onRenderPreview = renderPreviewMock();
    render(
      <ExpertCreateView
        saving={false}
        onBack={() => {}}
        onOpenPackaging={() => {}}
        onPickDirectory={async () => null}
        onSave={onSave}
        onRenderPreview={onRenderPreview}
      />
    );

    await answer("我的测试技能");
    await waitFor(() => {
      expect(onRenderPreview).toHaveBeenCalledTimes(2);
    });
    await answer("用于整理本地文件");
    await waitFor(() => {
      expect(onRenderPreview).toHaveBeenCalledTimes(3);
    });
    await answer("需要整理大量文件时");
    await waitFor(() => {
      expect(onRenderPreview).toHaveBeenCalledTimes(4);
    });

    fireEvent.click(screen.getByRole("button", { name: "使用默认目录" }));
    fireEvent.click(screen.getByRole("button", { name: "保存技能" }));

    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          name: "我的测试技能",
          description: "用于整理本地文件",
          whenToUse: "需要整理大量文件时",
          targetDir: undefined,
        })
      );
    });
  });
});
