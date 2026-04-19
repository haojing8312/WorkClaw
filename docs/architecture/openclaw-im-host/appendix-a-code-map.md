# OpenClaw IM Host：附录 A - 代码映射

本文档列出当前调研涉及的核心代码位置，以及建议的保留、删除、重写和新增方向。

## 保留并收边界

- `apps/runtime/src-tauri/src/commands/feishu_gateway/ingress_service.rs`
- `apps/runtime/src-tauri/src/commands/feishu_gateway/pairing_service.rs`
- `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs`
- `apps/runtime/src-tauri/src/commands/openclaw_plugins/*`

这些模块仍是宿主能力的基础，但需要减少业务编排与渠道特化逻辑。

## 应删除或迁出的私有逻辑

- `apps/runtime/src/scenes/useImBridgeIntegration.ts`
  - Feishu fallback poll
  - Feishu retry
  - 最终答复补发
  - 文本截断

- `apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx`
  - 基于前端 Feishu fallback 的测试假设

## 应重写的模块

- `apps/runtime/src-tauri/src/commands/feishu_gateway/outbound_service.rs`
- `apps/runtime/plugin-host/src/runtime.ts`
- `apps/runtime/plugin-host/scripts/run-feishu-host.mjs`
- `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs`

## 建议新增的模块

- `apps/runtime/src-tauri/src/commands/openclaw_plugins/im_host_contract.rs`
- `apps/runtime/src-tauri/src/commands/feishu_gateway/reply_host_service.rs`
- `apps/runtime/src-tauri/src/commands/feishu_gateway/chunk_planner.rs`
- `apps/runtime/src-tauri/src/commands/feishu_gateway/delivery_trace.rs`
- `apps/runtime/src-tauri/src/commands/im_host/*`

## 作为对齐基准的上游参考

- `references/openclaw-lark/src/messaging/inbound/dispatch.ts`
- `references/openclaw/extensions/feishu/src/reply-dispatcher.ts`
- `references/openclaw-lark/src/card/streaming-card-controller.ts`
- `references/openclaw/src/auto-reply/dispatch.ts`
- `references/openclaw/src/auto-reply/reply/reply-dispatcher.ts`

## 当前已确认的关键问题位置

- 前端 Feishu 截断：
  - `apps/runtime/src/scenes/useImBridgeIntegration.ts`

- Outbound 单次 `send_result` 语义过薄：
  - `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs`

- Plugin-host reply lifecycle stub：
  - `apps/runtime/plugin-host/src/runtime.ts`
  - `apps/runtime/plugin-host/scripts/run-feishu-host.mjs`

- OpenClaw 上游 `waitForIdle` 完成屏障参考：
  - `references/openclaw-lark/src/messaging/inbound/dispatch.ts`
