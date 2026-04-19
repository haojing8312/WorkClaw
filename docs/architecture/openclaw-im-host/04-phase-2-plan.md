# OpenClaw IM Host：第二阶段实施计划

第二阶段的目标是让 WorkClaw 真正跑上 OpenClaw 官方 IM lifecycle，而不是只把 reply 从前端挪到后端。

## 阶段目标

- 对齐 OpenClaw 官方 reply lifecycle 语义
- 恢复 processing reaction / typing 等官方体验
- 去除 plugin-host 中的关键 stub
- 让宿主知道整轮 reply 何时真正完成

## 任务清单

### 1. 固化 lifecycle 对齐清单

建议新增文档：

- `docs/architecture/openclaw-im-host/openclaw-im-lifecycle-alignment.md`

至少明确这些语义：

- `dispatchReplyFromConfig`
- `withReplyDispatcher`
- `waitForIdle`
- `markFullyComplete`
- `markDispatchIdle`
- typing / reaction start-stop
- chunked final delivery
- ask_user / approval 中断与恢复

### 2. 重写 plugin-host 的 dispatcher bridge

文件：

- `apps/runtime/plugin-host/src/runtime.ts`
- `apps/runtime/plugin-host/scripts/run-feishu-host.mjs`

目标：

- 移除 `markDispatchIdle()` 等空实现
- 停止返回伪 `queuedFinal`
- 真正承接 official plugin 所需 lifecycle callback

### 3. 升级 runtime_service 协议

文件：

- `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs`

目标：

- 从单个 `send_result` 模型升级为 reply-lifecycle-aware 模型
- 至少能观察 reply started / progress / chunk sent / completed / failed
- 让 logical reply completion 成为一等状态

### 4. 接回 processing reaction / typing

目标：

- 开始处理时显示 processing 状态
- ask_user / approval 时正确切换状态
- 最终完成或失败时清理 processing 状态

### 5. 对齐 final flush 屏障

重点：

- 尊重上游 `dispatchReplyFromConfig -> waitForIdle -> markFullyComplete -> markDispatchIdle`
- 禁止宿主过早宣告完成
- 禁止尾段仍在发送时就结束 reply trace

### 6. 对齐 ask_user / approval bridge

原则：

- WorkClaw 保持自己的业务审批能力
- IM 表达与 channel lifecycle 尽量对齐 OpenClaw
- 宿主负责翻译 runtime 状态与 plugin 生命周期

### 7. 收敛 target mapping

目标：

- direct / group / thread / reply-to 解析集中管理
- 避免映射逻辑分散在 ingress、outbound、runtime_service、前端等多处

### 8. 增加兼容回归测试

重点测试：

- lifecycle completion 顺序
- long reply chunk 完整性
- processing reaction start-stop
- ask_user / approval 闭环
- partial failure trace

## 验收标准

- processing reaction / typing 与 OpenClaw 官方插件行为尽量一致
- plugin-host 不再是 reply lifecycle stub
- final completion 由 official lifecycle 与宿主共同确认
- WorkClaw 成为 official IM plugin 的标准宿主
