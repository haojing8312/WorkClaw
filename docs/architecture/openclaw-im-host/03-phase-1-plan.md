# OpenClaw IM Host：第一阶段实施计划

第一阶段的目标是把 WorkClaw 从“前端私有 Feishu 桥接”切到“后端宿主接管 IM reply”。

## 阶段目标

- 切断前端 Feishu 最终答复 fallback
- 建立 OpenClaw-compatible IM host contract
- 建立 reply host service、chunk planner、delivery trace
- 让 Tauri 后端成为唯一 Feishu reply orchestration 入口

## 任务清单

### 1. 定义宿主 contract

建议新增：

- `apps/runtime/src-tauri/src/commands/openclaw_plugins/im_host_contract.rs`

第一版定义：

- `InboundDispatchRequest`
- `ReplyLifecycleEvent`
- `ReplyDeliveryPlan`
- `ReplyDeliveryResult`
- `AskUserBridgeEvent`
- `ApprovalBridgeEvent`

### 2. 新增 reply host service

建议新增：

- `apps/runtime/src-tauri/src/commands/feishu_gateway/reply_host_service.rs`

职责：

- 接收 runtime 的 final / ask_user / approval / failure
- 生成统一 reply plan
- 调用 official runtime outbound
- 回写 delivery state

### 3. 引入统一 chunk planner

建议新增：

- `apps/runtime/src-tauri/src/commands/feishu_gateway/chunk_planner.rs`

要求：

- 不允许任何业务层再直接 `slice`
- 输出结构化 chunk plan
- 未来可复用给企业微信

### 4. 重构 outbound_service

文件：

- `apps/runtime/src-tauri/src/commands/feishu_gateway/outbound_service.rs`

目标：

- 从“发一条文本”升级为“执行 reply plan”
- 返回结构化 delivery result
- 支持多 chunk

### 5. 为 runtime_service 增加 reply trace

文件：

- `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs`

目标：

- 引入 logical reply id
- 能区分 completed / failed / failed_partial
- 记录 request_id 与 chunk 的关系
- 改善 stdout 读取与超时日志

### 6. 冻结前端 Feishu fallback

文件：

- `apps/runtime/src/scenes/useImBridgeIntegration.ts`

目标：

- 保留 IM session UI 管理
- 删除或 feature-flag 掉：
  - Feishu fallback poll
  - Feishu retry
  - Feishu final reply send
  - 任何 Feishu `slice(0, 1800)`

### 7. 调整测试方向

文件：

- `apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx`

目标：

- 删除基于前端 Feishu fallback 的测试假设
- 新增 chunk integrity、delivery result、partial failure 的后端测试

### 8. 引入 diagnostics 文档与 trace 存储

建议新增：

- `apps/runtime/src-tauri/src/commands/feishu_gateway/delivery_trace.rs`

至少记录：

- session_id
- thread_id / chat_id
- logical_reply_id
- planned / delivered chunk count
- final state

## 验收标准

- Feishu 最终答复不再由前端发起
- 长文本不会因前端截断丢后半段
- outbound 具备统一 chunk planner
- reply trace 可观察
- 下一阶段已具备接回 official lifecycle 的结构基础

## 截至 2026-04-19 的完成状态

Phase 1 的核心目标已经达成，WorkClaw 已从“前端私有 Feishu 桥接”切到“后端宿主接管 reply orchestration”的结构基线。

### 已完成

- 前端 `useImBridgeIntegration` 不再承担 Feishu 最终答复 fallback、轮询补发或最终 reply send；Feishu 分支保留 IM session 派发与 `ask_user` 跟进路由，最终答复主路径已回到宿主侧。
- 宿主侧 contract、reply plan、chunk planner、delivery trace 已落地，并进一步收敛到通用 `im_host/*` 平台层；`openclaw_plugins/im_host_contract.rs` 作为兼容入口 re-export 通用 contract。
- `feishu_gateway/outbound_service.rs` 已从“发一条文本”升级为执行 `ImReplyDeliveryPlan`，支持多 chunk 发送与 `Completed / Failed / FailedPartial` 结果表达。
- `openclaw_plugins/runtime_service.rs` 已记录 reply lifecycle，并向前端投影 `latest_reply_completion`，可区分 `awaiting_user`、`awaiting_approval`、`failed`、`stopped`、`completed` 等状态。
- 设置页已开始展示统一 channel registry，并为 Feishu reply completion 暴露 next-step guidance 与快捷入口，便于定位员工关联或高级配置问题。

### 本阶段明确未完成

- 还未宣称已经完整对齐 OpenClaw official lifecycle。`waitForIdle -> markFullyComplete -> markDispatchIdle` 的最终 completion barrier 仍属于第二阶段收口范围。
- processing reaction / typing、`ask_user`、`approval` 的完整 IM 生命周期闭环仍按第二阶段继续打磨。
- 通用 `im_host` framework 虽已开始落地，但让 Feishu 完全退化为 adapter、并用同一宿主 contract 证明 WeCom 等第二通道，仍属于第三阶段工作。

### 与最初计划的结构差异

- 第一版计划把 contract、chunk planner、delivery trace 等能力写成 Feishu/OpenClaw 侧专属文件。
- 实际实现中，这些能力进一步上提到了 `apps/runtime/src-tauri/src/commands/im_host/`，再由 `feishu_gateway`、`openclaw_plugins`、`wecom_gateway` 复用。
- 这意味着实现路径比 Phase 1 计划更早进入了第三阶段的“平台化收敛”，但不改变本阶段的交付边界：前端退场、后端接管、reply plan / chunk / trace 成为宿主能力。

### 进入第二阶段前的冻结结论

- 可以认为 Phase 1 已建立了“接回 official lifecycle”的结构基础。
- 后续阶段应以 lifecycle 语义对齐与多渠道平台化为主，而不是回到前端补丁式 Feishu reply 编排。

### 2026-04-19 收尾验证

- `pnpm test:rust-fast`：PASS
- `pnpm --dir apps/runtime test -- App.im-feishu-bridge.test.tsx`：PASS（17 tests）
- `pnpm --dir apps/runtime test -- src/components/__tests__/SettingsView.feishu.test.tsx`：PASS（5 tests）

这些验证说明：

- Feishu 前端 bridge 已冻结在“分发与跟进”边界，而不是继续承担最终 reply 发送。
- `latest_reply_completion` 与设置页 guidance 已能反映宿主侧 reply completion 投影。
- Phase 1 的结构基线可以被冻结，后续工作应转入第二阶段 lifecycle 语义收口。
