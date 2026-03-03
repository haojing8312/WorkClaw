# WorkClaw 单端合并发布验收清单

## A. 核心目标验收

- [x] 仅保留一个客户端应用（`apps/runtime`）
- [x] `apps/studio` 已从仓库移除
- [x] 应用窗口标题统一为 `WorkClaw`
- [x] 打包能力已并入单端应用（可在应用内完成技能打包）

## B. 功能对等验收

- [x] 技能安装（`.skillpack`）仍可用
- [x] 本地技能导入仍可用
- [x] 会话创建/删除/导出仍可用
- [x] 聊天与工具调用主流程保持可用
- [x] 设置页（模型/MCP/搜索/路由）仍可访问
- [x] 新增“打包”页面可选择目录并生成 `.skillpack`

## C. 迁移与兼容验收

- [x] 历史 `permission_mode` 存储值兼容（`default/accept_edits/unrestricted`）
- [x] 展示层提供用户友好标签映射（谨慎/推荐/全自动）
- [x] 未改动原会话与模型配置表结构（无感迁移基础成立）

## D. 术语与体验验收

- [x] 产品窗口与页面标题不再出现 `Runtime/Studio`
- [x] 用户可见权限模式文案不再显示 `accept_edits/unrestricted`
- [x] README 对外叙述已切换为单端 WorkClaw 口径

## E. 构建与测试验收（本次执行结果）

- [x] 前端构建：`pnpm --filter runtime build` 通过
- [x] 后端编译：`cargo check --lib` 通过
- [x] 新增打包命令测试：`cargo test --test test_packaging_commands -- --nocapture` 通过（3/3）
- [x] 权限映射测试：`cargo test --lib permission_mode_label_is_user_friendly -- --nocapture` 通过

## F. 已知限制

- [ ] 全量 `cargo test` 在当前机器受 Windows 分页/内存限制影响，存在非本次改动导致的编译失败风险；发布前建议在 CI 或更高内存环境执行一次全量测试。

