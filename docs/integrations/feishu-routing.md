# 飞书路由集成说明

本文档面向集成方、二开团队和维护者，说明 WorkClaw 当前内置的 OpenClaw Feishu 路由能力。

## 能力范围

- 内置 Sidecar 路由引擎：`apps/runtime/sidecar/vendor/openclaw-core/`
- Rust 路由规则持久化：`im_routing_bindings`
- 聊天页路由决策展示：`matched_by` / `session_key` / `agent_id`

## 典型使用场景

- 多员工并行监听与消息分发
- 路由规则可视化配置与模拟验证
- 线程/会话归属一致性追踪

## 相关文档

- 员工身份模型：`docs/architecture/employee-identity-model.md`
- OpenClaw 升级维护：`docs/maintainers/openclaw-upgrade.md`
