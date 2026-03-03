# Default WorkDir And Employee UX Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 建立统一默认工作目录体系，移除新建会话目录弹窗，完善员工配置（`role_id` 唯一、模板化输入、隐藏技术 ID）。

**Architecture:** 以 Tauri `app_settings` 为配置中心新增运行时偏好命令，由后端统一解析/创建默认目录并在会话创建时兜底。前端仅负责展示和交互，不承担配置真值逻辑。员工配置采用前后端双校验，确保 `role_id` 全局唯一并保持飞书字段非必填。

**Tech Stack:** Rust (Tauri, sqlx), React + TypeScript, Vitest, tokio tests

---

### Task 1: Runtime Preferences Backend

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/runtime_preferences.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src-tauri/tests/test_runtime_preferences.rs`

1. 写失败测试：默认目录解析、自动创建目录。
2. 运行测试确认失败。
3. 实现 `get/set/resolve` 命令与 helper。
4. 运行测试确认通过。

### Task 2: Session Creation Default Dir Fallback

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Test: `apps/runtime/src-tauri/tests/test_chat_create_session_default_workdir.rs`

1. 写失败测试：`create_session` 传空目录时自动写入默认目录。
2. 运行测试确认失败。
3. 最小实现：在 `create_session` 使用 preferences fallback。
4. 运行测试确认通过。

### Task 3: Employee Role Id Uniqueness

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/tests/helpers/mod.rs`
- Modify: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`

1. 写失败测试：重复 `role_id` 保存失败。
2. 运行测试确认失败。
3. 添加唯一索引迁移与冲突错误处理。
4. 运行测试确认通过。

### Task 4: New Session UI Flow

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`

1. 写失败测试：创建会话不调用目录选择。
2. 运行测试确认失败。
3. 移除新建会话目录弹窗逻辑，直接调用 `create_session`。
4. 运行测试确认通过。

### Task 5: Employee UX Templates And Labels

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/types.ts`
- Test: `apps/runtime/src/components/__tests__/SettingsView.feishu.test.tsx` (如需)

1. 写失败测试（或扩展现有测试）：不显示 `builtin-general` 技术文案，模板可一键填充。
2. 运行测试确认失败。
3. 实现角色模板、友好标签、`feishu_open_id` 文案。
4. 运行测试确认通过。

### Task 6: Verify And Regressions

**Files:**
- Modify: `README.zh-CN.md`（必要时）
- Modify: `README.md`（必要时）

1. 运行前端与后端相关测试集合。
2. 修复回归并复测。
3. 汇总变更与验证证据。
