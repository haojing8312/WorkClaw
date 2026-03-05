import { useMemo, useRef, useState } from "react";
import { motion } from "framer-motion";
import { SessionInfo } from "../types";

interface Props {
  sessions: SessionInfo[];
  creating: boolean;
  error?: string | null;
  onSelectSession: (id: string) => void;
  onCreateSessionWithInitialMessage: (message: string) => void;
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

export function NewSessionLanding({
  sessions,
  creating,
  error,
  onSelectSession,
  onCreateSessionWithInitialMessage,
}: Props) {
  const [input, setInput] = useState("");
  const [selectedScenarioId, setSelectedScenarioId] = useState<string | null>(null);
  const [showFilledHint, setShowFilledHint] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const inputContainerRef = useRef<HTMLDivElement>(null);
  const recentSessionGroups = useMemo(() => groupRecentSessions(sessions), [sessions]);

  const submit = () => {
    if (creating) return;
    onCreateSessionWithInitialMessage(input.trim());
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

  return (
    <div className="h-full overflow-y-auto bg-gradient-to-b from-blue-50/60 via-gray-50 to-gray-50">
      <div className="max-w-5xl mx-auto px-8 pt-12 pb-12">
        <motion.div
          className="text-center mb-10"
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.28, ease: "easeOut" }}
        >
          <h1 className="text-3xl md:text-4xl font-semibold tracking-tight text-gray-900 mb-3">
            把你的电脑任务，交给 AI 助手（打工虾）协作完成
          </h1>
          <p className="text-sm text-gray-600 max-w-2xl mx-auto">
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
              className="inline-flex items-center rounded-full px-3 py-1 text-xs border border-blue-100 bg-blue-50 text-blue-700"
            >
              {item}
            </span>
          ))}
        </motion.div>

        <motion.div
          ref={inputContainerRef}
          className="bg-white border border-gray-200 rounded-2xl p-4 md:p-5 shadow-[0_8px_24px_-20px_rgba(59,130,246,0.5)]"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut", delay: 0.08 }}
        >
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
            className="w-full resize-none bg-transparent text-sm md:text-[15px] text-gray-800 placeholder-gray-400 focus:outline-none"
          />
          {showFilledHint && (
            <div className="mt-2 text-xs text-blue-600">已填入场景示例，你可以继续修改后再开始任务</div>
          )}
          {error && <div className="mt-2 text-xs text-red-500">{error}</div>}
          <div className="mt-3 flex justify-end">
            <button
              onClick={submit}
              disabled={creating}
              className="h-9 px-4 rounded-lg bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-sm transition-colors shadow-sm"
            >
              {creating ? "正在创建..." : "开始任务"}
            </button>
          </div>
        </motion.div>

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
            <div className="rounded-xl border border-dashed border-gray-200 bg-white px-4 py-6 text-center text-sm text-gray-400">
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
                        className="text-left rounded-xl border border-gray-200 bg-white hover:border-blue-300 hover:bg-blue-50/30 transition-colors px-4 py-3"
                        aria-label={session.title || "未命名任务"}
                      >
                        <div className="text-sm text-gray-800 truncate">{session.title || "未命名任务"}</div>
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
                    "text-left rounded-xl border px-4 py-3 transition-colors bg-white " +
                    (selected
                      ? "border-blue-400 bg-blue-50/40 shadow-[0_8px_20px_-16px_rgba(59,130,246,0.6)]"
                      : "border-gray-200 hover:border-blue-300 hover:bg-blue-50/20")
                  }
                >
                  <div className="text-sm font-medium text-gray-800 mb-1">{card.title}</div>
                  <div className="text-xs text-gray-500">{card.description}</div>
                </button>
              );
            })}
          </div>
        </motion.div>
      </div>
    </div>
  );
}
