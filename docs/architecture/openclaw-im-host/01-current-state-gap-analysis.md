# OpenClaw IM Host：现状差距分析

本文档总结当前 WorkClaw 与 OpenClaw 官方 IM 架构之间的关键差距，以及本次调研确认的主要问题点。

## 当前 WorkClaw 链路

飞书消息进入 WorkClaw 后，当前链路大致分为三段：

1. `feishu_gateway` / `openclaw_plugins` 负责接收官方插件的 `dispatch_request` 并路由到 WorkClaw session
2. 前端 `useImBridgeIntegration` 监听 `im-role-dispatch-request`、`stream-token`、`ask-user-event`
3. Feishu 最终答复在前端通过 fallback 轮询 `get_messages` 后再调用 `send_feishu_text_message`

这意味着最终 IM 回推并不是由后端与官方插件生命周期驱动，而是由前端观察本地会话状态后自行补发。

## OpenClaw 官方链路

OpenClaw 官方飞书链路的核心特点是：

- 插件侧自带 gateway / monitor / inbound dispatch
- reply dispatcher 负责 typing、chunking、streaming card、final deliver
- 完成顺序强调 `dispatchReplyFromConfig -> waitForIdle -> markFullyComplete -> markDispatchIdle`
- 生命周期行为由插件侧统一承接，不依赖前端轮询聊天记录

## 关键差距

### 1. 最终回复编排位置错误

WorkClaw 现在把 Feishu 最终回复回推放在前端，这与 OpenClaw 官方“插件侧 reply lifecycle 主导”的模型相反。

结果是：

- 前端页面存活与否会影响 IM 回推
- 文本完整性、重试、补发、截断逻辑分散在 UI 层
- 宿主后端无法准确知道整轮 reply 是否真正完成

### 2. 存在明确的前端截断风险

本地代码已确认：

- `useImBridgeIntegration.ts` 的 Feishu fallback 路径会对 `latestAssistant.content.slice(0, 1800)`
- `sendTextToImThread()` 还会再次 `.slice(0, 1800)`

因此只要最终答复超过 1800 字符，后半段会在进入 Tauri 发送前直接丢失。

### 3. Plugin host 的 reply lifecycle 仍是半桩实现

当前 `plugin-host` 中：

- `dispatchReplyFromConfig()` 只发 `dispatch_request`
- `markDispatchIdle()` 为空实现
- `queuedFinal` 返回值固定
- helper 看似具备 dispatcher 语义，但实际没有承接 OpenClaw 官方的完整 reply 生命周期

这会使 WorkClaw 看起来“接了官方插件”，但实际上没有真正复用上游修过的 lifecycle 行为。

### 4. Outbound 完成语义过薄

当前 Rust outbound 路径更接近“一次请求等一个 `send_result`”。

这对于多 chunk、multi-step reply lifecycle 来说过于薄弱，容易出现：

- 第一段成功、后续段失败，但系统不清楚整轮状态
- stdout 读异常或超时后，后续结果丢失
- 只能看到某一次命令结果，看不到 logical reply 全貌

### 5. 缺少 OpenClaw 官方 processing reaction 体验

OpenClaw 官方飞书插件会在处理期间打 `Typing` reaction，最终回复时再移除。WorkClaw 当前没有完整承接这套流程，因此用户体验已经与官方插件有显著差距。

## 已确认的主要 bug / 风险

### 高置信主因：Feishu 最终答复前端截断

这是当前“只收到前半段”的最高置信根因。问题不在上游 OpenClaw，而在 WorkClaw 前端私有 fallback 回推路径。

### 上游同类风险：waitForIdle / markFullyComplete 时序

OpenClaw 上游代码中已经明确修过“最终内容截断”的生命周期问题，说明 reply lifecycle 完成屏障是实打实的高风险点。WorkClaw 既然要兼容插件生态，就必须对齐这部分语义，而不是自行定义完成条件。

### 协议层风险：单次 send_result 语义不足

即使修复前端截断，如果宿主仍按“一次请求一个结果”理解 outbound，后续在多段发送、streaming close、partial failure 等场景仍然会不稳。

## 结论

WorkClaw 现在最大的问题不是单个 bug，而是 IM 层职责边界错误：

- 前端承担了不该承担的 reply orchestration
- 宿主后端没有形成完整的 logical reply 模型
- plugin-host 还没有真正接住 OpenClaw 官方 lifecycle

因此后续重构不应停留在“补一个截断修复”，而应整体转向 OpenClaw-compatible host 架构。
