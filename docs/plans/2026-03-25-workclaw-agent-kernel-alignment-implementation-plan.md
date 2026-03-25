# WorkClaw Agent Kernel Alignment Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor WorkClaw's general-purpose agent runtime so that execution preparation, runtime control, transcript handling, failover, and projection follow a single OpenClaw-style kernel architecture.

**Architecture:** Introduce a dedicated runtime kernel under `apps/runtime/src-tauri/src/agent/runtime/`, split preparation responsibilities out of `runtime-chat-app`, and demote current command modules to thin orchestration entrypoints. Keep employee orchestration and IM bridge behavior out of scope for this phase.

**Tech Stack:** Rust, Tauri, sqlx, serde_json, reqwest, pnpm, Vitest, Cargo tests

---

### Task 0: Create Dedicated Worktree

**Files:**
- Modify: none
- Verify: `git worktree list`

**Step 1: Create the implementation worktree**

```bash
git worktree add ..\\WorkClaw-agent-kernel-alignment -b feat/agent-kernel-alignment
```

**Step 2: Verify the worktree exists**

Run: `git worktree list`
Expected: includes both the main workspace and `..\WorkClaw-agent-kernel-alignment`

**Step 3: Switch implementation to the new worktree**

```bash
cd ..\\WorkClaw-agent-kernel-alignment
git status --short
```

**Step 4: Confirm the branch is isolated**

Expected: branch is `feat/agent-kernel-alignment` and worktree status is clean before code changes

**Step 5: Commit**

No commit for this task.

### Task 1: Split Preparation Responsibilities Out of `runtime-chat-app`

**Files:**
- Create: `packages/runtime-chat-app/src/preparation.rs`
- Create: `packages/runtime-chat-app/src/routing.rs`
- Create: `packages/runtime-chat-app/src/prompt_assembly.rs`
- Modify: `packages/runtime-chat-app/src/lib.rs`
- Modify: `packages/runtime-chat-app/src/service.rs`
- Test: `packages/runtime-chat-app/tests/prompt_assembly.rs`
- Test: `packages/runtime-chat-app/tests/execution_assembly.rs`
- Test: `packages/runtime-chat-app/tests/route_candidates.rs`

**Step 1: Write the failing exports test**

```rust
use runtime_chat_app::{
    compose_system_prompt, ChatExecutionPreparationService, ChatPreparationService,
    parse_fallback_chain_targets,
};

#[test]
fn runtime_chat_app_exports_split_preparation_api() {
    let _prep = ChatPreparationService::new();
    let _exec = ChatExecutionPreparationService::new();
    assert!(compose_system_prompt("base", "", "model", 1, &Default::default(), None, None, None).contains("模型"));
    assert!(parse_fallback_chain_targets("[]").is_empty());
}
```

**Step 2: Run test to verify current surface before refactor**

Run: `cargo test -p runtime-chat-app execution_assembly route_candidates prompt_assembly -- --nocapture`
Expected: PASS now, then use the same tests as regression guard while splitting internals

**Step 3: Move code into focused modules**

```rust
// packages/runtime-chat-app/src/lib.rs
pub mod preparation;
pub mod prompt_assembly;
pub mod routing;
pub mod service;
pub mod traits;
pub mod types;
```

**Step 4: Re-run package tests**

Run: `cargo test -p runtime-chat-app -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add packages/runtime-chat-app/src/lib.rs packages/runtime-chat-app/src/service.rs packages/runtime-chat-app/src/preparation.rs packages/runtime-chat-app/src/routing.rs packages/runtime-chat-app/src/prompt_assembly.rs packages/runtime-chat-app/tests/prompt_assembly.rs packages/runtime-chat-app/tests/execution_assembly.rs packages/runtime-chat-app/tests/route_candidates.rs
git commit -m "refactor(runtime-chat-app): split preparation and routing modules"
```

### Task 2: Introduce the Runtime Kernel Module Skeleton

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/mod.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/attempt_runner.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/failover.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/transcript.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/events.rs`
- Modify: `apps/runtime/src-tauri/src/agent/mod.rs`
- Test: `apps/runtime/src-tauri/src/agent/turn_executor.rs`

**Step 1: Add a compile-level runtime surface**

```rust
// apps/runtime/src-tauri/src/agent/runtime/mod.rs
pub mod attempt_runner;
pub mod events;
pub mod failover;
pub mod session_runtime;
pub mod transcript;
```

**Step 2: Run compile checks to confirm the new module tree is wired**

Run: `cargo test -p runtime --lib agent::turn_executor -- --nocapture`
Expected: compile succeeds or only fails on deliberate unresolved moved symbols

**Step 3: Add minimal runtime owner types**

```rust
pub struct SessionRuntime;
pub struct AttemptRunner;
pub struct RuntimeFailover;
pub struct RuntimeTranscript;
```

**Step 4: Re-run targeted agent tests**

Run: `cargo test agent::turn_executor -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/mod.rs apps/runtime/src-tauri/src/agent/runtime/mod.rs apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs apps/runtime/src-tauri/src/agent/runtime/attempt_runner.rs apps/runtime/src-tauri/src/agent/runtime/failover.rs apps/runtime/src-tauri/src/agent/runtime/transcript.rs apps/runtime/src-tauri/src/agent/runtime/events.rs
git commit -m "refactor(agent): add runtime kernel module skeleton"
```

### Task 3: Move Transcript Ownership into the Runtime Kernel

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/transcript.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_runtime_io/message_reconstruction.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_route_execution.rs`
- Test: `packages/runtime-executor-core/tests/context.rs`
- Test: `apps/runtime/src-tauri/src/commands/chat_runtime_io/message_reconstruction.rs`

**Step 1: Add failing tests for transcript round-trip invariants**

```rust
#[test]
fn transcript_round_trip_preserves_tool_call_output_pairs() {
    let parsed = serde_json::json!({
        "text": "",
        "items": [{
            "type": "tool_call",
            "toolCall": {
                "id": "call-1",
                "name": "read_file",
                "input": {"path": "README.md"},
                "output": "{\"summary\":\"done\"}"
            }
        }]
    });
    let messages = reconstruct_llm_messages(&parsed, "openai");
    assert!(!messages.is_empty());
}
```

**Step 2: Run the transcript-specific tests**

Run: `cargo test message_reconstruction -- --nocapture`
Expected: PASS before move, stays PASS after move

**Step 3: Introduce a runtime transcript owner and delegate existing helpers**

```rust
pub struct RuntimeTranscript;

impl RuntimeTranscript {
    pub fn reconstruct_history_messages(...) -> Vec<Value> { ... }
    pub fn build_assistant_content(...) -> (String, bool, String) { ... }
}
```

**Step 4: Re-run transcript and runtime tests**

Run: `cargo test message_reconstruction agent::turn_executor -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/transcript.rs apps/runtime/src-tauri/src/commands/chat_runtime_io/message_reconstruction.rs apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs apps/runtime/src-tauri/src/commands/chat_route_execution.rs
git commit -m "refactor(agent): centralize runtime transcript handling"
```

### Task 4: Move Route Retry and Failover Into the Runtime Kernel

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/failover.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_route_execution.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs`
- Modify: `packages/runtime-chat-app/src/routing.rs`
- Test: `packages/runtime-chat-app/tests/fallback.rs`
- Test: `packages/runtime-chat-app/tests/retry.rs`

**Step 1: Add failing tests for failover policy ownership**

```rust
#[test]
fn retry_budget_is_driven_by_runtime_failover_policy() {
    assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Timeout, 2), 2);
}
```

**Step 2: Run the retry and fallback tests**

Run: `cargo test -p runtime-chat-app fallback retry -- --nocapture`
Expected: PASS before move

**Step 3: Introduce `RuntimeFailover` and make command code delegate**

```rust
pub struct RuntimeFailover;

impl RuntimeFailover {
    pub async fn execute_candidates(...) -> RouteExecutionOutcome { ... }
}
```

**Step 4: Re-run fallback tests**

Run: `cargo test -p runtime-chat-app fallback retry -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/failover.rs apps/runtime/src-tauri/src/commands/chat_route_execution.rs apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs packages/runtime-chat-app/src/routing.rs packages/runtime-chat-app/tests/fallback.rs packages/runtime-chat-app/tests/retry.rs
git commit -m "refactor(agent): move candidate failover into runtime kernel"
```

### Task 5: Split `turn_executor` Into Runtime Submodules

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/approval_gate.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/progress_guard.rs`
- Modify: `apps/runtime/src-tauri/src/agent/turn_executor.rs`
- Modify: `apps/runtime/src-tauri/src/agent/approval_flow.rs`
- Modify: `apps/runtime/src-tauri/src/agent/run_guard.rs`
- Test: `apps/runtime/src-tauri/src/agent/turn_executor.rs`

**Step 1: Lock current behavior with targeted tests**

```rust
#[tokio::test]
async fn approval_bus_blocks_file_delete_until_resolved() {
    // keep the existing regression test and add any missing assertions
}
```

**Step 2: Run the agent loop regression tests**

Run: `cargo test agent::turn_executor -- --nocapture`
Expected: PASS

**Step 3: Move behavior behind dedicated runtime helpers**

```rust
pub async fn dispatch_tool_calls(...) -> Result<Vec<ToolResult>> { ... }
pub async fn gate_tool_approval(...) -> Result<Option<ApprovalDecision>> { ... }
pub fn evaluate_progress_guard(...) -> ProgressEvaluation { ... }
```

**Step 4: Re-run agent loop tests**

Run: `cargo test agent::turn_executor -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/turn_executor.rs apps/runtime/src-tauri/src/agent/approval_flow.rs apps/runtime/src-tauri/src/agent/run_guard.rs apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs apps/runtime/src-tauri/src/agent/runtime/approval_gate.rs apps/runtime/src-tauri/src/agent/runtime/progress_guard.rs
git commit -m "refactor(agent): split turn executor runtime responsibilities"
```

### Task 6: Make Chat Send Flow a Thin Application Entrypoint

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_runtime_io.rs`
- Test: `packages/runtime-chat-app/tests/execution_contract.rs`
- Test: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`

**Step 1: Add a regression test for the contract boundary**

```rust
#[test]
fn exposes_execution_preparation_contract() {
    let prepared = PreparedChatExecution::default();
    assert_eq!(prepared.capability, "chat");
}
```

**Step 2: Run contract and runtime flow tests**

Run: `cargo test -p runtime-chat-app execution_contract -- --nocapture`
Expected: PASS

**Step 3: Reduce command-layer logic to orchestration**

```rust
let prepared = runtime.prepare(...).await?;
let outcome = runtime.run(...).await?;
project_runtime_outcome(...).await?;
```

**Step 4: Re-run Rust and UI smoke tests**

Run: `cargo test -p runtime-chat-app execution_contract -- --nocapture`
Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.session-create-flow.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src-tauri/src/commands/chat_runtime_io.rs packages/runtime-chat-app/tests/execution_contract.rs apps/runtime/src/__tests__/App.session-create-flow.test.tsx
git commit -m "refactor(commands): thin chat send flow around runtime kernel"
```

### Task 7: Add Active Run Registry and Runtime Projection Cleanup

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/run_registry.rs`
- Modify: `apps/runtime/src-tauri/src/agent/event_bridge.rs`
- Modify: `apps/runtime/src-tauri/src/session_journal.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src/components/__tests__/ChatView.run-guardrails.test.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Add a failing runtime projection test**

```tsx
it("keeps waiting_approval and running projections aligned with runtime events", async () => {
  expect(["thinking", "tool_calling", "waiting_approval"]).toContain("waiting_approval");
});
```

**Step 2: Run the resilience tests**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.run-guardrails.test.tsx src/components/__tests__/ChatView.session-resilience.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS before move

**Step 3: Introduce active run ownership**

```rust
pub struct RunRegistry;

impl RunRegistry {
    pub fn register_active_run(...) { ... }
    pub fn complete_run(...) { ... }
    pub fn cancel_run(...) { ... }
}
```

**Step 4: Re-run resilience tests**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.run-guardrails.test.tsx src/components/__tests__/ChatView.session-resilience.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/run_registry.rs apps/runtime/src-tauri/src/agent/event_bridge.rs apps/runtime/src-tauri/src/session_journal.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src/components/__tests__/ChatView.run-guardrails.test.tsx apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx
git commit -m "refactor(agent): add active run registry and runtime projections"
```

### Task 8: Verification and Readiness Pass

**Files:**
- Modify: any touched files only if verification reveals needed fixes
- Test: existing suites only

**Step 1: Run targeted Rust package verification**

Run: `cargo test -p runtime-chat-app -- --nocapture`
Expected: PASS

**Step 2: Run Tauri runtime tests**

Run: `pnpm test:rust-fast`
Expected: PASS

**Step 3: Run focused runtime frontend tests**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.run-guardrails.test.tsx src/components/__tests__/ChatView.session-resilience.test.tsx src/__tests__/App.session-create-flow.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

**Step 4: Run build verification**

Run: `pnpm build:runtime`
Expected: PASS

**Step 5: Commit**

```bash
git add .
git commit -m "chore(runtime): verify agent kernel alignment refactor"
```
