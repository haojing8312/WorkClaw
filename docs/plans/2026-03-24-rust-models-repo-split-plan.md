# Rust Models Repo Split Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Split `apps/runtime/src-tauri/src/commands/models_repo.rs` into focused config and read repository modules while preserving the current public constructors and `models.rs` usage.

**Architecture:** Keep `PoolModelsRepository`, `RegistryProviderCatalog`, `BuiltinProviderCatalog`, `NullModelsRepository`, `NullProviderCatalog`, and `RuntimeProviderHealthProbe` visible from the root module. Move the trait implementations into `config_repo.rs` and `read_repo.rs` so the root file becomes a thinner adapter shell.

**Tech Stack:** Rust, sqlx, SQLite, WorkClaw runtime tests

---

### Task 1: Create the config repository module

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/models_repo/config_repo.rs`
- Modify: `apps/runtime/src-tauri/src/commands/models_repo.rs`

**Step 1: Move configuration trait impls**

- Move `ModelsConfigRepository for PoolModelsRepository`
- Move `ModelsConfigRepository for NullModelsRepository`

**Step 2: Keep behavior unchanged**

- Preserve the current SQL and error messages
- Preserve `PoolModelsRepository` constructor in the root file

**Step 3: Verify**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_models -- --nocapture
```

Expected: PASS

### Task 2: Create the read repository module

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/models_repo/read_repo.rs`
- Modify: `apps/runtime/src-tauri/src/commands/models_repo.rs`

**Step 1: Move read trait impls**

- Move `ModelsReadRepository for PoolModelsRepository`
- Move `ModelsReadRepository for NullModelsRepository`

**Step 2: Keep behavior unchanged**

- Preserve provider key, catalog cache, route logs, stats, and provider connection SQL

**Step 3: Verify**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_skill_route_settings -- --nocapture
```

Expected: PASS

### Task 3: Thin the root module

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/models_repo.rs`

**Step 1: Add the new submodules**

- Add `#[path = "models_repo/config_repo.rs"] mod config_repo;`
- Add `#[path = "models_repo/read_repo.rs"] mod read_repo;`

**Step 2: Re-export the moved traits from the root**

- Keep `models.rs` import surface stable

**Step 3: Verify the whole Rust fast path**

Run:

```bash
pnpm test:rust-fast
```

Expected: PASS
