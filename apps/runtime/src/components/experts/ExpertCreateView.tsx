import { useEffect, useMemo, useState } from "react";

export interface ExpertCreatePayload {
  name: string;
  description: string;
  whenToUse: string;
  targetDir?: string;
}

export interface ExpertPreviewPayload {
  name: string;
  description: string;
  whenToUse: string;
  targetDir?: string;
}

export interface ExpertPreviewResult {
  markdown: string;
  savePath: string;
}

type Step = "name" | "description" | "when" | "dir" | "confirm";

interface ChatMessage {
  role: "assistant" | "user";
  content: string;
}

interface DraftState {
  name: string;
  description: string;
  whenToUse: string;
  targetDir: string;
}

interface Props {
  saving: boolean;
  error?: string | null;
  savedPath?: string | null;
  canRetryImport?: boolean;
  retryingImport?: boolean;
  onBack: () => void;
  onOpenPackaging: () => void;
  onPickDirectory: () => Promise<string | null>;
  onSave: (payload: ExpertCreatePayload) => Promise<void>;
  onRetryImport?: () => Promise<void>;
  onRenderPreview: (payload: ExpertPreviewPayload) => Promise<ExpertPreviewResult>;
}

const DEFAULT_DIR = "~/.skillmint/skills/";

function sanitizeSlug(input: string): string {
  const slug = input
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug || "expert-skill";
}

function joinPath(basePath: string, segment: string): string {
  if (!basePath) return segment;
  if (basePath.endsWith("/") || basePath.endsWith("\\")) return `${basePath}${segment}`;
  return `${basePath}/${segment}`;
}

function buildFallbackPreview(draft: DraftState): string {
  const name = draft.name.trim() || "expert-skill";
  const desc = draft.description.trim() || "Use when users need a reusable expert workflow.";
  const when = draft.whenToUse.trim() || "需要在特定任务场景中提供稳定执行能力";
  const normalizedDescription = desc.toLowerCase().startsWith("use when") ? desc : `Use when ${desc}`;

  return `---\nname: ${name}\ndescription: ${normalizedDescription}\n---\n\n# ${name}\n\n## Overview\n${
    draft.description.trim() || "该技能用于稳定处理特定任务流程。"
  }\n\n## When to Use\n- ${when}\n\n## Workflow\n1. 明确任务目标、输入资料和交付标准。\n2. 拆分执行步骤，先确认关键约束再动手。\n3. 使用可复现的方法完成任务并记录关键决策。\n4. 交付前进行结果校验，必要时给出备选方案。\n\n## Quality Checklist\n- 结果是否满足用户目标与约束\n- 输出是否可执行且包含关键上下文\n- 风险点与限制是否已明确说明\n`;
}

function initialMessages(): ChatMessage[] {
  return [
    {
      role: "assistant",
      content: "我会用对话方式帮你创建专家技能。先告诉我技能名称。",
    },
  ];
}

export function ExpertCreateView({
  saving,
  error,
  savedPath,
  canRetryImport = false,
  retryingImport = false,
  onBack,
  onOpenPackaging,
  onPickDirectory,
  onSave,
  onRetryImport,
  onRenderPreview,
}: Props) {
  const [step, setStep] = useState<Step>("name");
  const [messages, setMessages] = useState<ChatMessage[]>(initialMessages);
  const [input, setInput] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [draft, setDraft] = useState<DraftState>({
    name: "",
    description: "",
    whenToUse: "",
    targetDir: "",
  });
  const [previewError, setPreviewError] = useState<string | null>(null);

  const appendMessage = (message: ChatMessage) => {
    setMessages((prev) => [...prev, message]);
  };

  const askNext = (text: string, nextStep: Step) => {
    appendMessage({ role: "assistant", content: text });
    setStep(nextStep);
  };

  const enterConfirm = (nextDraft?: Partial<DraftState>) => {
    const merged = { ...draft, ...(nextDraft || {}) };
    setDraft(merged);
    appendMessage({
      role: "assistant",
      content:
        "信息已收集完成。你可以点击“保存技能”直接创建，也可以继续修改后再保存。",
    });
    setStep("confirm");
  };

  const resetFlow = () => {
    setMessages(initialMessages());
    setDraft({
      name: "",
      description: "",
      whenToUse: "",
      targetDir: "",
    });
    setInput("");
    setFormError(null);
    setStep("name");
  };

  const handleSubmitInput = () => {
    const value = input.trim();
    if (!value) return;
    appendMessage({ role: "user", content: value });
    setInput("");

    if (step === "name") {
      setDraft((prev) => ({ ...prev, name: value }));
      askNext("这个技能主要解决什么问题？请用一句话描述。", "description");
      return;
    }

    if (step === "description") {
      setDraft((prev) => ({ ...prev, description: value }));
      askNext("这个技能在什么场景下使用？请描述触发条件。", "when");
      return;
    }

    if (step === "when") {
      setDraft((prev) => ({ ...prev, whenToUse: value }));
      askNext("请输入保存目录，或点击下方按钮选择目录。你也可以使用默认目录。", "dir");
      return;
    }

    if (step === "dir") {
      enterConfirm({ targetDir: value });
    }
  };

  const handleChooseDirectory = async () => {
    if (step !== "dir") return;
    const selected = await onPickDirectory();
    if (selected) {
      appendMessage({ role: "user", content: `保存到：${selected}` });
      enterConfirm({ targetDir: selected });
    } else {
      appendMessage({ role: "user", content: "使用默认目录" });
      enterConfirm({ targetDir: "" });
    }
  };

  const handleUseDefaultDirectory = () => {
    if (step !== "dir") return;
    appendMessage({ role: "user", content: "使用默认目录" });
    enterConfirm({ targetDir: "" });
  };

  const handleSave = async () => {
    if (!draft.name.trim()) {
      setFormError("技能名称不能为空");
      return;
    }
    if (!draft.whenToUse.trim()) {
      setFormError("使用场景不能为空");
      return;
    }
    setFormError(null);
    await onSave({
      name: draft.name.trim(),
      description: draft.description.trim(),
      whenToUse: draft.whenToUse.trim(),
      targetDir: draft.targetDir.trim() || undefined,
    });
  };

  const fallbackPreview = useMemo(() => buildFallbackPreview(draft), [draft]);
  const fallbackSavePath = useMemo(() => {
    const basePath = draft.targetDir.trim() || DEFAULT_DIR;
    const slug = sanitizeSlug(draft.name || "expert-skill");
    return joinPath(basePath, slug);
  }, [draft]);
  const [preview, setPreview] = useState(fallbackPreview);
  const [previewPath, setPreviewPath] = useState(fallbackSavePath);

  useEffect(() => {
    let cancelled = false;
    async function loadPreview() {
      try {
        const result = await onRenderPreview({
          name: draft.name.trim(),
          description: draft.description.trim(),
          whenToUse: draft.whenToUse.trim(),
          targetDir: draft.targetDir.trim() || undefined,
        });
        if (cancelled) return;
        setPreview(result.markdown);
        setPreviewPath(result.savePath);
        setPreviewError(null);
      } catch (e) {
        console.error("技能预览生成失败:", e);
        if (cancelled) return;
        setPreview(fallbackPreview);
        setPreviewPath(fallbackSavePath);
        setPreviewError("预览生成失败，已显示本地草稿。");
      }
    }
    loadPreview();
    return () => {
      cancelled = true;
    };
  }, [draft, fallbackPreview, fallbackSavePath, onRenderPreview]);

  return (
    <div className="h-full overflow-hidden bg-gray-50">
      <div className="h-full grid grid-cols-1 xl:grid-cols-2 gap-0">
        <div className="h-full overflow-y-auto border-r border-gray-200 bg-white">
          <div className="p-6">
            <div className="flex items-center justify-between mb-4">
              <h1 className="text-xl font-semibold text-gray-900">创建专家技能</h1>
              <button
                onClick={onBack}
                className="text-sm text-gray-500 hover:text-gray-700 transition-colors"
              >
                返回我的技能
              </button>
            </div>

            <p className="text-sm text-gray-600 mb-5">
              采用智能体原生对话引导创建，逐步补齐技能信息并实时预览产物。
            </p>

            <div className="rounded-xl border border-gray-200 bg-gray-50 p-3 h-[360px] overflow-y-auto space-y-2">
              {messages.map((m, idx) => (
                <div key={idx} className={`flex ${m.role === "assistant" ? "justify-start" : "justify-end"}`}>
                  <div
                    className={
                      "max-w-[85%] rounded-xl px-3 py-2 text-sm whitespace-pre-wrap " +
                      (m.role === "assistant"
                        ? "bg-white border border-gray-200 text-gray-700"
                        : "bg-blue-500 text-white")
                    }
                  >
                    {m.content}
                  </div>
                </div>
              ))}
            </div>

            <div className="mt-3">
              {step !== "confirm" && (
                <div className="flex gap-2">
                  <input
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !e.shiftKey) {
                        e.preventDefault();
                        handleSubmitInput();
                      }
                    }}
                    placeholder="输入你的回答..."
                    className="flex-1 bg-gray-50 border border-gray-200 rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-400"
                  />
                  <button
                    onClick={handleSubmitInput}
                    className="h-9 px-4 rounded-lg bg-blue-500 hover:bg-blue-600 text-white text-sm transition-colors"
                  >
                    发送
                  </button>
                </div>
              )}

              {step === "dir" && (
                <div className="flex gap-2 mt-2">
                  <button
                    onClick={handleChooseDirectory}
                    className="h-9 px-3 rounded bg-gray-100 hover:bg-gray-200 text-sm text-gray-700 transition-colors"
                  >
                    选择目录
                  </button>
                  <button
                    onClick={handleUseDefaultDirectory}
                    className="h-9 px-3 rounded bg-blue-50 hover:bg-blue-100 text-sm text-blue-700 transition-colors"
                  >
                    使用默认目录
                  </button>
                </div>
              )}

              {step === "confirm" && (
                <div className="flex flex-wrap gap-2 mt-2">
                  <button
                    onClick={handleSave}
                    disabled={saving}
                    className="h-9 px-4 rounded-lg bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-sm transition-colors"
                  >
                    {saving ? "保存中..." : "保存技能"}
                  </button>
                  <button
                    onClick={resetFlow}
                    className="h-9 px-4 rounded-lg bg-gray-100 hover:bg-gray-200 text-sm text-gray-700 transition-colors"
                  >
                    重新引导
                  </button>
                  <button
                    onClick={onOpenPackaging}
                    className="h-9 px-4 rounded-lg bg-amber-50 hover:bg-amber-100 text-amber-700 text-sm transition-colors"
                  >
                    技能打包
                  </button>
                  {canRetryImport && onRetryImport && (
                    <button
                      onClick={onRetryImport}
                      disabled={retryingImport}
                      className="h-9 px-4 rounded-lg bg-blue-50 hover:bg-blue-100 disabled:bg-blue-100 text-blue-700 text-sm transition-colors"
                    >
                      {retryingImport ? "重试中..." : "重试导入"}
                    </button>
                  )}
                </div>
              )}
            </div>

            {savedPath && <div className="mt-2 text-xs text-gray-500">最近保存路径：{savedPath}</div>}
            {formError && <div className="mt-2 text-xs text-red-500">{formError}</div>}
            {error && <div className="mt-2 text-xs text-red-500">{error}</div>}
          </div>
        </div>

        <div className="h-full overflow-y-auto bg-gray-50">
          <div className="p-6">
            <h2 className="text-sm font-medium text-gray-700 mb-3">实时预览</h2>
            <div className="text-xs text-gray-500 mb-2">
              保存路径：{previewPath}
            </div>
            {previewError && <div className="text-xs text-amber-600 mb-2">{previewError}</div>}
            <pre className="bg-white border border-gray-200 rounded-xl p-4 text-xs text-gray-700 overflow-x-auto whitespace-pre-wrap">
              {preview}
            </pre>
          </div>
        </div>
      </div>
    </div>
  );
}
