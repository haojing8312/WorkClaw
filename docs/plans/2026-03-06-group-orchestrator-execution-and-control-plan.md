# Group Orchestrator Execution And Control Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将“拉群协作”从静态模拟升级为可关联会话的运行链路，并补齐实时展示、取消、重试与回归验证。

**Architecture:** 后端在 `employee_agents` 中扩展 group run 生命周期命令（启动、查询、取消、失败重试），并为 run 绑定会话；前端在 EmployeeHub 启动后自动跳转该会话，ChatView 通过 run snapshot 渲染协作看板。保留现有事件时间线，新增 DB 快照兜底，避免丢事件导致看板空白。

**Tech Stack:** Rust (Tauri + sqlx), React + TypeScript, Vitest, SQLite.

---

### Task 1: Start Run 绑定会话并持久化可查询状态

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`

### Task 2: 增加 run 控制命令（snapshot / cancel / retry_failed）

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`

### Task 3: 前端自动关联会话 + ChatView 快照看板

**Files:**
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx`

### Task 4: 回归验证与收口

**Commands:**
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --nocapture`
- `pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx src/components/__tests__/ChatView.im-routing-panel.test.tsx`

