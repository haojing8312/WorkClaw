# Close-Code Alignment Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a low-conflict adapter layer that separates team topology, delegation policy, and agent definition so self-dispatch stops leaking through legacy employee group execution paths.

**Architecture:** Keep the existing WorkClaw task runtime as the kernel, add `agent_catalog` and `employee_runtime_adapter` as compatibility layers, and leave legacy `team_rules.rs` in place as a wrapper. The first implementation wave is intentionally additive so it can coexist with parallel changes in the core agent area.

**Tech Stack:** Rust, Tauri backend, existing employee group runtime, Rust unit tests

---

### Task 1: Add failing tests for topology and delegation semantics

**Files:**
- Create: `apps/runtime/src-tauri/src/employee_runtime_adapter/mod.rs`
- Create: `apps/runtime/src-tauri/src/employee_runtime_adapter/team_topology.rs`
- Create: `apps/runtime/src-tauri/src/employee_runtime_adapter/delegation_policy.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents/team_rules.rs`

- [ ] **Step 1: Write the failing topology tests**

Add unit tests that assert:

```rust
#[test]
fn normalize_member_employee_ids_dedupes_and_trims() {}

#[test]
fn resolve_team_topology_handles_single_member_team() {}

#[test]
fn resolve_team_topology_prefers_entry_then_review_rules() {}
```

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test -p runtime team_topology -- --nocapture`
Expected: FAIL because the new module/functions do not exist yet

- [ ] **Step 3: Write the failing delegation-policy tests**

Add unit tests that assert:

```rust
#[test]
fn resolve_delegation_targets_returns_dispatch_to_other_for_valid_pair() {}

#[test]
fn resolve_delegation_targets_downgrades_self_dispatch_to_self_execute() {}

#[test]
fn resolve_delegation_targets_does_not_fallback_to_self_dispatch_for_multi_member_team() {}
```

- [ ] **Step 4: Run tests to verify RED**

Run: `cargo test -p runtime delegation_policy -- --nocapture`
Expected: FAIL because the new policy types/functions do not exist yet

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/employee_runtime_adapter apps/runtime/src-tauri/src/commands/employee_agents/team_rules.rs
git commit -m "test: add employee runtime adapter red tests"
```

### Task 2: Add agent catalog primitives

**Files:**
- Create: `apps/runtime/src-tauri/src/agent_catalog/mod.rs`
- Create: `apps/runtime/src-tauri/src/agent_catalog/agent_definition.rs`
- Create: `apps/runtime/src-tauri/src/agent_catalog/agent_workspace.rs`
- Create: `apps/runtime/src-tauri/src/agent_catalog/agent_permissions.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing agent-catalog tests**

Add tests that assert:

```rust
#[test]
fn normalize_agent_id_trims_and_lowercases() {}

#[test]
fn default_memory_scope_matches_role_kind() {}

#[test]
fn derive_capabilities_for_role_disables_executor_delegation() {}
```

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test -p runtime agent_catalog -- --nocapture`
Expected: FAIL because the catalog module and helpers do not exist yet

- [ ] **Step 3: Implement the minimal catalog**

Add:

```rust
pub enum AgentRoleKind { Coordinator, Planner, Reviewer, Executor, General }
pub enum AgentMemoryScope { Session, Employee, Shared }
pub struct AgentCapabilityFlags { pub can_delegate: bool, pub can_spawn_subagents: bool, pub can_review: bool, pub background_capable: bool }
pub struct AgentDefinition { /* normalized runtime-facing employee definition */ }
```

and helpers for ID normalization, default memory scope, workspace/persona normalization, default tool set, and role-based capabilities.

- [ ] **Step 4: Run tests to verify GREEN**

Run: `cargo test -p runtime agent_catalog -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent_catalog apps/runtime/src-tauri/src/lib.rs
git commit -m "feat: add agent catalog primitives"
```

### Task 3: Implement team topology and delegation policy

**Files:**
- Modify: `apps/runtime/src-tauri/src/employee_runtime_adapter/mod.rs`
- Modify: `apps/runtime/src-tauri/src/employee_runtime_adapter/team_topology.rs`
- Modify: `apps/runtime/src-tauri/src/employee_runtime_adapter/delegation_policy.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`

- [ ] **Step 1: Implement normalized rule and topology structures**

Add:

```rust
pub struct TeamTopology {
    pub coordinator_employee_id: String,
    pub planner_employee_id: String,
    pub reviewer_employee_id: Option<String>,
    pub executor_employee_ids: Vec<String>,
}

pub struct NormalizedTeamRule {
    pub from_employee_id: String,
    pub to_employee_id: String,
    pub relation_type: String,
    pub phase_scope: String,
}
```

and the normalization/topology resolution helpers.

- [ ] **Step 2: Implement delegation policy with explicit self-execute**

Add:

```rust
pub enum DelegationKind { DispatchToOther, SelfExecute }

pub struct DelegationTarget {
    pub source_employee_id: String,
    pub target_employee_id: String,
    pub kind: DelegationKind,
}

pub struct DelegationPolicy {
    pub targets: Vec<DelegationTarget>,
    pub has_dispatch_targets: bool,
}
```

with logic that never emits `DispatchToOther` when `source == target`.

- [ ] **Step 3: Run targeted tests**

Run: `cargo test -p runtime employee_runtime_adapter -- --nocapture`
Expected: PASS

- [ ] **Step 4: Refactor only after GREEN**

Keep helpers small and deduped, but do not expand scope into execution service changes in this task.

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/employee_runtime_adapter apps/runtime/src-tauri/src/lib.rs
git commit -m "feat: add team topology and delegation policy"
```

### Task 4: Convert legacy team_rules into compatibility wrappers

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents/team_rules.rs`

- [ ] **Step 1: Add the failing legacy compatibility test**

Add a test that asserts:

```rust
#[test]
fn legacy_select_group_execute_dispatch_targets_filters_out_self_dispatch() {}
```

- [ ] **Step 2: Run test to verify RED**

Run: `cargo test -p runtime legacy_select_group_execute_dispatch_targets_filters_out_self_dispatch -- --nocapture`
Expected: FAIL because legacy wrapper still returns raw self-dispatch-compatible targets

- [ ] **Step 3: Implement the wrapper conversion**

Convert `EmployeeGroupRule` to `NormalizedTeamRule`, delegate to the new adapter modules, and only map `DispatchToOther` results back into `GroupRunExecuteTarget`.

- [ ] **Step 4: Run focused wrapper tests**

Run: `cargo test -p runtime team_rules -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/employee_agents/team_rules.rs
git commit -m "refactor: route legacy team rules through adapter layer"
```

### Task 5: Verify repo-level affected surface

**Files:**
- Modify: none expected beyond code/test files above

- [ ] **Step 1: Run targeted Rust fast path**

Run: `pnpm test:rust-fast`
Expected: PASS

- [ ] **Step 2: Record any residual gaps**

Note explicitly if `group_run_execution_service.rs` integration and `employee_step_profile.rs` alignment are not yet changed in this phase.

- [ ] **Step 3: Commit verification-safe state**

```bash
git add docs/superpowers/specs/2026-04-11-close-code-aligned-agent-runtime-design.md docs/superpowers/plans/2026-04-11-close-code-alignment-phase1.md apps/runtime/src-tauri/src/agent_catalog apps/runtime/src-tauri/src/employee_runtime_adapter apps/runtime/src-tauri/src/commands/employee_agents/team_rules.rs apps/runtime/src-tauri/src/lib.rs
git commit -m "feat: add phase-1 close-code alignment adapter layer"
```

## Self-Review

- Spec coverage:
  - topology split: covered in Task 1 and Task 3
  - self-dispatch prevention: covered in Task 1, Task 3, and Task 4
  - agent definition foundation: covered in Task 2
  - low-conflict compatibility path: preserved by Task 4 and Task 5
- Placeholder scan:
  - no `TODO` or `TBD` placeholders remain
- Type consistency:
  - `TeamTopology`, `NormalizedTeamRule`, `DelegationPolicy`, and `AgentDefinition` names are consistent across tasks
