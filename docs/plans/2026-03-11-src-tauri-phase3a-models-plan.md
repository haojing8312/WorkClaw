# src-tauri Phase 3A Models Application Layer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract `commands/models.rs` configuration and route-template use cases into a dedicated application-layer crate while keeping SQLx and runtime integrations inside `src-tauri`.

**Architecture:** Create `packages/runtime-models-app` with trait-based repositories and a `ModelsAppService`. Keep Tauri commands as thin adapters and implement repository/catalog adapters inside `apps/runtime/src-tauri`.

**Tech Stack:** Rust workspace crates, Tauri runtime crate, SQLx adapters, focused cargo tests

---

### Task 1: Create `runtime-models-app` crate skeleton

**Files:**
- Create: `packages/runtime-models-app/Cargo.toml`
- Create: `packages/runtime-models-app/src/lib.rs`
- Create: `packages/runtime-models-app/src/types.rs`
- Create: `packages/runtime-models-app/src/traits.rs`
- Create: `packages/runtime-models-app/src/service.rs`

**Step 1: Create the crate manifest**

Add `Cargo.toml` with only the dependencies required for:

- serde derives
- application-layer types
- `runtime-routing-core`

**Step 2: Create the public module surface**

Export from `src/lib.rs`:

- application DTOs
- traits
- service

**Step 3: Define stable application DTOs**

Add `src/types.rs` for:

- provider config input/output
- routing settings input/output
- provider plugin info
- capability routing policy output if needed by the service

Keep these independent from Tauri and SQLx row types.

**Step 4: Define trait boundaries**

Add `src/traits.rs` for:

- `ModelsConfigRepository`
- `ModelsReadRepository`
- `ProviderCatalog`

Only include methods needed for Phase 3A use cases.

**Step 5: Commit**

```bash
git add packages/runtime-models-app
git commit -m "refactor(rust): scaffold models application crate"
```

### Task 2: Write lightweight failing tests for application use cases

**Files:**
- Create: `packages/runtime-models-app/tests/routing_settings.rs`
- Create: `packages/runtime-models-app/tests/provider_configs.rs`
- Create: `packages/runtime-models-app/tests/route_templates.rs`
- Create: `packages/runtime-models-app/tests/provider_catalog.rs`

**Step 1: Write routing settings tests**

Cover:

- defaults when repository values are absent
- validation/clamping behavior if owned by the service

**Step 2: Write provider config tests**

Cover:

- save/list flow against a fake config repository
- stable output mapping

**Step 3: Write route template tests**

Cover:

- apply template against enabled providers
- clear failure when required provider keys are missing

**Step 4: Write provider catalog tests**

Cover:

- plugin metadata listing and mapping

**Step 5: Run tests to verify failure**

Run:

```bash
cargo test --manifest-path packages/runtime-models-app/Cargo.toml -- --nocapture
```

Expected: FAIL because service implementations do not exist yet.

### Task 3: Implement `ModelsAppService`

**Files:**
- Modify: `packages/runtime-models-app/src/service.rs`
- Modify: `packages/runtime-models-app/src/types.rs`
- Modify: `packages/runtime-models-app/src/traits.rs`

**Step 1: Implement routing settings use cases**

Add:

- `load_routing_settings`
- `save_routing_settings`

Keep defaults and validation centralized in the service if possible.

**Step 2: Implement provider config use cases**

Add:

- `save_provider_config`
- `list_provider_configs`

**Step 3: Implement provider plugin listing**

Add:

- `list_provider_plugins`

**Step 4: Implement capability route template application**

Add:

- `apply_capability_route_template`

Reuse `runtime-routing-core` template definitions instead of duplicating route logic.

**Step 5: Run tests to verify pass**

Run:

```bash
cargo test --manifest-path packages/runtime-models-app/Cargo.toml -- --nocapture
```

Expected: PASS

**Step 6: Commit**

```bash
git add packages/runtime-models-app
git commit -m "refactor(rust): add models application service"
```

### Task 4: Add `src-tauri` infrastructure adapters

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/models_repo.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Modify: `apps/runtime/src-tauri/Cargo.toml`

**Step 1: Add crate dependency**

Wire `runtime-models-app` into `apps/runtime/src-tauri/Cargo.toml`.

**Step 2: Implement repository adapter**

In `models_repo.rs`, implement the repository traits using:

- `SqlitePool`
- existing SQLx queries from `commands/models.rs`

Do not redesign storage in this phase. Move queries with minimal behavioral change.

**Step 3: Implement provider catalog adapter**

Expose provider plugin metadata from `ProviderRegistry` via `ProviderCatalog`.

**Step 4: Add focused adapter tests if practical**

Prefer narrow tests over broad integration suites.

### Task 5: Convert `commands/models.rs` into a thin command adapter

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/models.rs`
- Test: `apps/runtime/src-tauri/tests/test_models.rs`

**Step 1: Replace direct orchestration with service calls**

For the Phase 3A in-scope commands:

- extract `DbState` / `ProviderRegistry`
- construct adapters
- call `ModelsAppService`
- map results into command responses

**Step 2: Remove migrated use-case logic from the command file**

Delete or inline-move:

- routing settings orchestration
- provider config orchestration
- route template application orchestration
- provider plugin metadata orchestration

**Step 3: Keep out-of-scope logic in place**

Do not refactor:

- provider health probing
- route attempt analytics
- unrelated Tauri commands

**Step 4: Update tests**

Keep only minimal command-level tests for:

- command wiring
- response shape
- key happy paths

**Step 5: Run targeted tests**

Run:

```bash
cargo test --manifest-path packages/runtime-models-app/Cargo.toml -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_models -- --nocapture
```

Expected: PASS

**Step 6: Commit**

```bash
git add packages/runtime-models-app apps/runtime/src-tauri
git commit -m "refactor(rust): route models commands through app service"
```

### Task 6: Final verification and scope review

**Files:**
- Modify: none

**Step 1: Run lightweight Rust verification**

Run:

```bash
cargo test --manifest-path packages/runtime-models-app/Cargo.toml -- --nocapture
cargo test --manifest-path packages/runtime-routing-core/Cargo.toml -- --nocapture
cargo test --manifest-path packages/runtime-executor-core/Cargo.toml -- --nocapture
cargo test --manifest-path packages/runtime-policy/Cargo.toml -- --nocapture
cargo test --manifest-path packages/runtime-skill-core/Cargo.toml -- --nocapture
cargo test --manifest-path packages/builtin-skill-checks/Cargo.toml -- --nocapture
```

**Step 2: Run minimal app-level verification**

Run only the narrow `src-tauri` tests affected by the new wiring, such as:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_models -- --nocapture
```

**Step 3: Inspect scope**

Confirm Phase 3A did not drift into:

- chat orchestration extraction
- provider health redesign
- bootstrap refactors
- storage crate extraction

**Step 4: Commit**

```bash
git add docs/plans
git commit -m "docs: add phase 3a models application layer plan"
```
