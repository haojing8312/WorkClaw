# Rust DB Split Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Split `apps/runtime/src-tauri/src/db.rs` into a small bootstrap shell plus focused schema, migration, and seed modules without changing current SQLite behavior.

**Architecture:** Keep `db.rs` responsible for pool creation, PRAGMA setup, and bootstrap order. Move `CREATE TABLE` and `CREATE INDEX` logic into `db/schema.rs`, historical `ALTER TABLE` and compatibility helpers into `db/migrations.rs`, and builtin-skill/default-data startup sync into `db/seed.rs`.

**Tech Stack:** Rust, sqlx, SQLite, Tauri, in-memory SQLite tests

---

## Guardrails

- Preserve current database path resolution and PRAGMA settings.
- Preserve table names, index names, default values, and startup ordering.
- Keep all seed operations idempotent.
- Do not change schema behavior while moving statements between files.
- Add or preserve legacy-schema coverage when touching migration logic.

## Task 1: Extract seed work into `db/seed.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/db/seed.rs`
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Test: `apps/runtime/src-tauri/src/db.rs`

**Step 1: Move builtin skill sync helpers**

- Move:
  - `build_builtin_manifest_json`
  - `sync_builtin_skills`
- Move the startup default app-setting inserts into a `seed_runtime_defaults` style function.
- Keep builtin team template seeding in the same seed entrypoint.

**Step 2: Keep startup behavior unchanged**

- `db.rs` should still call the same seed work after schema and migration setup.
- Preserve `INSERT OR IGNORE` semantics and builtin skill upsert behavior.

**Step 3: Verify**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib sync_builtin_skills_upserts_manifest_and_source_type -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib sync_builtin_skills_is_idempotent -- --nocapture`

Expected:
- PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/db/seed.rs
git commit -m "refactor(runtime): extract db seed bootstrap"
```

## Task 2: Extract legacy compatibility into `db/migrations.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/db/migrations.rs`
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Test: `apps/runtime/src-tauri/src/db.rs`

**Step 1: Move `ALTER TABLE` and schema-repair helpers**

- Move the historical `ALTER TABLE ... ADD COLUMN ...` chain into a dedicated migration entrypoint.
- Move `ensure_im_thread_sessions_channel_column` into `migrations.rs`.
- Group related compatibility steps with short comments by table family rather than by chronological sprawl.

**Step 2: Keep migration entrypoint idempotent**

- Preserve the existing `let _ = sqlx::query(...).execute(...).await;` style where migration attempts are intentionally tolerant.
- Do not remove legacy-shape checks such as `pragma_table_info`.

**Step 3: Verify**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib ensure_im_thread_sessions_channel_column_migrates_legacy_schema -- --nocapture`
- `pnpm test:rust-fast`

Expected:
- PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/db/migrations.rs
git commit -m "refactor(runtime): extract db legacy migrations"
```

## Task 3: Extract current schema creation into `db/schema.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/db/schema.rs`
- Modify: `apps/runtime/src-tauri/src/db.rs`

**Step 1: Move `CREATE TABLE` and `CREATE INDEX` blocks**

- Group the schema into a few cohesive helper functions, for example:
  - runtime/session tables
  - approvals and routing tables
  - IM integration tables
  - employee/group tables
  - cache/catalog tables
- Keep helper names descriptive; avoid one helper per table unless the table is unusually complex.

**Step 2: Keep current-shape creation separate from migration logic**

- `schema.rs` should only express the current desired shape.
- Do not move `ALTER TABLE` steps into `schema.rs`.

**Step 3: Verify**

Run:
- `cargo check --manifest-path apps/runtime/src-tauri/Cargo.toml -q`
- `pnpm test:rust-fast`

Expected:
- PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/db/schema.rs
git commit -m "refactor(runtime): extract db schema bootstrap"
```

## Task 4: Thin the root bootstrap shell

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Test: `apps/runtime/src-tauri/src/db.rs`

**Step 1: Reduce `init_db` to bootstrap orchestration**

- Keep:
  - app-data-dir resolution
  - DB URL construction
  - pool creation
  - PRAGMA setup
  - explicit ordered calls into `schema`, `migrations`, and `seed`
- Remove any remaining large inline schema or migration blocks from the root file.

**Step 2: Keep root tests only if they still belong there**

- If tests naturally belong to `seed.rs` or `migrations.rs`, move them after the bootstrap shell is stable.
- If moving them would create churn without value, keep them temporarily and defer relocation.

**Step 3: Verify**

Run:
- `cargo check --manifest-path apps/runtime/src-tauri/Cargo.toml -q`
- `pnpm test:rust-fast`

Expected:
- PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/db
git commit -m "refactor(runtime): thin db bootstrap shell"
```

## Task 5: Record migration governance in local docs if needed

**Files:**
- Modify: `apps/runtime/src-tauri/AGENTS.md` if the new `db/` landing zones need local Rust runtime guidance
- Modify: `docs/plans/2026-03-23-rust-large-file-backlog.md` when `db.rs` moves below the split threshold

**Step 1: Update governance wording only if the split changes the local default landing zones**

- Add a short note that:
  - new tables go to `db/schema.rs`
  - new columns and legacy upgrades go to `db/migrations.rs`
  - repeatable default data goes to `db/seed.rs`

**Step 2: Verify**

Run:
- no runtime command required if this step is docs-only

**Step 3: Commit**

```bash
git add apps/runtime/src-tauri/AGENTS.md docs/plans/2026-03-23-rust-large-file-backlog.md
git commit -m "docs(runtime): record db migration governance"
```

Plan complete and saved to `docs/plans/2026-03-24-rust-db-split-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

**Which approach?**
