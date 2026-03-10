# OpenClaw 升级维护手册

本文档用于维护者执行内置 OpenClaw 核心同步与回归验证，包括当前已启用的 routing subset，以及为后续多 IM 连接器预留的 vendor lane。

## 前置条件

1. 准备 OpenClaw 上游仓库本地副本
2. 设置环境变量 `OPENCLAW_UPSTREAM_PATH` 指向上游仓库路径

## 升级步骤

### 1. 路由核心子集（当前已启用）

1. 执行同步脚本：`node scripts/sync-openclaw-core.mjs`
2. 核对并更新以下文件：
   - `apps/runtime/sidecar/vendor/openclaw-core/UPSTREAM_COMMIT`
   - `apps/runtime/sidecar/vendor/openclaw-core/PATCHES.md`

### 2. IM adapter vendor lane（当前仅预留，不默认启用）

1. 准备 OpenClaw 上游仓库本地副本，并设置：
   - `OPENCLAW_IM_UPSTREAM_PATH`
   - 或沿用 `OPENCLAW_UPSTREAM_PATH`
2. 执行同步脚本：`node scripts/sync-openclaw-im-core.mjs`
3. 核对并更新以下文件：
   - `apps/runtime/sidecar/vendor/openclaw-im-core/UPSTREAM_COMMIT`
   - `apps/runtime/sidecar/vendor/openclaw-im-core/PATCHES.md`
4. 在真正启用第二个渠道前，先补全同步 manifest，再引入对应 adapter 包装层。

## 回归验证

1. Vendor lane 元数据检查：`node --test scripts/check-openclaw-vendor-lane.test.mjs`
2. Sidecar 测试：`pnpm --dir apps/runtime/sidecar test`
3. 路由回归：`cargo test --test test_openclaw_gateway --test test_openclaw_route_regression -- --nocapture`

## 建议补充检查

- Feishu 路由配置页可视化行为
- 聊天页路由决策卡片字段完整性
- 多员工监听与自动恢复流程
- 新渠道接入时，确认所有上游代码仍被限制在 `apps/runtime/sidecar/vendor/` 和 `apps/runtime/sidecar/src/adapters/` 边界内
