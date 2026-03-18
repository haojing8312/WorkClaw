import { ChangeEvent, useMemo, useRef, useState } from "react";
import { motion } from "framer-motion";
import { LandingSessionLaunchInput, PendingAttachment, SessionInfo } from "../types";

interface Props {
  sessions: SessionInfo[];
  teams?: Array<{
    id: string;
    name: string;
    description?: string;
    memberCount?: number;
  }>;
  creating: boolean;
  error?: string | null;
  onSelectSession: (id: string) => void;
  onCreateSessionWithInitialMessage: (input: LandingSessionLaunchInput) => void;
  onCreateTeamEntrySession?: (input: { teamId: string; initialMessage: string }) => void;
  onPickWorkDir?: (currentWorkDir?: string) => Promise<string | null>;
  defaultWorkDir?: string;
}

const MAX_FILES = 5;
const MAX_IMAGE_FILES = 3;
const MAX_IMAGE_SIZE = 5 * 1024 * 1024;
const MAX_TEXT_FILE_SIZE = 1 * 1024 * 1024;
const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "webp"]);
const TEXT_FILE_EXTENSIONS = new Set(["txt", "md", "json", "yaml", "yml", "xml", "csv", "tsv", "log"]);
const LANDING_FILE_INPUT_ID = "landing-file-upload";

function getFileExtension(name: string): string {
  const dotIndex = name.lastIndexOf(".");
  return dotIndex >= 0 ? name.slice(dotIndex + 1).toLowerCase() : "";
}

function isImageFile(file: File): boolean {
  return file.type.startsWith("image/") || IMAGE_EXTENSIONS.has(getFileExtension(file.name));
}

function isTextFile(file: File): boolean {
  return TEXT_FILE_EXTENSIONS.has(getFileExtension(file.name));
}

function createAttachmentId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `landing-attachment-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : "");
    reader.onerror = () => reject(reader.error ?? new Error("文件读取失败"));
    reader.readAsDataURL(file);
  });
}

function readTextFile(file: File): Promise<string> {
  if (typeof file.text === "function") {
    return file.text();
  }

  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : "");
    reader.onerror = () => reject(reader.error ?? new Error("文件读取失败"));
    reader.readAsText(file);
  });
}

const SCENARIO_CARDS = [
  {
    id: "file-organize",
    title: "文件整理助手",
    description: "批量整理下载目录，按类型与时间归档。",
    promptTemplate:
      "请帮我整理下载目录，把文件按类型分类到子文件夹，并按近30天和更早文件分开。先告诉我你的整理方案。",
  },
  {
    id: "local-data-summary",
    title: "本地数据汇总",
    description: "从本地文件提取关键数据并形成结论。",
    promptTemplate:
      "我有一批本地文件，请帮我提取关键数据并汇总成简明结论，先说明你会如何处理这些文件。",
  },
  {
    id: "browser-collection",
    title: "浏览器信息采集",
    description: "检索并整理公开网页信息，附来源。",
    promptTemplate:
      "请帮我在浏览器中查找这个主题的最新公开信息，并整理成要点列表，标注来源。",
  },
  {
    id: "code-debug",
    title: "代码问题排查",
    description: "结合报错与代码片段定位根因并给出修复。",
    promptTemplate:
      "我会提供报错和代码片段，请先定位最可能根因，再给出最小可行修复方案。",
  },
] as const;

type SessionGroup = {
  key: "today" | "week" | "older";
  label: string;
  items: SessionInfo[];
};

function groupRecentSessions(sessions: SessionInfo[]): SessionGroup[] {
  const now = new Date();
  const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const weekStart = todayStart - 6 * 24 * 60 * 60 * 1000;

  const groups: SessionGroup[] = [
    { key: "today", label: "今天", items: [] },
    { key: "week", label: "最近7天", items: [] },
    { key: "older", label: "更早", items: [] },
  ];

  for (const session of sessions.slice(0, 6)) {
    const ts = new Date(session.created_at).getTime();
    if (Number.isNaN(ts)) {
      groups[2].items.push(session);
      continue;
    }
    if (ts >= todayStart) {
      groups[0].items.push(session);
      continue;
    }
    if (ts >= weekStart) {
      groups[1].items.push(session);
      continue;
    }
    groups[2].items.push(session);
  }

  return groups.filter((group) => group.items.length > 0);
}

function getSessionDisplayTitle(session: SessionInfo): string {
  return (session.display_title || session.title || "").trim() || "未命名任务";
}

export function NewSessionLanding({
  sessions,
  teams = [],
  creating,
  error,
  onSelectSession,
  onCreateSessionWithInitialMessage,
  onCreateTeamEntrySession,
  onPickWorkDir,
  defaultWorkDir = "",
}: Props) {
  const [input, setInput] = useState("");
  const [attachedFiles, setAttachedFiles] = useState<PendingAttachment[]>([]);
  const [selectedWorkDir, setSelectedWorkDir] = useState(defaultWorkDir.trim());
  const [selectedScenarioId, setSelectedScenarioId] = useState<string | null>(null);
  const [showFilledHint, setShowFilledHint] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const inputContainerRef = useRef<HTMLDivElement>(null);
  const recentSessionGroups = useMemo(() => groupRecentSessions(sessions), [sessions]);

  const submit = () => {
    if (creating) return;
    onCreateSessionWithInitialMessage({
      initialMessage: input.trim(),
      attachments: attachedFiles,
      workDir: selectedWorkDir.trim(),
    });
  };

  const selectScenario = (scenarioId: string, template: string) => {
    setSelectedScenarioId(scenarioId);
    setInput(template);
    setShowFilledHint(true);
    if (inputContainerRef.current && typeof inputContainerRef.current.scrollIntoView === "function") {
      inputContainerRef.current.scrollIntoView({ behavior: "smooth", block: "center" });
    }
    inputRef.current?.focus();
  };

  const submitTeamEntry = (teamId: string) => {
    if (creating || !onCreateTeamEntrySession) return;
    onCreateTeamEntrySession({
      teamId,
      initialMessage: input.trim(),
    });
  };

  const handleFileSelect = async (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files || []);
    const currentImageCount = attachedFiles.filter((file) => file.kind === "image").length;

    if (attachedFiles.length + files.length > MAX_FILES) {
      alert(`最多只能上传 ${MAX_FILES} 个文件`);
      event.target.value = "";
      return;
    }

    const newFiles: PendingAttachment[] = [];
    let nextImageCount = currentImageCount;

    for (const file of files) {
      if (isImageFile(file)) {
        if (nextImageCount >= MAX_IMAGE_FILES) {
          alert(`最多只能上传 ${MAX_IMAGE_FILES} 张图片`);
          continue;
        }
        if (file.size > MAX_IMAGE_SIZE) {
          alert(`图片 ${file.name} 超过 5MB 限制`);
          continue;
        }
        const dataUrl = await readFileAsDataUrl(file);
        newFiles.push({
          id: createAttachmentId(),
          kind: "image",
          name: file.name,
          mimeType: file.type || "image/png",
          size: file.size,
          data: dataUrl,
          previewUrl: dataUrl,
        });
        nextImageCount += 1;
        continue;
      }

      if (!isTextFile(file)) {
        alert(`暂不支持附件类型 ${file.name}`);
        continue;
      }

      const text = await readTextFile(file);
      const truncated = text.length > MAX_TEXT_FILE_SIZE;
      newFiles.push({
        id: createAttachmentId(),
        kind: "text-file",
        name: file.name,
        mimeType: file.type || "text/plain",
        size: file.size,
        text: truncated ? text.slice(0, MAX_TEXT_FILE_SIZE) : text,
        truncated,
      });
    }

    if (newFiles.length > 0) {
      setAttachedFiles((prev) => [...prev, ...newFiles]);
    }

    event.target.value = "";
  };

  const handlePickWorkDir = async () => {
    if (!onPickWorkDir || creating) return;
    const nextDir = await onPickWorkDir(selectedWorkDir);
    if (typeof nextDir === "string" && nextDir.trim()) {
      setSelectedWorkDir(nextDir.trim());
    }
  };

  const removeAttachedFile = (attachmentId: string) => {
    setAttachedFiles((prev) => prev.filter((file) => file.id !== attachmentId));
  };

  const displayWorkDir = selectedWorkDir.trim() || defaultWorkDir.trim();
  const displayWorkDirLabel = displayWorkDir || "选择工作目录";

  return (
    <div className="h-full overflow-y-auto bg-[var(--sm-bg)]">
      <div className="max-w-5xl mx-auto px-8 pt-12 pb-12">
        <motion.div
          className="text-center mb-10"
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.28, ease: "easeOut" }}
        >
          <h1 className="mb-3 text-3xl font-semibold tracking-tight text-[var(--sm-text)] md:text-4xl">
            你的电脑任务，交给打工虾们协作完成
          </h1>
          <p className="mx-auto max-w-2xl text-sm text-[var(--sm-text-muted)]">
            一句话描述需求，它可以帮你创建和修改文件、分析本地数据、整理文件、操作浏览器，并持续反馈执行过程。
          </p>
        </motion.div>

        <motion.div
          className="flex flex-wrap gap-2 justify-center mb-8"
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.28, ease: "easeOut", delay: 0.04 }}
        >
          {["创建/修改文件", "分析本地文件", "文件整理", "浏览器操作"].map((item) => (
            <span
              key={item}
              className="inline-flex items-center rounded-full border border-[var(--sm-primary-soft)] bg-[var(--sm-primary-soft)] px-3 py-1 text-xs text-[var(--sm-primary-strong)]"
            >
              {item}
            </span>
          ))}
        </motion.div>

        <motion.div
          ref={inputContainerRef}
          className="rounded-[26px] border border-[var(--sm-border)] bg-[var(--sm-surface)] p-4 shadow-[0_8px_24px_-20px_rgba(59,130,246,0.22)] md:p-5"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut", delay: 0.08 }}
        >
          <input
            id={LANDING_FILE_INPUT_ID}
            aria-label="添加附件"
            type="file"
            multiple
            className="hidden"
            onChange={handleFileSelect}
          />
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => {
              setInput(e.target.value);
              if (showFilledHint) {
                setShowFilledHint(false);
              }
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                submit();
              }
            }}
            placeholder="先描述你要完成什么任务..."
            rows={5}
            className="w-full resize-none bg-transparent text-sm text-[var(--sm-text)] placeholder-[var(--sm-text-muted)] focus:outline-none md:text-[15px]"
          />
          {showFilledHint && (
            <div className="mt-2 text-xs text-[var(--sm-primary-strong)]">已填入场景示例，你可以继续修改后再开始任务</div>
          )}
          {error && <div className="mt-2 text-xs text-red-500">{error}</div>}
          {attachedFiles.length > 0 && (
            <div className="mt-3 space-y-2">
              <div className="text-xs text-gray-500">已添加 {attachedFiles.length} 个附件</div>
              <div className="flex flex-wrap gap-2">
                {attachedFiles.map((file) => (
                  <span
                    key={file.id}
                    className="inline-flex items-center gap-2 rounded-full border border-[var(--sm-primary-soft)] bg-[var(--sm-primary-soft)] px-3 py-1 text-xs text-[var(--sm-primary-strong)]"
                    title={file.name}
                  >
                    <span className="max-w-[220px] truncate">{file.name}</span>
                    <button
                      type="button"
                      aria-label={`移除附件 ${file.name}`}
                      onClick={() => removeAttachedFile(file.id)}
                      className="text-[var(--sm-primary)] hover:text-[var(--sm-primary-strong)]"
                    >
                      ×
                    </button>
                  </span>
                ))}
              </div>
            </div>
          )}
          <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
            <div className="flex flex-wrap items-center gap-2">
              <label
                htmlFor={LANDING_FILE_INPUT_ID}
                data-testid="landing-attachment-trigger"
                className="sm-btn sm-btn-secondary inline-flex cursor-pointer items-center gap-1.5 rounded-xl px-3 py-2 text-xs"
              >
                <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M15.172 7 8.586 13.586a2 2 0 1 0 2.828 2.828l6.414-6.586a4 4 0 0 0-5.656-5.656L5.757 10.757a6 6 0 1 0 8.486 8.486L20.5 13" />
                </svg>
                附件
              </label>
              <button
                type="button"
                onClick={() => void handlePickWorkDir()}
                className="inline-flex items-center gap-1.5 rounded-lg bg-[var(--sm-surface-muted)] px-2.5 py-1 text-xs text-[var(--sm-text-muted)] transition-colors hover:bg-[var(--sm-surface-soft)]"
                title={displayWorkDirLabel}
              >
                <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2Z" />
                </svg>
                <span data-testid="landing-workdir-label" className="max-w-[150px] truncate">
                  {displayWorkDirLabel}
                </span>
              </button>
            </div>
            <button
              onClick={submit}
              disabled={creating}
              className="sm-btn sm-btn-primary h-9 rounded-lg px-4 text-sm shadow-[var(--sm-shadow-sm)] disabled:opacity-60"
            >
              {creating ? "正在创建..." : "开始任务"}
            </button>
          </div>
        </motion.div>

        {teams.length > 0 && (
          <motion.div
            className="mt-8"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.28, ease: "easeOut", delay: 0.1 }}
          >
            <div className="mb-3">
              <h2 className="text-sm font-medium text-gray-700">团队协作入口</h2>
              <p className="text-xs text-gray-500 mt-1">
                适合需要拆解、审议、分工执行的复杂任务。只有显式点击后才会进入团队协作。
              </p>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
              {teams.map((team) => (
                <div
                  key={team.id}
                  className="rounded-2xl border border-[var(--sm-border)] bg-[var(--sm-surface)] px-4 py-4 shadow-[var(--sm-shadow-sm)]"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <div className="text-sm font-medium text-gray-900">{team.name}</div>
                      {team.description ? (
                        <div className="mt-1 text-xs leading-5 text-gray-500">{team.description}</div>
                      ) : null}
                    </div>
                    {team.memberCount ? (
                      <span className="inline-flex shrink-0 items-center rounded-full border border-indigo-100 bg-indigo-50 px-2.5 py-1 text-[11px] font-medium text-indigo-700">
                        {team.memberCount} 人团队
                      </span>
                    ) : null}
                  </div>
                  <button
                    type="button"
                    aria-label={`交给团队处理：${team.name}`}
                    onClick={() => submitTeamEntry(team.id)}
                    disabled={creating}
                    className="mt-4 inline-flex min-h-11 w-full items-center justify-center rounded-xl border border-indigo-200 bg-indigo-600 px-4 text-sm font-medium text-white transition-colors hover:bg-indigo-700 disabled:cursor-not-allowed disabled:bg-indigo-300"
                  >
                    交给团队处理
                  </button>
                </div>
              ))}
            </div>
          </motion.div>
        )}

        <motion.div
          className="mt-10"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.28, ease: "easeOut", delay: 0.12 }}
        >
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-sm font-medium text-gray-700">最近会话</h2>
          </div>
          {recentSessionGroups.length === 0 ? (
            <div className="rounded-xl border border-dashed border-[var(--sm-border)] bg-[var(--sm-surface)] px-4 py-6 text-center text-sm text-[var(--sm-text-muted)]">
              暂无会话，从上方输入任务开始
            </div>
          ) : (
            <div className="space-y-4">
              {recentSessionGroups.map((group) => (
                <div key={group.key}>
                  <div className="text-xs text-gray-500 mb-2">{group.label}</div>
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                    {group.items.map((session) => (
                      <button
                        key={session.id}
                        onClick={() => onSelectSession(session.id)}
                        className="rounded-xl border border-[var(--sm-border)] bg-[var(--sm-surface)] px-4 py-3 text-left transition-colors hover:border-[var(--sm-primary-soft)] hover:bg-[var(--sm-surface-muted)]"
                        aria-label={getSessionDisplayTitle(session)}
                      >
                        <div className="text-sm text-gray-800 truncate">{getSessionDisplayTitle(session)}</div>
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </motion.div>

        <motion.div
          className="mt-10"
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut", delay: 0.16 }}
        >
          <div className="mb-3">
            <h2 className="text-sm font-medium text-gray-700">精选任务场景</h2>
            <p className="text-xs text-gray-500 mt-1">选择一个场景，自动填入示例任务后再发起会话</p>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3">
            {SCENARIO_CARDS.map((card) => {
              const selected = selectedScenarioId === card.id;
              return (
                <button
                  key={card.id}
                  type="button"
                  aria-pressed={selected}
                  onClick={() => selectScenario(card.id, card.promptTemplate)}
                  className={
                    "rounded-xl border bg-[var(--sm-surface)] px-4 py-3 text-left transition-colors " +
                    (selected
                      ? "border-[var(--sm-primary-soft)] bg-[var(--sm-primary-soft)] shadow-[var(--sm-shadow-sm)]"
                      : "border-[var(--sm-border)] hover:border-[var(--sm-primary-soft)] hover:bg-[var(--sm-surface-muted)]")
                  }
                >
                  <div className="mb-1 text-sm font-medium text-[var(--sm-text)]">{card.title}</div>
                  <div className="text-xs text-[var(--sm-text-muted)]">{card.description}</div>
                </button>
              );
            })}
          </div>
        </motion.div>
      </div>
    </div>
  );
}
