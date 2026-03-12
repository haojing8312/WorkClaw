# Tool Confirm Hang Fix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 让工具确认链路在未确认或确认事件丢失时也能可靠结束 run，不再让聊天界面无限转圈。

**Architecture:** 保持现有工具确认机制不变，只在后端为确认等待加短超时和可收敛行为，在前端为确认弹窗增加自动拒绝清理。这样既能止住无限等待，也不会放宽现有权限模型。

**Tech Stack:** Rust (`tokio`, `tauri`), React/TypeScript, Vitest, Testing Library

---

### Task 1: 后端确认等待行为测试

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`

**Step 1: Write the failing test**

增加一个测试，验证当工具确认通道没有收到前端响应时，执行不会无限卡住，而是把工具当作拒绝处理并返回。

**Step 2: Run test to verify it fails**

Run: `cargo test tool_confirmation_timeout_is_treated_as_rejection --manifest-path apps/runtime/src-tauri/Cargo.toml --lib -- --nocapture`

Expected: FAIL，因为当前实现会等待很久且没有可测的短超时路径。

**Step 3: Write minimal implementation**

在工具确认等待逻辑中抽出短超时辅助函数，允许测试直接覆盖“无响应即拒绝”的行为。

**Step 4: Run test to verify it passes**

Run: `cargo test tool_confirmation_timeout_is_treated_as_rejection --manifest-path apps/runtime/src-tauri/Cargo.toml --lib -- --nocapture`

Expected: PASS

### Task 2: 后端实现短超时收口

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`

**Step 1: Write the failing test**

增加一个测试，验证确认超时后会发出 `tool-call-event(error)` 或至少返回“用户拒绝/确认超时”结果，保证执行继续向前。

**Step 2: Run test to verify it fails**

Run: `cargo test tool_confirmation_timeout_emits_rejection_result --manifest-path apps/runtime/src-tauri/Cargo.toml --lib -- --nocapture`

Expected: FAIL

**Step 3: Write minimal implementation**

将工具确认等待时间改为短超时；超时后统一视为拒绝，并沿用现有错误事件与工具结果路径。

**Step 4: Run test to verify it passes**

Run: `cargo test tool_confirmation_timeout_emits_rejection_result --manifest-path apps/runtime/src-tauri/Cargo.toml --lib -- --nocapture`

Expected: PASS

### Task 3: 前端自动拒绝兜底测试

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Write the failing test**

增加一个测试，验证收到 `tool-confirm-event` 后，如果组件卸载或会话切换导致确认弹窗被清理，前端会调用 `confirm_tool_execution(false)`。

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

Expected: FAIL

**Step 3: Write minimal implementation**

在 `ChatView` 的确认事件 effect 清理阶段检测未处理确认，并自动调用拒绝。

**Step 4: Run test to verify it passes**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

Expected: PASS

### Task 4: 回归验证

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Run focused Rust tests**

Run: `cargo test tool_confirmation_timeout --manifest-path apps/runtime/src-tauri/Cargo.toml --lib -- --nocapture`

Expected: PASS

**Step 2: Run focused frontend tests**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

Expected: PASS

**Step 3: Real scenario verification**

在本地桌面应用中用“让我先查看当前工作目录”发起一次工具型请求，确认：

1. 不再无限转圈
2. 若前端未确认，最终能停止并给出拒绝/失败收口
3. 若前端确认，工具能继续执行

