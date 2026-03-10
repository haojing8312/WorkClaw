# 飞书路由集成说明

本文档面向集成方、二开团队和维护者，说明 WorkClaw 当前内置的 OpenClaw Feishu 路由能力，以及它在统一渠道连接器架构中的位置。

## 能力范围

- 内置 Sidecar 路由引擎：`apps/runtime/sidecar/vendor/openclaw-core/`
- 统一渠道适配器内核：`apps/runtime/sidecar/src/adapters/`
- Feishu 连接器适配器：`apps/runtime/sidecar/src/adapters/feishu/`
- Rust 路由规则持久化：`im_routing_bindings`
- 聊天页路由决策展示：`matched_by` / `session_key` / `agent_id`

## 连接器边界

- WorkClaw 当前将 Feishu 作为第一个 `ChannelAdapter` 接入。
- Sidecar 对外暴露统一 `/api/channels/*` 接口，Feishu 旧入口保留为兼容别名。
- 后续新增 Slack / Discord / Telegram 等渠道时，目标是复用同一连接器边界，而不是继续在业务层增加新的 Feishu 风格专用逻辑。

## 典型使用场景

- 多员工并行监听与消息分发
- 路由规则可视化配置与模拟验证
- 线程/会话归属一致性追踪
- 连接器状态诊断（重连次数、队列事件、最近事件时间）

## 相关文档

- 员工身份模型：`docs/architecture/employee-identity-model.md`
- OpenClaw 升级维护：`docs/maintainers/openclaw-upgrade.md`
- 飞书 IM 闭环桥接：`docs/integrations/feishu-im-bridge.md`
