# OpenAI Tag Compatibility Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在现有字符安全的 OpenAI thinking 过滤器上，对齐 `openclaw` 已覆盖的 `<thinking>...</thinking>` 和 `<final>...</final>` 标签兼容。

**Architecture:** 保持 `process_openai_sse_text()` 和上层消息协议不变，只扩展 `apps/runtime/src-tauri/src/adapters/openai.rs` 内的流式标签状态机。`think` / `thinking` 标签内部内容隐藏，`final` 标签仅移除外层标签并保留正文。

**Tech Stack:** Rust, cargo test

---

### Task 1: Add failing tests for openclaw-compatible tags

**Files:**
- Modify: `apps/runtime/src-tauri/src/adapters/openai.rs`

**Step 1: Write the failing tests**

新增测试覆盖：
- `<thinking>内部</thinking>你好`
- `<think>内部</think><final>可见结果</final>`
- 跨 chunk 的 `<final>` 开闭标签

**Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib openai_tag -- --nocapture -q`

Expected:
- 至少一个测试失败
- 失败原因是当前状态机不识别 `thinking` 或 `final`

### Task 2: Extend the state machine with fixed tag support

**Files:**
- Modify: `apps/runtime/src-tauri/src/adapters/openai.rs`

**Step 1: Write minimal implementation**

- 扩展标签匹配集合为 `think`、`thinking`、`final`
- `think` / `thinking` 进入隐藏模式
- `final` 仅剥离标签，不隐藏正文
- 继续支持跨 chunk 前缀缓存

**Step 2: Run targeted tests**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib openai -- --nocapture -q`

Expected:
- 新增测试通过
- 既有 OpenAI 适配器测试通过

### Task 3: Build a fresh installer

**Files:**
- No source changes expected

**Step 1: Run backend verification**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib -- --nocapture -q`

Expected:
- 全部通过

**Step 2: Build app installer**

Run: `pnpm build:app`

Expected:
- NSIS `.exe` 产物生成成功
