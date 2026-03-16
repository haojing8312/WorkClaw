# Agent Run Guardrails Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace generic max-iteration failures with layered run guardrails, typed stop reasons, unified execution budgets, and user-friendly recovery messaging.

**Architecture:** Add a shared Rust run-guard module, route all execution entry points through one budget policy resolver, emit structured stop reasons through session-run events and agent-state events, and update the chat UI to render recoverable stop states differently from real runtime exceptions. Deliver P0 first, then layer in progress fingerprints and browser-heavy stage hints.

**Tech Stack:** Rust (Tauri, `serde`, `serde_json`, `tokio`, `sqlx`), React + TypeScript, Vitest, targeted Cargo unit tests

---

### Task 1: Add Shared Run Guard Types And Defaults

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/run_guard.rs`
- Modify: `apps/runtime/src-tauri/src/agent/mod.rs`
- Modify: `apps/runtime/src-tauri/src/model_errors.rs`
- Test: `apps/runtime/src-tauri/src/agent/run_guard.rs`

**Step 1: Write the failing test**

Add unit tests in `apps/runtime/src-tauri/src/agent/run_guard.rs` for:

```rust
#[test]
fn run_budget_policy_defaults_general_chat_to_12_turns() {
    let policy = RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat);
    assert_eq!(policy.max_turns, 12);
}

#[test]
fn run_stop_reason_kind_serializes_to_snake_case() {
    let value = serde_json::to_string(&RunStopReasonKind::MaxTurns).unwrap();
    assert_eq!(value, "\"max_turns\"");
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml run_budget_policy_defaults_general_chat_to_12_turns -- --nocapture
```

Expected: FAIL because `run_guard.rs` and the new types do not exist yet.

**Step 3: Write minimal implementation**

Create `apps/runtime/src-tauri/src/agent/run_guard.rs` with:

- `RunBudgetScope`
- `RunBudgetPolicy`
- `RunStopReasonKind`
- `RunStopReason`
- default budget helpers for:
  - general chat
  - skill
  - employee
  - sub-agent
  - browser-heavy

Export the module from `apps/runtime/src-tauri/src/agent/mod.rs`.

If `apps/runtime/src-tauri/src/model_errors.rs` already contains overlapping user-facing buckets, keep model errors focused on provider failures and keep run-stop reasons focused on execution governance.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml run_budget_policy_defaults_general_chat_to_12_turns -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml run_stop_reason_kind_serializes_to_snake_case -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/run_guard.rs apps/runtime/src-tauri/src/agent/mod.rs apps/runtime/src-tauri/src/model_errors.rs
git commit -m "feat(runtime): add shared run guard defaults and stop reasons"
```

### Task 2: Refactor AgentExecutor To Return Structured Stop Reasons

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/task_tool.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs`
- Modify: `apps/runtime/src-tauri/src/adapters/openai.rs`
- Modify: `apps/runtime/src-tauri/src/adapters/anthropic.rs`
- Test: `apps/runtime/src-tauri/src/agent/executor.rs`

**Step 1: Write the failing test**

Add executor tests covering the current max-iteration path:

```rust
#[tokio::test]
async fn executor_returns_max_turns_stop_reason_instead_of_generic_error() {
    let result = /* run executor with max_turns = 1 and a looping mock response */;
    assert!(result.is_ok());
    let stop = extract_stop_reason(&result.unwrap());
    assert_eq!(stop.kind, RunStopReasonKind::MaxTurns);
}
```

Also add a focused test for repeated tool failure circuit breaking if you keep the existing failure-streak logic:

```rust
#[test]
fn repeated_tool_failures_map_to_tool_failure_circuit_breaker() {
    assert_eq!(
        map_failure_streak_to_stop_reason(/*...*/).kind,
        RunStopReasonKind::ToolFailureCircuitBreaker
    );
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml executor_returns_max_turns_stop_reason_instead_of_generic_error -- --nocapture
```

Expected: FAIL because `executor.rs` still returns `Err(anyhow!("达到最大迭代次数 ..."))`.

**Step 3: Write minimal implementation**

In `apps/runtime/src-tauri/src/agent/executor.rs`:

- replace the raw max-iteration `Err(anyhow!(...))` path with a structured stop result
- emit `agent-state-event` with:
  - `state: "stopped"`
  - `stop_reason_kind: "max_turns"`
  - user-facing title and message
- keep `state: "error"` only for genuine internal failures

In `task_tool.rs`, `skill_invoke.rs`, `openai.rs`, and `anthropic.rs`:

- stop matching or returning raw `达到最大迭代次数 ...` strings as a control signal
- propagate typed stop reasons instead

Prefer a small internal helper such as:

```rust
fn stop_run(kind: RunStopReasonKind, iteration: usize, detail: Option<String>) -> Vec<Value>
```

if that helps keep `executor.rs` readable.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml executor_returns_max_turns_stop_reason_instead_of_generic_error -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/executor.rs apps/runtime/src-tauri/src/agent/tools/task_tool.rs apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs apps/runtime/src-tauri/src/adapters/openai.rs apps/runtime/src-tauri/src/adapters/anthropic.rs
git commit -m "refactor(runtime): return structured run stop reasons"
```

### Task 3: Unify Budget Resolution Across Chat, Skill, Employee, And Sub-Agent Flows

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_route_execution.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/task_tool.rs`
- Modify: `apps/runtime/src-tauri/src/agent/run_guard.rs`
- Test: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Test: `apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs`

**Step 1: Write the failing test**

Add tests for budget selection:

```rust
#[test]
fn skill_flow_uses_skill_default_budget_when_no_override_exists() {
    let policy = resolve_run_budget_for_skill(None);
    assert_eq!(policy.max_turns, 16);
}

#[test]
fn employee_flow_caps_unsafe_override_values() {
    let policy = resolve_run_budget_for_employee(Some(999));
    assert!(policy.max_turns <= 24);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml skill_flow_uses_skill_default_budget_when_no_override_exists -- --nocapture
```

Expected: FAIL because budget resolution is still spread across multiple files with hard-coded values.

**Step 3: Write minimal implementation**

Move budget selection behind shared helpers in `run_guard.rs`.

Replace hard-coded values in:

- `chat_send_message_flow.rs`
- `employee_agents.rs`
- `chat_route_execution.rs`
- `task_tool.rs`

Rules:

- skill-specific overrides remain supported
- overrides are clamped to safe ranges
- browser-heavy tasks can opt into a broader budget
- sub-agent budgets remain smaller than parent budgets by default

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml skill_flow_uses_skill_default_budget_when_no_override_exists -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml employee_flow_caps_unsafe_override_values -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs apps/runtime/src-tauri/src/commands/chat_route_execution.rs apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/src/agent/tools/task_tool.rs apps/runtime/src-tauri/src/agent/run_guard.rs
git commit -m "refactor(runtime): unify run budget resolution"
```

### Task 4: Persist Structured Stop Events And Guard Warnings

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_runtime_io.rs`
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_session_io.rs`
- Test: `apps/runtime/src-tauri/src/commands/chat_runtime_io.rs`
- Test: `apps/runtime/src-tauri/src/db.rs`

**Step 1: Write the failing test**

Add a test proving stopped runs persist distinct error kinds and event payloads:

```rust
#[tokio::test]
async fn append_run_stopped_event_persists_loop_detected_reason() {
    let event = append_run_stopped_event(/* loop_detected payload */).await.unwrap();
    assert_eq!(event.event_type, "run_stopped");
    assert!(event.payload_json.contains("\"kind\":\"loop_detected\""));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml append_run_stopped_event_persists_loop_detected_reason -- --nocapture
```

Expected: FAIL because the new event type and payload contract do not exist yet.

**Step 3: Write minimal implementation**

In `chat_runtime_io.rs`:

- add helpers to append:
  - `run_guard_warning`
  - `run_stopped`
  - optional `progress_snapshot`

In `db.rs` and `chat_session_io.rs`:

- preserve `session_runs.error_kind` for stop reasons such as:
  - `max_turns`
  - `max_session_turns`
  - `timeout`
  - `loop_detected`
  - `no_progress`
  - `cancelled`
- expose enough event payload to render stop summaries later

Favor extending existing event storage instead of adding a large migration in this phase.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml append_run_stopped_event_persists_loop_detected_reason -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib chat_runtime_io
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat_runtime_io.rs apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/commands/chat_session_io.rs
git commit -m "feat(runtime): persist structured run guard events"
```

### Task 5: Update ChatView To Distinguish Recoverable Stops From Real Errors

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/types.ts`
- Create: `apps/runtime/src/lib/run-stop-display.ts`
- Create: `apps/runtime/src/components/__tests__/ChatView.run-guardrails.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Write the failing test**

Add a frontend test for the new display rules:

```ts
it("renders loop-detected stop as a stopped task instead of execution exception", async () => {
  // emit agent-state-event with state=stopped and stop_reason_kind=loop_detected
  expect(screen.getByText("任务疑似卡住，已自动停止")).toBeInTheDocument();
  expect(screen.queryByText(/执行异常/)).not.toBeInTheDocument();
});
```

Add one regression test proving real runtime errors still render as `执行异常`.

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime test -- --run src/components/__tests__/ChatView.run-guardrails.test.tsx
```

Expected: FAIL because `ChatView.tsx` still maps only `agentState.state === "error"` and has no `stopped` stop-reason rendering.

**Step 3: Write minimal implementation**

In `apps/runtime/src/types.ts`:

- extend the agent-state event shape with:
  - `stop_reason_kind`
  - `stop_reason_title`
  - `stop_reason_message`

Create `apps/runtime/src/lib/run-stop-display.ts` with one mapping helper for:

- `max_turns`
- `max_session_turns`
- `timeout`
- `loop_detected`
- `no_progress`
- `cancelled`

Update `ChatView.tsx` to:

- treat `stopped` as a separate visual state
- show user-friendly primary copy
- keep raw detail in expandable or secondary text when available

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --dir apps/runtime test -- --run src/components/__tests__/ChatView.run-guardrails.test.tsx
pnpm --dir apps/runtime test -- --run src/components/__tests__/ChatView.session-resilience.test.tsx
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/types.ts apps/runtime/src/lib/run-stop-display.ts apps/runtime/src/components/__tests__/ChatView.run-guardrails.test.tsx apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx
git commit -m "feat(runtime): render run stop states separately from errors"
```

### Task 6: Add ProgressGuard For Repeated Tool Calls And No-Progress Runs

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/run_guard.rs`
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Test: `apps/runtime/src-tauri/src/agent/run_guard.rs`
- Test: `apps/runtime/src-tauri/src/agent/executor.rs`

**Step 1: Write the failing test**

Add guard tests such as:

```rust
#[test]
fn repeated_identical_tool_calls_trigger_loop_detected_stop() {
    let policy = RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat);
    let history = vec![
        ProgressFingerprint::tool("browser_click", "same-input"),
        ProgressFingerprint::tool("browser_click", "same-input"),
        ProgressFingerprint::tool("browser_click", "same-input"),
        ProgressFingerprint::tool("browser_click", "same-input"),
        ProgressFingerprint::tool("browser_click", "same-input"),
        ProgressFingerprint::tool("browser_click", "same-input"),
    ];
    let evaluation = ProgressGuard::evaluate(&policy, &history);
    assert_eq!(evaluation.stop_reason.unwrap().kind, RunStopReasonKind::LoopDetected);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml repeated_identical_tool_calls_trigger_loop_detected_stop -- --nocapture
```

Expected: FAIL because `ProgressGuard` does not exist yet.

**Step 3: Write minimal implementation**

Implement `ProgressGuard` in `run_guard.rs` with:

- repeated identical tool detector
- same-output no-progress detector
- ping-pong detector placeholder or first-pass implementation

Wire it into `executor.rs` so the guard runs once per turn before the next model call.

For P0/P1 boundary:

- implement generic repeated-tool and no-progress logic now
- keep browser page-signature logic behind a follow-up task if it adds too much scope

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml repeated_identical_tool_calls_trigger_loop_detected_stop -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/run_guard.rs apps/runtime/src-tauri/src/agent/executor.rs
git commit -m "feat(runtime): add progress guard for looping runs"
```

### Task 7: Add Browser-Heavy Progress Fingerprints And Stage Hints

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_route_execution.rs`
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Create: `apps/runtime/src-tauri/src/agent/browser_progress.rs`
- Test: `apps/runtime/src-tauri/src/agent/browser_progress.rs`
- Test: `apps/runtime/src/components/__tests__/ChatView.run-guardrails.test.tsx`

**Step 1: Write the failing test**

Add a test that proves unchanged page signatures produce a no-progress stop summary:

```rust
#[test]
fn browser_progress_marks_unchanged_page_signature_as_no_progress() {
    let snapshot_a = BrowserProgressSnapshot::new("https://x.com", "发布", "hash-1", "facts-1");
    let snapshot_b = BrowserProgressSnapshot::new("https://x.com", "发布", "hash-1", "facts-1");
    assert!(snapshot_b.is_same_state_as(&snapshot_a));
}
```

Add a UI test that a stopped browser-heavy run can render `最后完成步骤`.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml browser_progress_marks_unchanged_page_signature_as_no_progress -- --nocapture
```

Expected: FAIL because browser progress snapshots and stage hints are not implemented.

**Step 3: Write minimal implementation**

Create `browser_progress.rs` with:

- `BrowserProgressSnapshot`
- helpers for hashing page facts and interactive elements
- stage hint extraction helpers such as:
  - `cover_filled`
  - `title_filled`
  - `body_segment_count`

Wire these summaries into:

- `executor.rs` progress snapshots
- `chat_route_execution.rs` or route event payloads
- `ChatView.tsx` stop summary display

Keep this stage-hint layer lightweight. Do not build a full workflow engine in this task.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml browser_progress_marks_unchanged_page_signature_as_no_progress -- --nocapture
pnpm --dir apps/runtime test -- --run src/components/__tests__/ChatView.run-guardrails.test.tsx
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/browser_progress.rs apps/runtime/src-tauri/src/agent/executor.rs apps/runtime/src-tauri/src/commands/chat_route_execution.rs apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView.run-guardrails.test.tsx
git commit -m "feat(runtime): add browser-heavy progress fingerprints"
```

### Task 8: Final Verification

**Files:**
- Verify only

**Step 1: Run backend verification**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib
```

Expected: PASS

**Step 2: Run frontend verification**

Run:

```bash
pnpm --dir apps/runtime test -- --run src/components/__tests__/ChatView.run-guardrails.test.tsx src/components/__tests__/ChatView.session-resilience.test.tsx
```

Expected: PASS

**Step 3: Run final type and build checks**

Run:

```bash
pnpm --dir apps/runtime exec tsc --noEmit
cargo check --manifest-path apps/runtime/src-tauri/Cargo.toml --lib
```

Expected: PASS

**Step 4: Commit**

```bash
git add -A
git commit -m "feat(runtime): add layered agent run guardrails"
```

Plan complete and saved to `docs/plans/2026-03-16-agent-run-guardrails-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
