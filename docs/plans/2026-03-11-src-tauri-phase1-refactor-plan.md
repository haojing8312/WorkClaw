# src-tauri Phase 1 Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract high-value pure logic from `apps/runtime/src-tauri` into lightweight crates so Rust developers can validate more changes without compiling the full Tauri runtime crate.

**Architecture:** Create two small workspace crates for skill logic and policy logic, migrate pure functions/types into them, then slim `src-tauri` into a thinner shell/composition layer while preserving current command and runtime behavior.

**Tech Stack:** Rust workspace crates, Tauri app crate, focused cargo tests

---

### Task 1: Create `runtime-skill-core`

**Files:**
- Create: `packages/runtime-skill-core/Cargo.toml`
- Create: `packages/runtime-skill-core/src/lib.rs`
- Create: `packages/runtime-skill-core/tests/...`
- Modify: `apps/runtime/src-tauri/src/builtin_skills.rs`
- Modify: `apps/runtime/src-tauri/src/agent/skill_config.rs`
- Modify: Rust call sites that currently import those modules directly

**Step 1: Write failing tests**

Add lightweight tests in the new crate for:
- builtin skill registry access
- local skill template exposure
- skill frontmatter parsing

**Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path packages/runtime-skill-core/Cargo.toml -- --nocapture`

Expected: FAIL because code has not been moved yet.

**Step 3: Implement minimal extraction**

Move pure types/functions into the new crate and make `src-tauri` consume them.

**Step 4: Run tests to verify they pass**

Run:
- `cargo test --manifest-path packages/runtime-skill-core/Cargo.toml -- --nocapture`
- targeted app checks if needed

Expected: PASS

### Task 2: Create `runtime-policy`

**Files:**
- Create: `packages/runtime-policy/Cargo.toml`
- Create: `packages/runtime-policy/src/lib.rs`
- Create: `packages/runtime-policy/tests/...`
- Modify: `apps/runtime/src-tauri/src/agent/permissions.rs`
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Modify: `apps/runtime/src-tauri/src/commands/models.rs`

**Step 1: Write failing tests**

Add lightweight tests for:
- tool name normalization
- allowed tool narrowing
- extracted pure routing/policy helper behavior

**Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path packages/runtime-policy/Cargo.toml -- --nocapture`

Expected: FAIL because logic has not been extracted yet.

**Step 3: Implement minimal extraction**

Move pure policy helpers into the new crate and adapt app code to call them.

**Step 4: Run tests to verify they pass**

Run:
- `cargo test --manifest-path packages/runtime-policy/Cargo.toml -- --nocapture`
- targeted app checks if needed

Expected: PASS

### Task 3: Thin `src/lib.rs`

**Files:**
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Create or modify small internal helper modules if needed

**Step 1: Refactor setup into helper functions**

Split the large setup flow into named helpers for:
- runtime state setup
- sidecar bootstrap
- MCP restore
- Feishu relay bootstrap

**Step 2: Verify behavior-preserving refactor**

Run targeted app tests that touch startup-sensitive paths.

**Step 3: Keep scope tight**

Do not change runtime behavior or extract integration code into crates in this phase.

### Task 4: Final verification

**Files:**
- Modify: none

**Step 1: Run lightweight crate tests**

Run:
- `cargo test --manifest-path packages/runtime-skill-core/Cargo.toml -- --nocapture`
- `cargo test --manifest-path packages/runtime-policy/Cargo.toml -- --nocapture`
- `cargo test --manifest-path packages/builtin-skill-checks/Cargo.toml -- --nocapture`

**Step 2: Run targeted app tests**

Run only tests affected by the wiring changes, not the whole Tauri suite.

**Step 3: Inspect diff**

Ensure Phase 1 only moves pure logic and thins app composition; it should not become a broad runtime rewrite.
