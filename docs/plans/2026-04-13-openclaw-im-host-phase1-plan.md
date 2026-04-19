# OpenClaw IM Host Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Feishu reply orchestration out of the frontend and into a Tauri host-managed path so WorkClaw stops truncating or privately replaying final Feishu replies.

**Architecture:** Keep inbound Feishu routing and plugin runtime management in place, but introduce a backend-owned reply planning layer with chunk planning and delivery tracing. Frontend Feishu fallback logic is removed or feature-flagged off, and outbound sends become logical-reply aware rather than a thin single-message helper.

**Tech Stack:** Rust (`sqlx`, Tauri command layer), TypeScript/React frontend, Node-based plugin host, Vitest, existing WorkClaw diagnostics and OpenClaw vendor references

---

## File Structure

### Existing files to modify

- `apps/runtime/src-tauri/src/commands/feishu_gateway/outbound_service.rs`
  Current single-message outbound helper. Phase 1 should make this consume a reply plan and centralized chunking logic.
- `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs`
  Current plugin runtime transport. Phase 1 should add logical reply trace semantics and better partial-failure observability.
- `apps/runtime/src/scenes/useImBridgeIntegration.ts`
  Current frontend Feishu fallback bridge. Phase 1 should remove Feishu final-reply polling/sending responsibility from here.
- `apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx`
  Current frontend bridge tests. Phase 1 should retire frontend-owned Feishu fallback assumptions and replace them with narrow frontend responsibilities.

### New files to create

- `apps/runtime/src-tauri/src/commands/openclaw_plugins/im_host_contract.rs`
  Shared host-side IM reply models for logical reply planning and lifecycle result transport.
- `apps/runtime/src-tauri/src/commands/feishu_gateway/chunk_planner.rs`
  Centralized text chunk planner for Feishu outbound limits.
- `apps/runtime/src-tauri/src/commands/feishu_gateway/delivery_trace.rs`
  Reply delivery trace models and helpers.
- `apps/runtime/src-tauri/src/commands/feishu_gateway/reply_host_service.rs`
  Backend-owned service that turns runtime outputs into reply plans and drives outbound delivery.

### Tests to add or update

- `apps/runtime/src-tauri/src/commands/feishu_gateway/outbound_service.rs`
  Add Rust unit tests near the module if the file already follows colocated tests.
- `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs`
  Add or expand runtime-service tests to cover reply trace state transitions.
- `apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx`
  Keep only UI/session bookkeeping behavior for Feishu; remove final-reply fallback ownership tests.

## Task 1: Add Host Contract Types

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/im_host_contract.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins/mod.rs` or the nearest re-export module for command-local helpers
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins/im_host_contract.rs`

- [ ] **Step 1: Create the failing compile target by declaring the new module and importing it from the local command tree**

Add the new module export in the nearest `mod.rs`/command aggregator before implementing the file.

Run: `cargo test -p runtime openclaw_plugins --lib`
Expected: FAIL with module/file not found or unresolved import errors for `im_host_contract`

- [ ] **Step 2: Create `im_host_contract.rs` with the minimum shared types**

Add models for:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImReplyChunkPlan {
    pub index: usize,
    pub text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImReplyDeliveryPlan {
    pub logical_reply_id: String,
    pub session_id: String,
    pub channel: String,
    pub thread_id: String,
    pub chunks: Vec<ImReplyChunkPlan>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum ImReplyDeliveryState {
    Completed,
    Failed,
    FailedPartial,
}
```

Keep the first pass small. Do not introduce channel-specific behavior here.

- [ ] **Step 3: Add a narrow unit test for serde round-trip and state equality**

Add a simple test like:

```rust
#[test]
fn delivery_state_round_trips() {
    let value = ImReplyDeliveryState::FailedPartial;
    let json = serde_json::to_string(&value).expect("serialize state");
    let parsed: ImReplyDeliveryState = serde_json::from_str(&json).expect("deserialize state");
    assert_eq!(parsed, value);
}
```

- [ ] **Step 4: Run the focused test**

Run: `cargo test -p runtime delivery_state_round_trips --lib`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins/im_host_contract.rs apps/runtime/src-tauri/src/commands/openclaw_plugins
git commit -m "feat: add im host contract types"
```

## Task 2: Introduce a Central Feishu Chunk Planner

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/chunk_planner.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`
- Test: `apps/runtime/src-tauri/src/commands/feishu_gateway/chunk_planner.rs`

- [ ] **Step 1: Write the failing chunk-integrity test**

Add a unit test that proves a long string is split into multiple chunks and reconstructs exactly:

```rust
#[test]
fn feishu_chunk_planner_preserves_full_text() {
    let text = "你好，世界。".repeat(500);
    let chunks = plan_feishu_text_chunks(&text, 1800);
    assert!(chunks.len() > 1);
    let rebuilt = chunks.iter().map(|c| c.text.as_str()).collect::<String>();
    assert_eq!(rebuilt, text);
}
```

- [ ] **Step 2: Run the test to confirm it fails**

Run: `cargo test -p runtime feishu_chunk_planner_preserves_full_text --lib`
Expected: FAIL because `plan_feishu_text_chunks` does not exist

- [ ] **Step 3: Implement the minimal planner**

Implement a pure helper that:

- trims only leading/trailing whitespace once at the caller boundary, not per chunk
- splits on character boundaries, not byte indices
- emits `ImReplyChunkPlan { index, text }`

Keep the helper deterministic and channel-agnostic except for the supplied limit.

- [ ] **Step 4: Add a second test for exact-limit and under-limit behavior**

Add:

```rust
#[test]
fn feishu_chunk_planner_keeps_short_text_in_one_chunk() {
    let text = "短消息";
    let chunks = plan_feishu_text_chunks(text, 1800);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].text, text);
}
```

- [ ] **Step 5: Run the focused tests**

Run: `cargo test -p runtime feishu_chunk_planner --lib`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway/chunk_planner.rs apps/runtime/src-tauri/src/commands/feishu_gateway.rs
git commit -m "feat: add feishu chunk planner"
```

## Task 3: Add Delivery Trace Models

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/delivery_trace.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`
- Test: `apps/runtime/src-tauri/src/commands/feishu_gateway/delivery_trace.rs`

- [ ] **Step 1: Write the failing state transition test**

Add a test that starts a trace with 3 planned chunks, marks 1 delivered, then marks final state as `FailedPartial`.

```rust
#[test]
fn delivery_trace_tracks_partial_failure() {
    let mut trace = ReplyDeliveryTrace::new("reply-1", "session-1", "feishu", "chat-1", 3);
    trace.mark_chunk_delivered(0);
    trace.finish(ImReplyDeliveryState::FailedPartial);
    assert_eq!(trace.delivered_chunk_count, 1);
    assert_eq!(trace.final_state, Some(ImReplyDeliveryState::FailedPartial));
}
```

- [ ] **Step 2: Run the test to confirm it fails**

Run: `cargo test -p runtime delivery_trace_tracks_partial_failure --lib`
Expected: FAIL because `ReplyDeliveryTrace` does not exist

- [ ] **Step 3: Implement the trace model**

Add fields for:

- `logical_reply_id`
- `session_id`
- `channel`
- `target_thread_id`
- `planned_chunk_count`
- `delivered_chunk_count`
- `failed_chunk_indexes`
- `final_state`

Keep this as a plain state model first; persistence can come later.

- [ ] **Step 4: Run the focused test**

Run: `cargo test -p runtime delivery_trace_tracks_partial_failure --lib`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway/delivery_trace.rs apps/runtime/src-tauri/src/commands/feishu_gateway.rs
git commit -m "feat: add feishu delivery trace models"
```

## Task 4: Add Backend Reply Host Service

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/reply_host_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`
- Test: `apps/runtime/src-tauri/src/commands/feishu_gateway/reply_host_service.rs`

- [ ] **Step 1: Write the failing plan-construction test**

Add a test that builds a reply plan from a long Feishu final reply and expects:

- stable `logical_reply_id`
- `channel == "feishu"`
- more than one chunk for oversized input

```rust
#[test]
fn reply_host_service_builds_multichunk_feishu_plan() {
    let plan = build_feishu_reply_plan("reply-1", "session-1", "chat-1", &"A".repeat(4000));
    assert_eq!(plan.channel, "feishu");
    assert_eq!(plan.logical_reply_id, "reply-1");
    assert!(plan.chunks.len() > 1);
}
```

- [ ] **Step 2: Run the test to confirm it fails**

Run: `cargo test -p runtime reply_host_service_builds_multichunk_feishu_plan --lib`
Expected: FAIL because `build_feishu_reply_plan` does not exist

- [ ] **Step 3: Implement minimal plan construction**

Use the new chunk planner and host contract types. Keep scope to final-text Feishu reply planning only; do not add ask_user or approval branching in this phase.

- [ ] **Step 4: Add a test for empty/whitespace-only replies**

```rust
#[test]
fn reply_host_service_rejects_empty_final_reply() {
    let result = try_build_feishu_reply_plan("reply-1", "session-1", "chat-1", "   ");
    assert!(result.is_err());
}
```

- [ ] **Step 5: Run the focused tests**

Run: `cargo test -p runtime reply_host_service --lib`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway/reply_host_service.rs apps/runtime/src-tauri/src/commands/feishu_gateway.rs
git commit -m "feat: add backend feishu reply host service"
```

## Task 5: Refactor `outbound_service.rs` to Consume Reply Plans

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway/outbound_service.rs`
- Test: `apps/runtime/src-tauri/src/commands/feishu_gateway/outbound_service.rs`

- [ ] **Step 1: Write the failing test for multi-chunk outbound sends**

Use the existing outbound send hook for tests. Assert that a 4000-character reply results in multiple send requests instead of one truncated request.

```rust
#[tokio::test]
async fn send_feishu_reply_plan_sends_all_chunks() {
    // Arrange a test hook that records every outbound request target/text.
    // Build a plan with multiple chunks.
    // Execute the plan.
    // Assert recorded request count > 1 and rebuilt text equals input.
}
```

- [ ] **Step 2: Run the focused test to confirm it fails**

Run: `cargo test -p runtime send_feishu_reply_plan_sends_all_chunks --lib`
Expected: FAIL because outbound execution still sends a single text payload

- [ ] **Step 3: Implement minimal multi-chunk execution**

Change outbound execution so it:

- accepts a reply plan
- iterates chunk-by-chunk
- records success/failure into a delivery trace
- returns `FailedPartial` if at least one chunk delivered before a later failure

- [ ] **Step 4: Add a failing-then-partial test**

Add a hook-backed test where chunk 0 succeeds and chunk 1 fails.

Expected assertions:

- final state is `FailedPartial`
- delivered chunk count is `1`
- failed chunk indexes contains `1`

- [ ] **Step 5: Run the focused outbound tests**

Run: `cargo test -p runtime feishu_gateway::outbound_service --lib`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway/outbound_service.rs
git commit -m "feat: execute feishu reply plans with chunk tracing"
```

## Task 6: Add Logical Reply Trace Handling in Runtime Service

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs`
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs`

- [ ] **Step 1: Write the failing test for reply-level timeout classification**

Add a unit test that simulates a reply trace with one delivered chunk and a later timeout, expecting `FailedPartial`.

- [ ] **Step 2: Run the focused test to confirm it fails**

Run: `cargo test -p runtime reply_timeout_after_first_chunk_is_partial_failure --lib`
Expected: FAIL because timeout path only returns a plain transport error

- [ ] **Step 3: Implement minimal reply-level classification**

Without redesigning the full runtime protocol yet, make runtime service return enough metadata for the caller to classify:

- no chunks delivered -> `Failed`
- some chunks delivered + later timeout/disconnect -> `FailedPartial`

Do not try to solve Phase 2 lifecycle semantics here.

- [ ] **Step 4: Run the focused runtime-service test**

Run: `cargo test -p runtime reply_timeout_after_first_chunk_is_partial_failure --lib`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_service.rs
git commit -m "feat: classify partial outbound reply failures"
```

## Task 7: Remove Frontend Feishu Final-Reply Fallback Ownership

**Files:**
- Modify: `apps/runtime/src/scenes/useImBridgeIntegration.ts`
- Modify: `apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx`

- [ ] **Step 1: Write the failing frontend test for “Feishu final reply is not sent from the UI layer”**

Add or rewrite a test so that after a Feishu IM session completes, the frontend does **not** call `send_feishu_text_message` for the final assistant answer.

- [ ] **Step 2: Run the focused test to confirm it fails**

Run: `pnpm --dir apps/runtime test -- App.im-feishu-bridge.test.tsx`
Expected: FAIL because the current UI layer still sends Feishu final replies

- [ ] **Step 3: Remove or feature-flag the Feishu fallback poll/send path**

Delete or disable:

- `scheduleFallbackReplyPoll()` behavior for Feishu final answers
- `invokeFeishuSend()` usage for final-reply fallback
- Feishu retry logic tied to frontend final answer delivery
- any Feishu `.slice(0, 1800)` behavior

Keep IM session bookkeeping and ask_user UI behavior intact.

- [ ] **Step 4: Update frontend tests to the new ownership boundary**

Keep tests only for:

- dispatching to the desktop session
- ask_user routing to `answer_user_question`
- session bookkeeping

Remove tests that assert frontend-owned Feishu final delivery/retry/fallback behavior.

- [ ] **Step 5: Run the focused frontend suite**

Run: `pnpm --dir apps/runtime test -- App.im-feishu-bridge.test.tsx`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src/scenes/useImBridgeIntegration.ts apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx
git commit -m "refactor: remove frontend feishu final reply fallback"
```

## Task 8: Run Phase 1 Verification and Summarize Gaps

**Files:**
- Modify: `docs/architecture/openclaw-im-host/03-phase-1-plan.md`
- Modify: `docs/architecture/openclaw-im-host/appendix-b-risk-and-verification.md`

- [ ] **Step 1: Run the Rust-focused verification commands**

Run:

```bash
pnpm test:rust-fast
```

Expected: PASS, or a clear list of unrelated failures with the new IM host tests passing

- [ ] **Step 2: Run the frontend bridge verification**

Run:

```bash
pnpm --dir apps/runtime test -- App.im-feishu-bridge.test.tsx
```

Expected: PASS

- [ ] **Step 3: Update the architecture docs with implementation notes**

Add a short “Phase 1 completed state” note to the phase-1 doc and risk appendix:

- what shipped
- what remains for Phase 2
- which old frontend paths were retired

- [ ] **Step 4: Commit**

```bash
git add docs/architecture/openclaw-im-host/03-phase-1-plan.md docs/architecture/openclaw-im-host/appendix-b-risk-and-verification.md
git commit -m "docs: record phase 1 im host verification notes"
```

## Self-Review

### Spec coverage

- `00-context-and-goals.md`: covered by Tasks 1-7, which move reply ownership to the backend and introduce host-side planning/trace concepts
- `01-current-state-gap-analysis.md`: covered by Tasks 2, 5, 6, and 7, which remove frontend truncation/fallback and improve reply-level failure semantics
- `02-target-architecture.md`: partially covered by Tasks 1-7; Phase 1 intentionally does not yet complete official lifecycle alignment
- `03-phase-1-plan.md`: fully covered by Tasks 1-8
- `04-phase-2-plan.md`: intentionally deferred
- `05-phase-3-plan.md`: intentionally deferred

### Placeholder scan

- No `TODO`, `TBD`, or “implement later” placeholders remain in the task list
- Phase 2 and Phase 3 work are explicitly deferred rather than implied

### Type consistency

- `ImReplyChunkPlan`, `ImReplyDeliveryPlan`, `ImReplyDeliveryState`, and `ReplyDeliveryTrace` names are used consistently in later tasks
- The plan keeps Phase 1 scope on final-text Feishu replies only and does not prematurely introduce ask_user/approval code paths into the host contract implementation

## Execution Handoff

Plan complete and saved to `docs/plans/2026-04-13-openclaw-im-host-phase1-plan.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
