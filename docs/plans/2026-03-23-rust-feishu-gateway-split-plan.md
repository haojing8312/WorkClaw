# Rust Feishu Gateway Split Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Turn `apps/runtime/src-tauri/src/commands/feishu_gateway.rs` into the next formal Rust command-splitting template by extracting payload parsing, gate policy, pairing, outbound delivery, relay flow, and command implementations into focused child modules while preserving current Tauri and Feishu-facing behavior.

**Architecture:** Keep the current Tauri command interface and sibling-call surface stable. The root file should end as a thin shell that owns macro-visible command wrappers, small compatibility glue, and public re-exports. Child modules should own protocol parsing, gate policy, pairing, outbound, relay, and settings/persistence concerns.

**Tech Stack:** Rust, Tauri commands, sqlx, SQLite, reqwest, WorkClaw runtime tests

---

## Status

This implementation plan has now been executed through the main structural split.

Completed:

- `types.rs`
- `payload_parser.rs`
- `repo.rs`
- `gate_service.rs`
- `pairing_service.rs`
- `approval_service.rs`
- `outbound_service.rs`
- `relay_service.rs`
- `ingress_service.rs`
- `settings_service.rs`
- `planning_service.rs`
- `tauri_commands.rs`
- `tests.rs`

Current outcome:

- Root [feishu_gateway.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/feishu_gateway.rs) is now `399` lines
- Tauri-visible commands remain at the root as thin wrappers
- Runtime behavior lives in focused child modules
- Internal module tests were moved out of the root file into `feishu_gateway/tests.rs`

Verification last confirmed during this split:

- `cargo check --manifest-path apps/runtime/src-tauri/Cargo.toml`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_feishu_gateway -- --nocapture`
- `pnpm test:rust-fast`

All passed.

---

## Task 1: Create the Feishu gateway module skeleton

Status: completed

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/types.rs`
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/payload_parser.rs`
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/repo.rs` or `settings_repo.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`

**Step 1: Add module declarations**

- Add local module declarations under `feishu_gateway.rs`
- Re-export only the pieces the root file and tests need
- Keep the root file behavior unchanged at this stage

**Step 2: Compile with placeholder exports**

Run: `cargo check -p runtime`
Expected: PASS

**Step 3: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway.rs apps/runtime/src-tauri/src/commands/feishu_gateway
git commit -m "refactor(runtime): add feishu gateway split skeleton"
```

## Task 2: Extract types and payload parsing first

Status: completed

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway/types.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway/payload_parser.rs`
- Test: `apps/runtime/src-tauri/tests/test_feishu_gateway.rs`

**Step 1: Move DTOs and protocol structs**

- Move gateway DTOs, parsed payload enums, websocket status structs, pairing record structs, and protocol envelope structs into `types.rs`
- Re-export from the root file so external imports do not break

**Step 2: Move payload parsing helpers**

- Move:
  - `parse_feishu_payload`
  - mention extraction helpers
  - message text cleanup helpers
  - signature calculation helpers if they fit naturally here

**Step 3: Verify with focused tests**

Run focused gateway tests for:
- challenge parsing
- event parsing
- signature calculation or validation helpers

Expected: PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway.rs apps/runtime/src-tauri/src/commands/feishu_gateway/types.rs apps/runtime/src-tauri/src/commands/feishu_gateway/payload_parser.rs apps/runtime/src-tauri/tests/test_feishu_gateway.rs
git commit -m "refactor(runtime): extract feishu payload parser"
```

## Task 3: Extract settings and persistence helpers

Status: completed

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway/repo.rs`
- Test: focused gateway settings tests or existing credential tests

**Step 1: Move app setting and credential helpers**

- Move:
  - `get_app_setting`
  - `set_app_setting`
  - `resolve_feishu_sidecar_base_url`
  - `resolve_feishu_app_credentials`
  - default account resolution helpers if their primary concern is persisted settings/state

**Step 2: Keep root exports stable**

- Preserve the current function names available to callers
- Do not change sidecar base URL defaults or credential fallback order

**Step 3: Verify**

Run:
- focused credential/settings tests from `test_feishu_gateway.rs`
- `pnpm test:rust-fast`

Expected: PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway.rs apps/runtime/src-tauri/src/commands/feishu_gateway/repo.rs apps/runtime/src-tauri/tests/test_feishu_gateway.rs
git commit -m "refactor(runtime): extract feishu settings helpers"
```

## Task 4: Extract inbound gate policy and pairing flow

Status: completed

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/gate_service.rs`
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/pairing_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`
- Test: `apps/runtime/src-tauri/tests/test_feishu_gateway.rs`

**Step 1: Move gate policy logic**

- Move account/group gate config shaping and allow/reject decisions into `gate_service.rs`
- Preserve sender allowlist and mention-required behavior

**Step 2: Move pairing lifecycle**

- Move pairing-code helpers, request creation, approval/deny, and request resolution into `pairing_service.rs`
- Preserve current request text formatting and account-resolution semantics

**Step 3: Verify**

Run focused tests for:
- gate behavior
- pairing account resolution
- pairing approval/deny flow

Expected: PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway.rs apps/runtime/src-tauri/src/commands/feishu_gateway/gate_service.rs apps/runtime/src-tauri/src/commands/feishu_gateway/pairing_service.rs apps/runtime/src-tauri/tests/test_feishu_gateway.rs
git commit -m "refactor(runtime): split feishu gate and pairing flow"
```

## Task 5: Extract outbound delivery and approval orchestration

Status: completed

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/outbound_service.rs`
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/approval_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`
- Test: outbound and approval-related gateway tests

**Step 1: Move outbound delivery helpers**

- Move:
  - sidecar send helpers
  - official runtime outbound hook helpers
  - outbound route target builders
  - outbound message formatting helpers tied to delivery flow

**Step 2: Move approval orchestration**

- Move approval command parsing, approval request/resolution text builders, and approval notification logic

**Step 3: Verify**

Run focused tests for:
- outbound request shaping
- approval command parsing and notification behavior

Expected: PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway.rs apps/runtime/src-tauri/src/commands/feishu_gateway/outbound_service.rs apps/runtime/src-tauri/src/commands/feishu_gateway/approval_service.rs apps/runtime/src-tauri/tests/test_feishu_gateway.rs
git commit -m "refactor(runtime): split feishu outbound and approval flow"
```

## Task 6: Extract websocket and relay orchestration

Status: completed

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/relay_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`
- Test: websocket/relay-related gateway tests and any targeted runtime relay coverage

**Step 1: Move ws status and relay state logic**

- Move:
  - relay status shaping
  - ws event sanitizing and role resolution helpers
  - sync core loop
  - relay start/stop/status helpers
  - long connection start/stop/status helpers if they cluster naturally with relay behavior

**Step 2: Preserve event-processing order**

- Keep the same sequence of:
  - inbound gate evaluation
  - pairing pending handling
  - `process_im_event`
  - approval command handling
  - route decision lookup
  - employee session bridge

**Step 3: Verify**

Run:
- focused relay/ws tests if available
- `pnpm test:rust-fast`

Expected: PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway.rs apps/runtime/src-tauri/src/commands/feishu_gateway/relay_service.rs apps/runtime/src-tauri/tests/test_feishu_gateway.rs
git commit -m "refactor(runtime): extract feishu relay orchestration"
```

## Task 7: Thin the root command shell

Status: completed with one important implementation note

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/feishu_gateway/tauri_commands.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`

**Step 1: Move command implementation bodies**

- Move concrete command implementation bodies into `tauri_commands.rs`
- Keep root `#[tauri::command]` wrappers because Tauri macro visibility does still require them in practice

**Step 2: Review public surface**

- Ensure sibling imports still work
- Ensure the root file remains the visible shell instead of becoming a second service layer

**Step 3: Run final verification**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_feishu_gateway -- --nocapture
pnpm test:rust-fast
```

Expected: PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/feishu_gateway.rs apps/runtime/src-tauri/src/commands/feishu_gateway/tauri_commands.rs apps/runtime/src-tauri/tests/test_feishu_gateway.rs
git commit -m "refactor(runtime): thin feishu gateway commands"
```

## Remaining Follow-Up

- Decide whether to move more of the `feishu_gateway/tests.rs` coverage into dedicated integration tests over time
- Optionally add a per-module test split if `tests.rs` grows too large
- Treat `feishu_gateway` as the current Rust reference template alongside `employee_agents`, rather than continuing to force more structural churn just to reduce line count further

## Success Conditions

- `feishu_gateway.rs` becomes materially smaller
- no single replacement child file becomes the new giant dumping ground
- parsing, gate, pairing, outbound, and relay logic each have a clear home
- existing Tauri commands and payload contracts remain stable
- focused Feishu gateway tests and `pnpm test:rust-fast` remain green
