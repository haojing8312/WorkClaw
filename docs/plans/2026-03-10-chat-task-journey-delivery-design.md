# Chat Task Journey And Delivery Design

## Goal

在不引入外部服务、也不新增模型能力的前提下，重构 WorkClaw 聊天主链路，让用户在主对话区直接看到：

- 当前正在做什么
- 已经完成了哪些关键步骤
- 哪里失败了、是否发生重试
- 本轮最终交付了哪些产物

## Problem

当前实现已经有 `streamItems`、`ToolIsland`、`TaskPanel`、`WebSearchPanel` 等能力，但它们主要承担“附属检视”作用，主聊天区仍以原始文本和工具块为主，导致：

- 任务过程不够连续，用户需要自己拼接上下文
- `TodoWrite` 未使用时，任务面板价值明显下降
- 连续失败调用会直接暴露为重复工具块
- 最终交付缺少统一收口，用户只能从正文里找文件名

## Non-Goals

- 不新增外部依赖服务
- 不新增独立结果页路由
- 不做录屏/动画 HTML 特化能力
- 不做环境探测与 fallback 机制扩展

## Design Summary

### 1. 主区新增任务旅程视图

在现有聊天消息渲染之上增加一层会话视图模型，从消息和工具轨迹中推导：

- 当前阶段：分析中 / 搜索中 / 生成中 / 收尾中 / 已完成 / 部分完成 / 失败
- 关键步骤：搜索、任务规划、文件写入、命令执行、错误、重试、取消
- 当前任务标题：优先来自 `todo_write`，没有时由最近工具轨迹推导

主聊天区新增：

- `TaskJourneyTimeline`
- `DeliverySummaryCard`

### 2. ToolIsland 降级为细节容器

`ToolIsland` 继续保留，但不再承担主叙事。主区默认展示“用户可理解步骤卡”，点击步骤可展开查看对应工具组细节。

### 3. 右侧面板改为辅助信息层

右侧面板继续保留：

- 当前任务
- 文件
- Web 搜索

但主流程信息必须在主区就能读懂。侧栏主要用于查看完整结果和原始细节。

### 4. 交付收口卡

在任务完成后的最后一条 assistant 消息内追加“交付卡”，统一展示：

- 结果状态：已完成 / 部分完成 / 失败
- 产物列表：主产物、辅助产物
- 风险提醒：失败重试、取消、缺失产物
- 后续动作：打开文件、打开目录、继续补做

## View Model

新增 `TaskJourneyViewModel`，由消息轨迹推导以下字段：

- `status`
- `currentPhase`
- `headline`
- `currentTaskTitle`
- `steps`
- `deliverables`
- `warnings`
- `stats`

步骤项按时间顺序生成，最小单位是“用户能理解的动作”，不是原始工具调用。

## Error Grouping

对连续重复失败进行聚合：

- 同工具名
- 同错误输出
- 相邻发生

聚合后展示为单条步骤，例如：

- `写入文件失败，已连续重试 7 次`

## Deliverable Detection

产物主要来源于成功的 `write_file` / `edit`：

- 优先识别常见主产物：`.docx` `.doc` `.pdf` `.html` `.md`
- 其余归类为辅助文件

如果存在写入成功但后续被取消，交付卡展示为“部分完成”。

## Test Strategy

优先覆盖：

- 无 `TodoWrite` 时自动推导当前任务
- 重复错误聚合
- 交付卡展示主产物和风险
- 主聊天区出现任务旅程，不依赖打开侧栏

## Files Expected

- `apps/runtime/src/components/ChatView.tsx`
- `apps/runtime/src/components/ToolIsland.tsx`
- `apps/runtime/src/components/chat-side-panel/view-model.ts`
- `apps/runtime/src/components/chat-journey/*`
- `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
- `apps/runtime/src/components/chat-side-panel/view-model.test.ts`
