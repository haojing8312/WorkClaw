# Default WorkDir And Employee UX Refactor Design

## Background

当前会话创建流程要求用户每次手动选择目录，员工页面暴露技术细节（如 `builtin-general`），以及员工角色配置缺少模板化引导。  
目标是将“默认工作目录 + 会话创建 + 员工配置”重构为统一配置体系，提升长期稳定性与可维护性。

## Confirmed Decisions

- 全局默认工作目录：`C:\Users\<username>\WorkClaw\workspace`
- 默认目录不存在时：自动创建并直接使用
- 新建会话：不再先弹目录选择，自动应用默认目录；用户可在会话内再改
- 员工 `feishu_open_id`：非必填，仅用于飞书精准路由
- 员工 `role_id`：全局唯一
- 用户界面：不暴露 `builtin-general` 技术 ID

## Architecture

### 1) Backend config center

- 新增 `commands/app_settings.rs`，提供统一命令：
  - `get_runtime_preferences`
  - `set_runtime_preferences`
  - `resolve_default_work_dir`
- 统一管理 `app_settings` 中的运行时偏好（默认目录、后续可扩展项）。
- `resolve_default_work_dir` 负责：
  - 读取配置或生成默认 `USERPROFILE\\WorkClaw\\workspace`
  - 确保目录存在（不存在则创建）
  - 失败时返回明确错误

### 2) Session creation behavior

- `create_session` 增加后端兜底逻辑：
  - 若前端未传 `work_dir` 或为空，后端自动解析默认目录
  - 写入 `sessions.work_dir`
- 前端 `App.tsx` 去除新建会话前的“目录选择”阻塞流程，改为直接创建。

### 3) Employee model hardening

- 数据库层新增唯一索引：`agent_employees(role_id)`（忽略空白后保存）。
- `upsert_agent_employee_with_pool` 增加冲突检测，返回可读错误信息。
- 员工默认目录保存时：
  - 为空则跟随全局默认目录
  - 非空则写入指定目录

### 4) UX simplification

- 新增“常用角色模板”（产品经理、技术负责人、客服、运营等），一键填充：
  - `role_id`
  - 人格/职责描述
- `builtin-general` 在 UI 中不再显示为原始 ID，显示“通用助手（系统默认）”。
- `feishu_open_id` 输入框增加说明“可空，仅用于飞书 @ 精准路由”。

## Data And Migration

- 复用现有 `app_settings` 表，新增键：
  - `runtime_default_work_dir`
- `db.rs` 启动迁移新增：
  - `INSERT OR IGNORE` 默认设置
  - `CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_employees_role_id_unique ON agent_employees(role_id)`

## Error Handling

- 默认目录创建失败：后端返回明确错误，前端 toast/消息提示。
- `role_id` 冲突：保存失败并提示“角色 ID 已存在，请更换”。
- 兼容旧数据：`role_id` 唯一索引创建前先检测重复并给迁移日志提示（不中断应用启动，先软失败提示）。

## Tests

- 前端：
  - 会话创建不再调用目录选择弹窗
  - 创建会话调用中携带默认目录或由后端兜底
  - UI 不显示 `builtin-general` 字面量
- 后端：
  - 默认目录解析与自动创建
  - `create_session` 空目录时自动填充
  - `role_id` 唯一性校验
  - 员工会话继承默认目录

## Rollout

- 保持命令向后兼容（`create_session` 仍接受 `work_dir`）。
- 先上线配置中心与后端兜底，再切换前端行为，避免灰度期间失败。
