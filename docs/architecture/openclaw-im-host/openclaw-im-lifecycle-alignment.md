# OpenClaw IM Lifecycle Alignment

本文档记录 WorkClaw 当前对齐 OpenClaw 官方 IM reply lifecycle 的实现语义。

## 官方顺序

OpenClaw 官方飞书链路强调以下顺序：

1. `dispatchReplyFromConfig`
2. `waitForIdle`
3. `markFullyComplete`
4. `markDispatchIdle`

其中关键点不是“开始发送了最终回复”，而是“所有排队中的 deliver 都已经 flush 完成后，才能宣告 fully complete”。

## WorkClaw 当前对齐

### plugin-host

文件：

- `apps/runtime/plugin-host/src/runtime.ts`
- `apps/runtime/plugin-host/scripts/run-feishu-host.mjs`

当前 host 会显式发出 `reply_lifecycle` 事件：

- `reply_started`
- `processing_started`
- `ask_user_requested`
- `ask_user_answered`
- `approval_requested`
- `approval_resolved`
- `interrupt_requested`
- `resumed`
- `failed`
- `stopped`
- `tool_chunk_queued`
- `block_chunk_queued`
- `final_chunk_queued`
- `wait_for_idle`
- `idle_reached`
- `fully_complete`
- `dispatch_idle`
- `processing_stopped`

### 时序保证

`withReplyDispatcher` 已按以下顺序执行：

1. `run()`
2. `dispatcher.waitForIdle()`
3. `dispatcher.markComplete()`
4. `onSettled()`

这意味着 WorkClaw 不再走“先 complete 再 idle”的危险路径。

另外，当前前端 `latest_reply_completion` 的完成态投影已经进一步收紧：

- `fully_complete` 不再直接投影为 `Completed`
- 只有 `dispatch_idle` 才会被前端和诊断层视为“这条回复真正结束”

这让 WorkClaw 的可观测完成语义更接近 OpenClaw 官方“flush 结束后再宣告完成”的边界。

## Runtime Host Observability

文件：

- `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs`
- `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`

当前 Tauri runtime 会：

- 解析 `reply_lifecycle` stdout 事件
- 将其写入 `OpenClawPluginFeishuRuntimeStatus.recent_reply_lifecycle`
- 同步写入 `recent_logs`

这样前端和诊断工具已经可以观察到“处理中开始了没有、idle 到了没有、dispatch idle 到了没有”。

同时，前端设置页已开始把恢复态单独展示出来：

- `ask_user_answered`
- `approval_resolved`
- `resumed`

这些 phase 仍然投影在 `running` 大类下，但 UI 不再把它们和普通“处理中”混为一谈，而是显示为“已恢复处理中”，帮助操作者区分“仍在初始处理中”与“已经收到继续执行所需信息，正在恢复推进”。

## 仍未完全对齐的部分

以下能力还需要下一步继续补：

- 让 ask_user / approval / interrupt 拥有更细粒度的独立 lifecycle phase，而不只是复用 `processing_stopped`
- 让 reply lifecycle 不只存在于 plugin-host 内部事件，还能驱动统一 IM host state machine
- 将同一套 contract 扩展到企业微信等 IM 渠道

## 新增对齐进展

截至本轮实现：

- `processing_started` / `processing_stopped` 已真正映射到飞书官方 `Typing` reaction 的启停
- Feishu `ask_user` 已改为宿主发送，前端不再直接往飞书线程代发澄清问题
- Feishu `approval_requested` 在发出审批消息前会先结束 processing reaction，避免“还在处理中”与“等待审批”同时显示
- 宿主 stdin 协议已支持通用 `lifecycle_event` 命令，可显式发送 `ask_user_requested / approval_requested`
- `run_failed / run_stopped` 已开始映射为独立 lifecycle phase，而不再只依赖 `processing_stopped + finalState`
- `answer_user_question / resolve_approval` 已开始映射为 `ask_user_answered / approval_resolved`
- `cancel_agent(session_id?)` 已开始映射为 `interrupt_requested`
- `ask_user` 收到回答、`approval_flow` 收到决策后，runtime 恢复执行会映射为 `resumed`
- Feishu interactive 闭环现在已有更硬的宿主级顺序保证：
  - 进入等待时，先 `processing_stopped`，再发 `ask_user_requested / approval_requested`
  - 恢复执行时，`ask_user_answered / approval_resolved / resumed` 会继续路由到注册宿主，而不是只停留在桌面本地状态
- 设置页已经能把恢复态展示为“已恢复处理中”，不再把 `ask_user_answered / approval_resolved / resumed` 压扁成和普通 `running` 完全相同的可见状态

这意味着 WorkClaw 在 Feishu 上已经不只是 final answer 对齐 OpenClaw，而是开始把 `final / ask_user / approval` 三类关键出站路径收束到宿主层。

## 当前结论

WorkClaw 现在已经从“只有 send_result 的单点返回模型”，走到“具备官方 lifecycle 语义和 idle barrier 的宿主基础层”。这还不是最终完成态，但已经和 OpenClaw 官方架构进入同一条演进路线。
