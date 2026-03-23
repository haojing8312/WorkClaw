# Rust Chat Runtime IO Split Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Split `apps/runtime/src-tauri/src/commands/chat_runtime_io.rs` into focused child modules by extracting workspace skill DTOs and workspace skill projection/sync helpers first, while keeping the root API stable for current callers.

**Architecture:** Keep the root chat runtime helper module as a compatibility facade and move cohesive helper clusters into child modules under `chat_runtime_io/`. The first batch only extracts the workspace-skill cluster because it is mostly helper code, has clear boundaries, and is already covered by existing focused tests. Later batches can split the remaining session title, runtime input, memory, message reconstruction, and run-event clusters.

**Tech Stack:** Rust, Tauri command helpers, sqlx, SQLite, skillpack_rs, walkdir, WorkClaw runtime tests

---

## Outcome

The planned split batches have now been implemented.

- created `types.rs` for workspace skill DTOs
- created `workspace_skills.rs` for workspace skill projection, prompt rendering, file-tree sync, and skill loading helpers
- created `session_titles.rs` for session title normalization and first-message title update behavior
- created `runtime_support.rs` for memory bucket, work-dir, and tool-name helpers
- created `runtime_inputs.rs` for session runtime inputs, installed skill source loading, session history loading, and default search-provider lookup
- created `message_reconstruction.rs` for message history reconstruction and assistant-content shaping
- created `runtime_events.rs` for session message persistence, route attempt logging, run-event journaling, and team-entry pre-execution orchestration
- re-exported the moved names from the root module so downstream callers keep using the same surface
- kept the root module as a compatibility facade
- migrated the remaining root-local tests into the matching child modules
- reduced the root file from 2117 lines to 40 lines
- verified the moved files with `rustfmt --check`
- attempted focused `cargo test` runs, but full crate compilation is currently blocked by an unrelated duplicate-import error in in-progress `feishu_gateway.rs`

## Task 1: Add workspace skill child modules

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/chat_runtime_io/types.rs`
- Create: `apps/runtime/src-tauri/src/commands/chat_runtime_io/workspace_skills.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_runtime_io.rs`

**Step 1: Add the new module declarations**

- Declare the new child modules from the root file
- Re-export the DTOs and skill helpers that current callers already use
- Keep the root public names stable

**Step 2: Move the DTOs and skill projection helpers**

- Move the workspace skill DTOs into `types.rs`
- Move the workspace skill projection, prompt rendering, file-tree sync, and skill loading helpers into `workspace_skills.rs`
- Keep helper behavior unchanged

**Step 3: Verify the moved surface still compiles**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib normalize_workspace_skill_dir_name_uses_skill_id_and_sanitizes -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib prepare_workspace_skills_prompt_syncs_and_returns_available_skills_block -- --nocapture
pnpm test:rust-fast
```

Expected: PASS

## Task 2: Update the split docs if needed

**Files:**
- Modify: `docs/plans/2026-03-23-rust-chat-runtime-io-split-design.md`
- Modify: `docs/plans/2026-03-23-rust-chat-runtime-io-split-plan.md`

**Step 1: Record the actual extracted surface**

- Update the docs if the extracted module names differ from the initial phase design
- Mark the first batch as completed once verification passes

**Step 2: Leave follow-on clusters for a later pass**

- Keep session title, runtime input, memory, message reconstruction, and run-event helpers listed as next steps
- Do not expand the first batch beyond the workspace skill cluster

## Follow-on Work

1. Re-run focused `runtime` crate tests once the parallel `feishu_gateway` compile errors are cleared.
2. Decide whether any child module now deserves a second-level split once real feature work resumes.
