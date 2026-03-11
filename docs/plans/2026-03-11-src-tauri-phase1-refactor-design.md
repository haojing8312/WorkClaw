# src-tauri Phase 1 Refactor Design

**Goal:** Reduce Rust test/compile cost in `apps/runtime/src-tauri` by extracting pure logic into lightweight crates while shrinking the Tauri app crate toward a shell/composition role.

## Why Phase 1 Exists

The current `apps/runtime/src-tauri` crate mixes:
- Tauri application wiring
- runtime bootstrap
- policy/permission rules
- skill asset/template logic
- config parsing
- integration-heavy code paths

This makes small Rust changes expensive to test because `cargo test` on the app crate pulls the full desktop/runtime dependency graph.

Phase 1 does not attempt full domain decomposition. It focuses on high-signal extractions that improve compile/test ergonomics with limited architectural risk.

## Phase 1 Principles

1. Extract pure logic first.
2. Leave Tauri commands and runtime setup mostly intact.
3. Prefer 2-3 small crates over one large "core" crate.
4. Each extraction must preserve behavior and gain fast tests.
5. Refactor toward cleaner boundaries, not theoretical perfection.

## Target Outcomes

- Skill-related pure logic no longer lives in the Tauri app crate.
- Permission/policy logic no longer lives in the Tauri app crate.
- `src/lib.rs` becomes thinner and more obviously a composition root.
- Future small Rust changes can often be tested without compiling Tauri.

## Proposed Crates

### 1. `packages/runtime-skill-core`

Purpose:
- builtin skill asset/template registry logic
- skill config parsing
- skill metadata and static asset references

Expected contents:
- logic currently in `apps/runtime/src-tauri/src/builtin_skills.rs`
- logic currently in `apps/runtime/src-tauri/src/agent/skill_config.rs`
- helper types/functions related to local skill template access

Not included yet:
- Tauri commands
- DB access
- installation workflows

### 2. `packages/runtime-policy`

Purpose:
- permission narrowing and allowlist logic
- policy/routing rule helpers that are pure functions

Expected contents:
- logic currently in `apps/runtime/src-tauri/src/agent/permissions.rs`
- selected pure rule helpers currently embedded in executor / models command code

Not included yet:
- command entrypoints
- stateful runtime coordination
- provider/network/database side effects

### 3. `src-tauri` shell cleanup

Purpose:
- reduce `src/lib.rs` to app setup plus calls into bootstrap helpers

Expected restructuring:
- split `run()` setup into focused internal functions
- keep behavior unchanged
- prepare a later Phase 2 extraction of bootstrap/integration code

## Migration Order

### Step 1: Extract skill core

Reason:
- highest compile/test payoff
- very low Tauri coupling
- already proven by the lightweight builtin skill check crate

### Step 2: Extract policy core

Reason:
- next-best pure logic cluster
- high test density
- makes command/executor layers thinner

### Step 3: Thin the Tauri composition root

Reason:
- improves maintainability without forcing large behavior changes
- creates stable cut points for future extraction

## Risks

### Risk: Hidden Tauri coupling in extracted files

Mitigation:
- move only pure logic first
- if a function needs Tauri state/handles, leave it in `src-tauri`

### Risk: Circular dependencies

Mitigation:
- new crates expose pure functions/types only
- `src-tauri` depends on them; they never depend back

### Risk: Large blast radius from command rewiring

Mitigation:
- do not rewrite command surface in Phase 1
- only replace internals behind existing interfaces

## Verification Strategy

- Each new crate gets its own focused `cargo test` path.
- Existing app behavior stays covered by current `src-tauri` integration tests.
- No Phase 1 step is complete unless at least one test moves from heavy app crate to lightweight crate.

## Definition of Done for Phase 1

- `runtime-skill-core` exists and is used by `src-tauri`
- `runtime-policy` exists and is used by `src-tauri`
- at least a meaningful subset of pure tests runs outside `apps/runtime/src-tauri`
- `src/lib.rs` is materially smaller and more compositional
