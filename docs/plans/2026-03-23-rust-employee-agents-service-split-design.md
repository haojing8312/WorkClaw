# Rust Employee Agents Service Split Design

**Goal:** Turn [employee_agents.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents.rs) into the first formal Rust command-splitting template for WorkClaw by extracting command, service, repository, and sub-domain helper responsibilities into explicit module boundaries without changing the Tauri command contract.

## Why This Is First

`employee_agents.rs` is already in active split work and sits on a path that the team is likely to keep changing. That makes it the best candidate for the first formal large-file governance template:

- it is already far above the `800` split-design threshold
- it mixes command entrypoints, business rules, and persistence
- it has enough domain complexity to prove the pattern
- it is smaller and safer than starting with `openclaw_plugins.rs`

If this split ends with a genuinely thin root command file, the same pattern can be repeated for `feishu_gateway.rs`, `clawhub.rs`, and other giant command surfaces.

## Scope

- Extract employee profile listing, create/update, delete, default-employee switching, and skill binding persistence
- Extract group/team management logic out of the root command file
- Extract group-run entry, continuation, and execution helper logic out of the root command file
- Extract memory/export command logic out of the root command file
- Preserve current Tauri command names and response payloads
- Keep existing database schema unchanged in this phase
- Keep Feishu reconcile side effects intact at the command boundary

## Starting Problem

At the start of this effort, [employee_agents.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents.rs) was 2529 lines and mixed:

- Tauri command entrypoints
- employee validation and normalization
- employee CRUD business rules
- SQLite query logic
- skill binding persistence
- downstream side effects after mutation
- employee group and team management
- group-run entry and execution helpers
- memory export and cleanup commands

This makes the employee domain hard to test, hard to extend, and too expensive to modify safely.

## Final Design

### 1. Make the root file a true command layer

The final structure under `apps/runtime/src-tauri/src/commands/employee_agents/` is:

- the existing root file keeps Tauri entrypoints, thin command wrappers, public re-exports, and a small set of remaining command-level helpers
- `service.rs` owns shared employee-domain orchestration glue and bridges to narrower subservices
- `repo.rs` remains the storage-facing aggregation layer and re-exports narrower repo slices
- `profile_service.rs` owns employee profile CRUD orchestration
- `profile_repo.rs` owns employee profile persistence and skill binding persistence
- `feishu_service.rs` owns employee-to-Feishu association orchestration
- `routing_service.rs` owns employee-routing selection rules
- `session_service.rs` owns session ensure/link/bridge flows
- `group_run_service.rs` owns pause/resume/cancel state flow
- `group_run_snapshot_service.rs` owns group-run read/query snapshots
- `group_run_action_service.rs` owns retry/reassign/review action flows
- `group_management.rs` owns team/group create, clone, list, and delete flows
- `group_run_entry.rs` owns start/continue/run entry logic and execute-step helper composition
- `memory_commands.rs` owns memory/export command logic
- `tauri_commands.rs` owns non-memory Tauri command implementation bodies that the root file wraps

### 2. Keep commands thin

Tauri commands should only:

- receive command input
- call a focused child module or service
- preserve existing post-write side effects like Feishu reconciliation
- return the same shapes as today

This keeps the external contract stable while making the internal structure maintainable.

### 3. Move SQL into repository functions

The repository layer should own:

- employee list and skill binding persistence
- group/team persistence
- group-run persistence helpers already exposed through existing repo/service boundaries

This makes later Rust-side regression tests much easier to add.

## Delivered Boundary

### Covered by the split

- employee profile CRUD
- Feishu association orchestration
- employee routing and session bridging
- employee group/team management
- group-run entry, snapshot, state, and action flows
- employee memory export / clear / stats command logic
- Tauri command implementation bodies

### Still intentionally left in the root file

- public command wrappers required by Tauri macro visibility
- public re-export surface for sibling modules and callers
- a small set of command-level helpers such as IM route matching and route session key shaping

## Responsibility Split

### Command layer

- `list_agent_employees`
- `upsert_agent_employee`
- `delete_agent_employee`
- preserve Feishu reconcile call after upsert/delete
- wrap child command modules instead of accumulating business logic

### Service layer

- own validation, normalization, single-default behavior, multi-step orchestration, and child-service coordination
- decide when repository calls should happen and in what order
- prepare skill binding updates and cross-submodule handoffs
- do not own raw SQL or external protocol plumbing

### Repository layer

- own SQL, row mapping, persistence transactions, and dependent cleanup that is purely storage-related
- persist employee, skill binding, and group/team rows
- do not own business policy decisions

### Gateway layer

- still optional for this module
- Feishu protocol behavior remains outside this split; `feishu_service.rs` only handles employee-domain orchestration around those calls

## Risks

- Losing mutation side effects after moving logic out of commands
- Accidentally changing default employee semantics
- Accidentally changing ordering or payload shaping in employee list results
- Moving code into equally giant child files without actually clarifying boundaries

## Final Module Layout

- [employee_agents.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents.rs)
- [service.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/service.rs)
- [repo.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/repo.rs)
- [profile_service.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/profile_service.rs)
- [profile_repo.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/profile_repo.rs)
- [feishu_service.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/feishu_service.rs)
- [routing_service.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/routing_service.rs)
- [session_service.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/session_service.rs)
- [group_run_service.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/group_run_service.rs)
- [group_run_snapshot_service.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/group_run_snapshot_service.rs)
- [group_run_action_service.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/group_run_action_service.rs)
- [group_management.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/group_management.rs)
- [group_run_entry.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/group_run_entry.rs)
- [memory_commands.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/memory_commands.rs)
- [tauri_commands.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/tauri_commands.rs)

## Target End State

- [employee_agents.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents.rs) remains the visible Tauri entrypoint file and is now down to 799 lines
- the root file is below the `800` split-design threshold
- child modules are organized by responsibility rather than by generic helper dumping
- future employee features should attach to focused submodules instead of the root command file
- this module is now the sample implementation for future Rust command-file splits

## Success Criteria

- [employee_agents.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents.rs) is materially smaller
- employee profile CRUD no longer lives directly inside the command giant file
- group/team and group-run entry logic no longer live directly inside the root command file
- Tauri command names and payloads remain unchanged
- targeted Rust verification remains green through the split
- the same command/service/repo/child-module pattern can be reused for later employee-domain splits
