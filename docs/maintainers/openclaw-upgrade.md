# OpenClaw 升级维护手册

本文档用于维护者执行内置 OpenClaw 核心同步与回归验证。

## 前置条件

1. 准备 OpenClaw 上游仓库本地副本
2. 设置环境变量 `OPENCLAW_UPSTREAM_PATH` 指向上游仓库路径

## 升级步骤

1. 执行同步脚本：`node scripts/sync-openclaw-core.mjs`
2. 核对并更新以下文件：
   - `apps/runtime/sidecar/vendor/openclaw-core/UPSTREAM_COMMIT`
   - `apps/runtime/sidecar/vendor/openclaw-core/PATCHES.md`

## 回归验证

1. Sidecar 测试：`pnpm --dir apps/runtime/sidecar test`
2. 路由回归：`cargo test --test test_openclaw_gateway --test test_openclaw_route_regression -- --nocapture`

## 建议补充检查

- Feishu 路由配置页可视化行为
- 聊天页路由决策卡片字段完整性
- 多员工监听与自动恢复流程
