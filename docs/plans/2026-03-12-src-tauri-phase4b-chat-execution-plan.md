# src-tauri Phase 4B Chat Execution Preparation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Move the remaining chat execution-preparation orchestration from `apps/runtime/src-tauri/src/commands/chat.rs` into `packages/runtime-chat-app` while keeping runtime behavior unchanged.

**Architecture:** Extend `runtime-chat-app` with a dedicated execution-preparation service, narrow read-only traits, and stable request/result types. Keep Tauri command wiring, SQLx adapters, persistence, event emission, and executor invocation in `src-tauri`.

**Tech Stack:** Rust, Tauri, SQLx, workspace path crates, isolated Cargo verification scripts

---

### Task 1: Define the execution-preparation contract

**Files:**
- Modify: `packages/runtime-chat-app/src/lib.rs`
- Modify: `packages/runtime-chat-app/src/types.rs`
- Modify: `packages/runtime-chat-app/src/traits.rs`
- Modify: `packages/runtime-chat-app/src/service.rs`
- Test: `packages/runtime-chat-app/tests/execution_contract.rs`

**Step 1: Write the failing test**

Add a crate test that asserts:

- `ChatExecutionPreparationRequest` is constructible
- `PreparedChatExecution` exposes the normalized execution outputs needed by `chat.rs`
- `ChatExecutionPreparationService` is exported

**Step 2: Run test to verify it fails**

Run:

```bash
node scripts/run-cargo-isolated.mjs phase4b-contract-red -- test --manifest-path packages/runtime-chat-app/Cargo.toml execution_contract -- --nocapture
```

Expected: FAIL because the new contract types and exports do not exist yet.

**Step 3: Write minimal implementation**

- Add `ChatExecutionPreparationRequest`
- Add/extend `PreparedChatExecution`
- Add/export `ChatExecutionPreparationService`
- Keep names and fields tightly scoped to execution preparation only

**Step 4: Run test to verify it passes**

Run the same command.

**Step 5: Commit**

```bash
git add packages/runtime-chat-app
git commit -m "refactor(chat): define execution preparation contract"
```

### Task 2: Add execution-context aggregation to `runtime-chat-app`

**Files:**
- Modify: `packages/runtime-chat-app/src/types.rs`
- Modify: `packages/runtime-chat-app/src/traits.rs`
- Modify: `packages/runtime-chat-app/src/service.rs`
- Test: `packages/runtime-chat-app/tests/execution_context.rs`

**Step 1: Write the failing test**

Use fake repositories to assert the service normalizes:

- session mode
- employee/team execution metadata
- invocation/capability hints

**Step 2: Run test to verify it fails**

Run:

```bash
node scripts/run-cargo-isolated.mjs phase4b-context-red -- test --manifest-path packages/runtime-chat-app/Cargo.toml execution_context -- --nocapture
```

Expected: FAIL because execution-context aggregation is still incomplete.

**Step 3: Write minimal implementation**

- Add `ChatSessionContextRepository`
- Move only the execution-context normalization logic
- Keep behavior identical to current `chat.rs`

**Step 4: Run test to verify it passes**

Run the same command.

**Step 5: Commit**

```bash
git add packages/runtime-chat-app
git commit -m "refactor(chat): extract execution context aggregation"
```

### Task 3: Add execution guidance aggregation

**Files:**
- Modify: `packages/runtime-chat-app/src/traits.rs`
- Modify: `packages/runtime-chat-app/src/service.rs`
- Test: `packages/runtime-chat-app/tests/execution_guidance.rs`

**Step 1: Write the failing test**

Add tests that assert the app layer aggregates:

- default workdir hints
- imported MCP guidance
- route-related context hints needed before executor invocation

**Step 2: Run test to verify it fails**

Run:

```bash
node scripts/run-cargo-isolated.mjs phase4b-guidance-red -- test --manifest-path packages/runtime-chat-app/Cargo.toml execution_guidance -- --nocapture
```

Expected: FAIL because guidance aggregation is still handled outside the service.

**Step 3: Write minimal implementation**

- Extend `ChatSettingsRepository` with only the read methods needed
- Move guidance aggregation into the service
- Do not emit events or write state

**Step 4: Run test to verify it passes**

Run the same command.

**Step 5: Commit**

```bash
git add packages/runtime-chat-app
git commit -m "refactor(chat): extract execution guidance assembly"
```

### Task 4: Add route-decision assembly

**Files:**
- Modify: `packages/runtime-chat-app/src/traits.rs`
- Modify: `packages/runtime-chat-app/src/service.rs`
- Test: `packages/runtime-chat-app/tests/route_decisions.rs`

**Step 1: Write the failing test**

Add tests using fake route catalogs/settings to verify:

- route candidate inputs are normalized
- chat/capability policy inputs are aggregated
- fallback decisions are packaged for execution
- default/usable model resolution is integrated at the app layer boundary

**Step 2: Run test to verify it fails**

Run:

```bash
node scripts/run-cargo-isolated.mjs phase4b-routes-red -- test --manifest-path packages/runtime-chat-app/Cargo.toml route_decisions -- --nocapture
```

Expected: FAIL because route-decision assembly is still split between `chat.rs` and the crate.

**Step 3: Write minimal implementation**

- Add `ChatRouteCatalog`
- Move route-decision packaging logic into `ChatExecutionPreparationService`
- Keep retry/fallback semantics identical

**Step 4: Run test to verify it passes**

Run the same command.

**Step 5: Commit**

```bash
git add packages/runtime-chat-app
git commit -m "refactor(chat): extract route decision assembly"
```

### Task 5: Add `src-tauri` adapters for the new traits

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_repo.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Modify: `apps/runtime/src-tauri/Cargo.toml`
- Modify: `apps/runtime/src-tauri/Cargo.lock`
- Test: `apps/runtime/src-tauri/tests/test_chat_repo.rs`

**Step 1: Write the failing adapter test**

Extend `test_chat_repo.rs` to cover the new read paths needed for:

- execution context
- guidance settings
- route candidate inputs

**Step 2: Run test to verify it fails**

Run:

```bash
node scripts/run-cargo-isolated.mjs phase4b-chat-repo-red -- test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_chat_repo -- --nocapture
```

Expected: FAIL because the adapter does not yet implement the expanded trait surface.

**Step 3: Write minimal adapter implementation**

- Extend `chat_repo.rs` to implement the new trait methods
- Keep SQLx queries and runtime handles in `src-tauri`
- Do not move persistence writes

**Step 4: Run test to verify it passes**

Run the same command.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri
git commit -m "refactor(chat): extend tauri chat preparation adapters"
```

### Task 6: Route `chat.rs` through the execution-preparation service

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Test: `apps/runtime/src-tauri/tests/test_chat_commands.rs`

**Step 1: Write the failing command smoke test**

Add or extend a narrow test to prove that send-message preparation now goes through the app-layer contract without changing behavior.

**Step 2: Run test to verify it fails**

Run:

```bash
node scripts/run-cargo-isolated.mjs phase4b-chat-commands-red -- test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_chat_commands -- --nocapture
```

Expected: FAIL because `chat.rs` still performs some request assembly directly.

**Step 3: Write minimal wiring**

- Instantiate/use `ChatExecutionPreparationService`
- Replace direct execution-preparation assembly in `chat.rs`
- Keep executor invocation, event emission, and persistence behavior unchanged

**Step 4: Run test to verify it passes**

Run the same command.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri
git commit -m "refactor(chat): route execution preparation through app layer"
```

### Task 7: Run focused verification and cleanup

**Files:**
- Modify: `docs/plans/2026-03-12-src-tauri-phase4b-chat-execution-design.md`
- Modify: `docs/plans/2026-03-12-src-tauri-phase4b-chat-execution-plan.md`

**Step 1: Run lightweight crate verification**

Run:

```bash
node scripts/run-cargo-isolated.mjs phase4b-chat-crate-final -- test --manifest-path packages/runtime-chat-app/Cargo.toml -- --nocapture
```

Expected: PASS.

**Step 2: Run focused `src-tauri` verification**

Run:

```bash
node scripts/run-cargo-isolated.mjs phase4b-chat-repo-final -- test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_chat_repo -- --nocapture
node scripts/run-cargo-isolated.mjs phase4b-chat-commands-final -- test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_chat_commands -- --nocapture
```

Expected: PASS, or a clearly documented external blocker unrelated to this refactor.

**Step 3: Review remaining `chat.rs` scope**

Run:

```bash
git diff --stat
```

Confirm the remaining `chat.rs` responsibilities are primarily:

- command entrypoints
- runtime wiring
- executor handoff
- event emission
- persistence integration

**Step 4: Update docs if implementation diverged**

Record only meaningful deviations from the design so the next phase starts from accurate intent.

**Step 5: Commit**

```bash
git add docs/plans/2026-03-12-src-tauri-phase4b-chat-execution-design.md docs/plans/2026-03-12-src-tauri-phase4b-chat-execution-plan.md
git commit -m "docs: finalize phase 4b chat execution notes"
```
