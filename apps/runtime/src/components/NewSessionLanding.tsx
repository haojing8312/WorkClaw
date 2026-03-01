import { useRef, useState } from "react";
import { SessionInfo } from "../types";

interface Props {
  sessions: SessionInfo[];
  creating: boolean;
  error?: string | null;
  onSelectSession: (id: string) => void;
  onCreateSessionWithInitialMessage: (message: string) => void;
  onOpenExperts?: () => void;
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

export function NewSessionLanding({
  sessions,
  creating,
  error,
  onSelectSession,
  onCreateSessionWithInitialMessage,
  onOpenExperts,
}: Props) {
  const [input, setInput] = useState("");
  const [selectedScenarioId, setSelectedScenarioId] = useState<string | null>(null);
  const [showFilledHint, setShowFilledHint] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const inputContainerRef = useRef<HTMLDivElement>(null);

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

  const recentSessions = sessions.slice(0, 6);

  return (
    <div className="h-full overflow-y-auto bg-gray-50">
      <div className="max-w-4xl mx-auto px-8 pt-14 pb-12">
        <div className="text-center mb-10">
          {onOpenExperts && (
            <div className="mb-3">
              <button
                onClick={onOpenExperts}
                className="inline-flex items-center h-8 px-3 rounded-full bg-blue-50 hover:bg-blue-100 text-blue-700 text-xs transition-colors"
              >
                专家技能
              </button>
            </div>
          )}
          <h1 className="text-3xl font-semibold text-gray-900 mb-3">把你的电脑任务，交给 AI 助手协作完成</h1>
          <p className="text-sm text-gray-600">
            一句话描述需求，它可以帮你创建和修改文件、分析本地数据、整理文件、操作浏览器，并持续反馈执行过程。
          </p>
        </div>

        <div className="flex flex-wrap gap-2 justify-center mb-8">
          {["创建/修改文件", "分析本地文件", "文件整理", "浏览器操作"].map((item) => (
            <span
              key={item}
              className="inline-flex items-center rounded-full px-3 py-1 text-xs border border-blue-100 bg-blue-50 text-blue-700"
            >
              {item}
            </span>
          ))}
        </div>

        <div ref={inputContainerRef} className="bg-white border border-gray-200 rounded-2xl p-4 shadow-sm">
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
            className="w-full resize-none bg-transparent text-sm text-gray-800 placeholder-gray-400 focus:outline-none"
          />
          {showFilledHint && (
            <div className="mt-2 text-xs text-blue-600">已填入场景示例，你可以继续修改后再开始会话</div>
          )}
          {error && <div className="mt-2 text-xs text-red-500">{error}</div>}
          <div className="mt-3 flex justify-end">
            <button
              onClick={submit}
              disabled={creating}
              className="h-9 px-4 rounded-lg bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-sm transition-colors"
            >
              {creating ? "正在创建..." : "开始新会话"}
            </button>
          </div>
        </div>

        <div className="mt-10">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-sm font-medium text-gray-700">最近会话</h2>
          </div>
          {recentSessions.length === 0 ? (
            <div className="rounded-xl border border-dashed border-gray-200 bg-white px-4 py-6 text-center text-sm text-gray-400">
              暂无会话，从上方输入任务开始
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              {recentSessions.map((session) => (
                <button
                  key={session.id}
                  onClick={() => onSelectSession(session.id)}
                  className="text-left rounded-xl border border-gray-200 bg-white hover:border-blue-300 hover:bg-blue-50/30 transition-colors px-4 py-3"
                  aria-label={session.title || "New Chat"}
                >
                  <div className="text-sm text-gray-800 truncate">{session.title || "New Chat"}</div>
                </button>
              ))}
            </div>
          )}
        </div>

        <div className="mt-10">
          <div className="mb-3">
            <h2 className="text-sm font-medium text-gray-700">常见任务场景</h2>
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
                      ? "border-blue-400 bg-blue-50/40"
                      : "border-gray-200 hover:border-blue-300 hover:bg-blue-50/20")
                  }
                >
                  <div className="text-sm font-medium text-gray-800 mb-1">{card.title}</div>
                  <div className="text-xs text-gray-500">{card.description}</div>
                </button>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
