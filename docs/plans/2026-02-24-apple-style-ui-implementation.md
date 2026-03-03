# Apple 风格 UI 重设计 — 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 WorkClaw Runtime 前端从深色技术风格改造为浅色清新的 Apple 设计语言，包含动态岛工具调用展示和 framer-motion 动效。

**Architecture:** 纯前端改造，不涉及 Rust 后端。在现有 Tailwind CSS 基础上切换配色体系，引入 framer-motion 处理动画，新建 ToolIsland 组件替换 ToolCallCard。所有改动限制在 `apps/runtime/src/` 目录。

**Tech Stack:** React 18, Tailwind CSS 3, framer-motion, react-markdown, react-syntax-highlighter

---

### Task 1: 安装 framer-motion

**Files:**
- Modify: `apps/runtime/package.json`

**Step 1: 安装依赖**

Run: `cd apps/runtime && pnpm add framer-motion`

**Step 2: 验证安装**

Run: `cd apps/runtime && pnpm list framer-motion`
Expected: `framer-motion` 版本号显示

**Step 3: Commit**

```bash
git add apps/runtime/package.json apps/runtime/pnpm-lock.yaml
git commit -m "chore: 添加 framer-motion 依赖"
```

---

### Task 2: 全局配色切换 — index.css + App.tsx

**Files:**
- Modify: `apps/runtime/src/index.css`
- Modify: `apps/runtime/src/App.tsx`

**Step 1: 修改全局样式**

`apps/runtime/src/index.css` 完整替换为：

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

body {
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", system-ui, sans-serif;
  background: #f9fafb;
  color: #1f2937;
  margin: 0;
  height: 100vh;
  overflow: hidden;
}
```

**Step 2: 修改 App.tsx 根布局**

将根 div 的 class 从：
```
flex h-screen bg-slate-900 text-slate-100 overflow-hidden
```
改为：
```
flex h-screen bg-gray-50 text-gray-800 overflow-hidden
```

将空状态区域的文字颜色从 `text-slate-400` 改为 `text-gray-400`，按钮从 `bg-blue-600 hover:bg-blue-700` 改为 `bg-blue-500 hover:bg-blue-600`。

**Step 3: 验证编译**

Run: `cd apps/runtime && pnpm build`
Expected: 编译成功，无错误

**Step 4: Commit**

```bash
git add apps/runtime/src/index.css apps/runtime/src/App.tsx
git commit -m "style: 全局配色切换为浅色主题"
```

---

### Task 3: 侧边栏配色 + 动效 — Sidebar.tsx

**Files:**
- Modify: `apps/runtime/src/components/Sidebar.tsx`

**Step 1: 添加 framer-motion import**

在文件顶部添加：
```tsx
import { motion, AnimatePresence } from "framer-motion";
```

**Step 2: 折叠模式配色替换**

折叠侧边栏根 div 的 class 从：
```
w-12 bg-slate-800 flex flex-col h-full border-r border-slate-700 items-center py-3 gap-3 flex-shrink-0
```
改为：
```
w-12 bg-white flex flex-col h-full border-r border-gray-200 items-center py-3 gap-3 flex-shrink-0
```

折叠按钮的 class 从 `text-slate-400 hover:text-slate-200 hover:bg-slate-700` 改为 `text-gray-400 hover:text-gray-600 hover:bg-gray-100`。

安装按钮的 class 从 `text-blue-400 hover:text-blue-300 hover:bg-slate-700` 改为 `text-blue-500 hover:text-blue-400 hover:bg-blue-50`。

设置按钮同折叠按钮一样改色。

**Step 3: 展开模式配色替换**

展开侧边栏根 div 的 class 从：
```
w-56 bg-slate-800 flex flex-col h-full border-r border-slate-700 flex-shrink-0
```
改为：
```
w-56 bg-white flex flex-col h-full border-r border-gray-200 flex-shrink-0
```

标题栏的所有 `border-slate-700` 改为 `border-gray-200`，`text-slate-400` 改为 `text-gray-500`。

Skill 列表项：
- 选中态从 `bg-blue-600/30 text-blue-300` 改为 `bg-blue-50 text-blue-600`
- 未选中态从 `text-slate-300 hover:bg-slate-700` 改为 `text-gray-700 hover:bg-gray-50`
- Skill 名字内置标签从 `bg-blue-800/60 text-blue-300` 改为 `bg-blue-100 text-blue-600`
- 本地标签从 `bg-green-800/60 text-green-300` 改为 `bg-green-100 text-green-600`
- 版本文字从 `text-slate-500` 改为 `text-gray-400`

空状态文字从 `text-slate-500` 改为 `text-gray-400`。

会话历史区域：
- 所有 `border-slate-700` → `border-gray-200`
- 搜索框从 `bg-slate-700 border-slate-600 text-slate-200 placeholder-slate-500 focus:border-blue-500` 改为 `bg-gray-50 border-gray-200 text-gray-800 placeholder-gray-400 focus:border-blue-400 focus:ring-1 focus:ring-blue-400`
- 会话项选中从 `bg-blue-600/20 text-blue-300` 改为 `bg-blue-50 text-blue-600`
- 未选中从 `text-slate-300 hover:bg-slate-700` 改为 `text-gray-700 hover:bg-gray-50`
- 删除按钮保持 `text-red-400 hover:text-red-300`（红色在浅色中也有效）
- 导出按钮从 `text-slate-400 hover:text-slate-200` 改为 `text-gray-400 hover:text-gray-600`

底部按钮区：
- `border-slate-700` → `border-gray-200`
- 安装按钮从 `bg-blue-600 hover:bg-blue-700` 改为 `bg-blue-500 hover:bg-blue-600 text-white`
- 设置按钮从 `bg-slate-700 hover:bg-slate-600` 改为 `bg-gray-100 hover:bg-gray-200 text-gray-700`

**Step 4: 添加会话删除动画**

将会话列表项的 `div` 改为 `motion.div`，包裹在 `AnimatePresence` 中：

```tsx
<AnimatePresence>
  {sessions.map((s) => (
    <motion.div
      key={s.id}
      initial={{ opacity: 0, x: -10 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: -20, height: 0 }}
      transition={{ duration: 0.2 }}
      className={...同上...}
      onClick={() => onSelectSession(s.id)}
    >
      {/* 内容不变 */}
    </motion.div>
  ))}
</AnimatePresence>
```

**Step 5: 验证编译**

Run: `cd apps/runtime && pnpm build`
Expected: 编译成功

**Step 6: Commit**

```bash
git add apps/runtime/src/components/Sidebar.tsx
git commit -m "style: 侧边栏浅色主题 + 删除动画"
```

---

### Task 4: 新建 ToolIsland 组件

**Files:**
- Create: `apps/runtime/src/components/ToolIsland.tsx`

**Step 1: 创建 ToolIsland 组件**

完整代码：

```tsx
import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import ReactMarkdown from "react-markdown";
import { StreamItem, ToolCallInfo } from "../types";

/** 工具名 → 人性化描述 */
const TOOL_LABELS: Record<string, string> = {
  read_file: "正在读取文件",
  write_file: "正在写入文件",
  edit: "正在编辑文件",
  glob: "正在搜索文件",
  grep: "正在搜索内容",
  bash: "正在执行命令",
  web_search: "正在搜索网页",
  web_fetch: "正在获取网页",
  task: "子任务执行中",
  todo_write: "正在更新任务",
  memory: "正在访问记忆",
  ask_user: "等待用户回复",
  compact: "正在压缩上下文",
};

/** 提取工具调用的关键参数摘要 */
function getParamSummary(tc: ToolCallInfo): string {
  if (tc.name === "task") return String(tc.input.agent_type || "general");
  if (tc.name === "read_file" || tc.name === "write_file" || tc.name === "edit") {
    const p = String(tc.input.file_path || tc.input.path || "");
    return p.split(/[/\\]/).pop() || p;
  }
  if (tc.name === "glob") return String(tc.input.pattern || "");
  if (tc.name === "grep") return String(tc.input.pattern || "");
  if (tc.name === "bash") {
    const cmd = String(tc.input.command || "");
    return cmd.length > 30 ? cmd.slice(0, 30) + "..." : cmd;
  }
  if (tc.name === "web_search") return String(tc.input.query || "");
  return "";
}

interface ToolIslandProps {
  /** 当前批次的工具调用 items（仅 type==="tool_call"） */
  toolCalls: ToolCallInfo[];
  /** 是否正在执行中 */
  isRunning: boolean;
  /** 子 Agent 实时输出 */
  subAgentBuffer?: string;
}

export function ToolIsland({ toolCalls, isRunning, subAgentBuffer }: ToolIslandProps) {
  const [expanded, setExpanded] = useState(false);
  const [detailIndex, setDetailIndex] = useState<number | null>(null);

  const completed = toolCalls.filter((tc) => tc.status !== "running").length;
  const total = toolCalls.length;
  const current = toolCalls.find((tc) => tc.status === "running");
  const currentLabel = current
    ? TOOL_LABELS[current.name] || `正在执行 ${current.name}`
    : null;

  const allDone = !isRunning && total > 0;

  return (
    <motion.div
      layout
      className="my-2 mx-auto max-w-[360px]"
      transition={{ type: "spring", stiffness: 400, damping: 30 }}
    >
      {/* 胶囊主体 */}
      <motion.div
        layout
        className={
          "rounded-2xl overflow-hidden cursor-pointer select-none " +
          (expanded
            ? "bg-white/95 backdrop-blur-md shadow-lg border border-gray-200"
            : "bg-white/90 backdrop-blur-md shadow-md border border-gray-200")
        }
        onClick={() => setExpanded(!expanded)}
      >
        {/* 顶部摘要行 */}
        <motion.div layout="position" className="flex items-center gap-2.5 px-4 py-2.5">
          {/* 状态指示器 */}
          {isRunning ? (
            <span className="relative flex h-2.5 w-2.5">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-blue-400 opacity-75" />
              <span className="relative inline-flex rounded-full h-2.5 w-2.5 bg-blue-500" />
            </span>
          ) : (
            <svg className="h-3.5 w-3.5 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
            </svg>
          )}

          {/* 描述文字 */}
          <span className="flex-1 text-xs font-medium text-gray-700 truncate">
            {isRunning
              ? currentLabel || "执行中..."
              : `已执行 ${total} 个操作`}
          </span>

          {/* 进度计数 */}
          {isRunning && total > 1 && (
            <span className="text-[11px] text-gray-400 tabular-nums">
              {completed}/{total}
            </span>
          )}

          {/* 展开箭头 */}
          <motion.span
            animate={{ rotate: expanded ? 180 : 0 }}
            transition={{ duration: 0.2 }}
            className="text-gray-400 text-xs"
          >
            ▾
          </motion.span>
        </motion.div>

        {/* 进度条（仅运行中且未展开时显示） */}
        {isRunning && !expanded && total > 1 && (
          <div className="px-4 pb-2.5">
            <div className="h-1 bg-gray-100 rounded-full overflow-hidden">
              <motion.div
                className="h-full bg-blue-400 rounded-full"
                initial={{ width: 0 }}
                animate={{ width: `${(completed / total) * 100}%` }}
                transition={{ duration: 0.3 }}
              />
            </div>
          </div>
        )}

        {/* 展开的详情列表 */}
        <AnimatePresence>
          {expanded && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ type: "spring", stiffness: 500, damping: 35 }}
              className="overflow-hidden"
            >
              <div className="border-t border-gray-100 px-3 py-2 space-y-0.5">
                {toolCalls.map((tc, i) => (
                  <div key={tc.id}>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setDetailIndex(detailIndex === i ? null : i);
                      }}
                      className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-xs hover:bg-gray-50 transition-colors text-left"
                    >
                      {/* 状态图标 */}
                      {tc.status === "running" ? (
                        <span className="h-2 w-2 rounded-full bg-blue-400 animate-pulse flex-shrink-0" />
                      ) : tc.status === "completed" ? (
                        <svg className="h-3 w-3 text-green-500 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                          <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                        </svg>
                      ) : (
                        <svg className="h-3 w-3 text-red-400 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                          <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                      )}
                      {/* 工具名 */}
                      <span className="font-mono text-gray-600 w-20 truncate flex-shrink-0">
                        {tc.name === "task" ? "子任务" : tc.name}
                      </span>
                      {/* 参数摘要 */}
                      <span className="text-gray-400 truncate flex-1">
                        {getParamSummary(tc)}
                      </span>
                    </button>
                    {/* 二级展开：完整参数和输出 */}
                    <AnimatePresence>
                      {detailIndex === i && (
                        <motion.div
                          initial={{ height: 0, opacity: 0 }}
                          animate={{ height: "auto", opacity: 1 }}
                          exit={{ height: 0, opacity: 0 }}
                          transition={{ duration: 0.15 }}
                          className="overflow-hidden"
                        >
                          <div className="ml-7 mr-2 mb-2 space-y-1.5">
                            <pre className="bg-gray-50 rounded-xl p-2.5 text-[11px] text-gray-600 overflow-x-auto max-h-32 overflow-y-auto">
                              {tc.name === "task"
                                ? String(tc.input.prompt || "")
                                : JSON.stringify(tc.input, null, 2)}
                            </pre>
                            {/* 子 Agent 实时输出 */}
                            {tc.name === "task" && tc.status === "running" && subAgentBuffer && (
                              <div className="bg-gray-50 rounded-xl p-2.5 text-[11px] text-gray-600 max-h-32 overflow-y-auto prose prose-xs prose-gray">
                                <ReactMarkdown>{subAgentBuffer}</ReactMarkdown>
                                <span className="animate-pulse text-blue-400">|</span>
                              </div>
                            )}
                            {tc.output && (
                              <pre className="bg-gray-50 rounded-xl p-2.5 text-[11px] text-gray-500 overflow-x-auto max-h-32 overflow-y-auto">
                                {tc.name === "task" ? (
                                  <div className="prose prose-xs prose-gray">
                                    <ReactMarkdown>{tc.output}</ReactMarkdown>
                                  </div>
                                ) : (
                                  tc.output
                                )}
                              </pre>
                            )}
                          </div>
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </div>
                ))}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </motion.div>
  );
}
```

**Step 2: 验证编译**

Run: `cd apps/runtime && pnpm build`
Expected: 编译成功（组件尚未被引用，tree-shaking 不会报错）

**Step 3: Commit**

```bash
git add apps/runtime/src/components/ToolIsland.tsx
git commit -m "feat: 新建 ToolIsland 动态岛组件"
```

---

### Task 5: ChatView 配色 + 动态岛集成 + 输入区重设计

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`

这是最大的改动，分步骤说明。

**Step 1: 替换 import**

将：
```tsx
import { ToolCallCard } from "./ToolCallCard";
```
替换为：
```tsx
import { motion, AnimatePresence } from "framer-motion";
import { ToolIsland } from "./ToolIsland";
```

**Step 2: 修改 markdownComponents 中行内代码的配色**

将第 346 行 `bg-slate-600/50` 改为 `bg-gray-200/60 text-gray-800`：

```tsx
<code className={"bg-gray-200/60 px-1.5 py-0.5 rounded text-sm text-gray-800 " + (className || "")} {...props}>
```

**Step 3: 重写 renderStreamItems 函数**

将 `renderStreamItems` 函数替换为使用 ToolIsland 的版本。核心逻辑：将连续的 tool_call items 聚合为一个 ToolIsland，文字块单独渲染。

```tsx
function renderStreamItems(items: StreamItem[], isStreaming: boolean) {
  const groups: { type: "text" | "tools"; items: StreamItem[] }[] = [];
  for (const item of items) {
    if (item.type === "tool_call") {
      const last = groups[groups.length - 1];
      if (last && last.type === "tools") {
        last.items.push(item);
      } else {
        groups.push({ type: "tools", items: [item] });
      }
    } else {
      groups.push({ type: "text", items: [item] });
    }
  }

  return groups.map((g, i) => {
    if (g.type === "tools") {
      const toolCalls = g.items
        .filter((it) => it.toolCall)
        .map((it) => it.toolCall!);
      const hasRunning = toolCalls.some((tc) => tc.status === "running");
      return (
        <ToolIsland
          key={`island-${i}`}
          toolCalls={toolCalls}
          isRunning={hasRunning}
          subAgentBuffer={hasRunning ? subAgentBuffer : undefined}
        />
      );
    }
    const text = g.items.map((it) => it.content || "").join("");
    if (!text) return null;
    return (
      <div key={`txt-${i}`}>
        <ReactMarkdown components={markdownComponents}>{text}</ReactMarkdown>
      </div>
    );
  });
}
```

**Step 4: 头部栏简化 + 配色**

将头部 div（约 383-396 行）替换为：

```tsx
<div className="flex items-center justify-between px-6 py-3.5 border-b border-gray-200 bg-white/70 backdrop-blur-sm">
  <span className="font-semibold text-gray-900">{skill.name}</span>
  {currentModel && (
    <span className="text-xs text-gray-400">{currentModel.name}</span>
  )}
</div>
```

**Step 5: Agent 状态指示器配色**

将 agentState div（约 401 行）的 class 改为：

```tsx
<div className="sticky top-0 z-10 flex items-center gap-2 bg-white/80 backdrop-blur-lg px-4 py-2 rounded-xl text-xs text-gray-600 border border-gray-200 shadow-sm mx-4 mt-2">
```

spinner 保持不变，`text-slate-500` 改为 `text-gray-400`。去掉 `迭代 {agentState.iteration}` 的显示（隐藏技术细节）。

**Step 6: 消息列表配色 + 动画**

消息区域容器的 `space-y-4` 改为 `space-y-5`。

消息气泡外层 div 用 `motion.div` 替换（仅最新消息加动画）：

```tsx
{messages.map((m, i) => {
  const isLatest = i === messages.length - 1;
  return (
    <motion.div
      key={i}
      initial={isLatest ? { opacity: 0, x: m.role === "user" ? 20 : -20 } : false}
      animate={{ opacity: 1, x: 0 }}
      transition={{ type: "spring", stiffness: 300, damping: 24 }}
      className={"flex " + (m.role === "user" ? "justify-end" : "justify-start")}
    >
      <div
        className={
          "max-w-[80%] rounded-2xl px-5 py-3 text-sm " +
          (m.role === "user"
            ? "bg-blue-500 text-white"
            : "bg-white text-gray-800 shadow-sm border border-gray-100")
        }
      >
```

内部 Markdown 和工具渲染逻辑不变（但 `ToolCallCard` 引用需替换为旧格式兼容的 ToolIsland 渲染）。

旧格式 `m.toolCalls` 兼容部分，将 `ToolCallCard` 替换为 `ToolIsland`：

```tsx
) : m.role === "assistant" && m.toolCalls ? (
  <>
    <ToolIsland toolCalls={m.toolCalls} isRunning={false} />
    <ReactMarkdown components={markdownComponents}>{m.content}</ReactMarkdown>
  </>
```

**Step 7: 流式输出区域配色**

将流式输出区域的 div（约 444-449 行）改为：

```tsx
{streamItems.length > 0 && (
  <motion.div
    initial={{ opacity: 0, x: -20 }}
    animate={{ opacity: 1, x: 0 }}
    className="flex justify-start"
  >
    <div className="max-w-[80%] bg-white rounded-2xl px-5 py-3 text-sm text-gray-800 shadow-sm border border-gray-100">
      {renderStreamItems(streamItems, true)}
      <span className="animate-pulse text-blue-400">|</span>
    </div>
  </motion.div>
)}
```

**Step 8: AskUser 问答卡片配色**

将 `bg-amber-900/40 border-amber-600/50` 改为 `bg-amber-50 border border-amber-200`。
`text-amber-200` 改为 `text-amber-700`。
选项按钮从 `bg-amber-700/50 hover:bg-amber-600/50 text-amber-100` 改为 `bg-amber-100 hover:bg-amber-200 text-amber-700`。
输入框从 `bg-slate-700 border-slate-600` 改为 `bg-white border-gray-200`。
回答按钮从 `bg-amber-600 hover:bg-amber-700 disabled:bg-slate-600` 改为 `bg-amber-500 hover:bg-amber-600 disabled:bg-gray-200 disabled:text-gray-400`。

**Step 9: 工具确认卡片配色**

将 `bg-orange-900/40 border-orange-600/50` 改为 `bg-orange-50 border border-orange-200`。
`text-orange-200` → `text-orange-700`。
`text-slate-300` → `text-gray-600`。
`text-orange-100` → `text-orange-600`。
`bg-slate-800/60` → `bg-gray-50`。
`text-slate-300` 在 pre 块里改为 `text-gray-600`。

**Step 10: 输入区重设计**

将整个输入区（约 525-557 行）替换为：

```tsx
<div className="px-6 py-4 bg-gray-50">
  <div className="relative max-w-3xl mx-auto">
    <textarea
      value={input}
      onChange={(e) => setInput(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          handleSend();
        }
      }}
      placeholder="输入消息..."
      rows={1}
      className="w-full bg-white border border-gray-200 rounded-xl pl-4 pr-12 py-3 text-sm resize-none focus:outline-none focus:border-blue-400 focus:ring-1 focus:ring-blue-400 shadow-sm placeholder-gray-400"
    />
    <div className="absolute right-2 top-1/2 -translate-y-1/2">
      {streaming ? (
        <button
          onClick={handleCancel}
          className="w-8 h-8 flex items-center justify-center rounded-lg bg-red-500 hover:bg-red-600 text-white transition-colors"
        >
          <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
            <rect x="6" y="6" width="12" height="12" rx="2" />
          </svg>
        </button>
      ) : (
        <button
          onClick={handleSend}
          disabled={!input.trim()}
          className="w-8 h-8 flex items-center justify-center rounded-lg bg-blue-500 hover:bg-blue-600 disabled:bg-gray-200 disabled:text-gray-400 text-white transition-colors"
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M5 12h14M12 5l7 7-7 7" />
          </svg>
        </button>
      )}
    </div>
  </div>
</div>
```

**Step 11: 验证编译**

Run: `cd apps/runtime && pnpm build`
Expected: 编译成功

**Step 12: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx
git commit -m "feat: ChatView 浅色主题 + 动态岛 + 消息动画 + 输入区重设计"
```

---

### Task 6: SettingsView + InstallDialog 配色

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/components/InstallDialog.tsx`

**Step 1: SettingsView 配色批量替换**

在 SettingsView.tsx 中进行以下全局替换：
- `bg-slate-900` → `bg-gray-50`
- `bg-slate-800` → `bg-white`
- `bg-slate-700` → `bg-gray-100`（用于按钮）或 `bg-gray-50`（用于输入框）
- `border-slate-700` → `border-gray-200`
- `border-slate-600` → `border-gray-200`
- `text-slate-100` → `text-gray-800`
- `text-slate-200` → `text-gray-700`
- `text-slate-300` → `text-gray-600`
- `text-slate-400` → `text-gray-500`
- `text-slate-500` → `text-gray-400`
- `hover:bg-slate-700` → `hover:bg-gray-100`
- `hover:bg-slate-600` → `hover:bg-gray-200`
- `bg-blue-600` → `bg-blue-500`
- `hover:bg-blue-700` → `hover:bg-blue-600`
- `disabled:bg-slate-600` → `disabled:bg-gray-200 disabled:text-gray-400`
- `focus:border-blue-500` → `focus:border-blue-400 focus:ring-1 focus:ring-blue-400`
- `bg-red-600/20 text-red-400` → `bg-red-50 text-red-600`
- `bg-green-600/20 text-green-400` → `bg-green-50 text-green-600`

**Step 2: InstallDialog 配色替换**

- 背景遮罩 `bg-black/60` 改为 `bg-black/30 backdrop-blur-sm`
- 对话框 `bg-slate-800 border-slate-600` 改为 `bg-white border-gray-200 shadow-xl`
- 标题 `font-semibold text-lg` 加 `text-gray-900`
- Tab 激活态 `bg-blue-600 text-white` → `bg-blue-500 text-white`
- Tab 未激活态 `bg-slate-700 text-slate-400 hover:bg-slate-600` → `bg-gray-100 text-gray-500 hover:bg-gray-200`
- 虚线按钮 `border-slate-500 text-slate-400 hover:border-blue-500 hover:text-blue-400` → `border-gray-300 text-gray-500 hover:border-blue-400 hover:text-blue-500`
- 输入框 `bg-slate-700 border-slate-600` → `bg-gray-50 border-gray-200`
- 取消按钮 `bg-slate-700 hover:bg-slate-600` → `bg-gray-100 hover:bg-gray-200 text-gray-700`
- 安装按钮 `bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600` → `bg-blue-500 hover:bg-blue-600 disabled:bg-gray-200 disabled:text-gray-400`
- 错误文字保持 `text-red-400` → `text-red-500`
- MCP 警告 `text-amber-400` → `text-amber-600`
- label `text-slate-400` → `text-gray-500`
- 描述文字 `text-slate-500` → `text-gray-400`

**Step 3: 验证编译**

Run: `cd apps/runtime && pnpm build`
Expected: 编译成功

**Step 4: Commit**

```bash
git add apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/InstallDialog.tsx
git commit -m "style: SettingsView + InstallDialog 浅色主题"
```

---

### Task 7: 清理旧 ToolCallCard + 最终验证

**Files:**
- Delete: `apps/runtime/src/components/ToolCallCard.tsx`

**Step 1: 确认 ToolCallCard 不再被引用**

搜索 `ToolCallCard` 在所有 tsx 文件中的引用，确认已全部替换为 ToolIsland。

Run: `cd apps/runtime && grep -r "ToolCallCard" src/`
Expected: 无结果

**Step 2: 删除旧文件**

```bash
rm apps/runtime/src/components/ToolCallCard.tsx
```

**Step 3: 最终编译验证**

Run: `cd apps/runtime && pnpm build`
Expected: 编译成功，零错误

**Step 4: 视觉验证**

Run: `cd apps/runtime && pnpm dev`

手动检查：
- [ ] 全局背景为浅灰色 (`#f9fafb`)
- [ ] 侧边栏白底，边框清晰
- [ ] 消息气泡：用户蓝色、助手白底带阴影
- [ ] 输入框居中，发送按钮内嵌在右侧
- [ ] 工具调用显示为动态岛胶囊，默认折叠
- [ ] 展开动态岛可见工具列表，二级展开可见详情
- [ ] 消息出现有滑入动效
- [ ] 设置页面为浅色主题
- [ ] 安装对话框为浅色毛玻璃背景

**Step 5: Commit + Push**

```bash
git add -A
git commit -m "feat(ui): Apple 风格浅色主题全面落地 — 动态岛 + framer-motion 动效"
git push
```
