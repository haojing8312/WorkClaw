# Rust Employee Manage Tool Split Design

**Goal:** Turn `apps/runtime/src-tauri/src/agent/tools/employee_manage.rs` into a thinner tool shell by extracting request parsing, employee lookup, action orchestration, and response shaping into focused child modules without changing the `employee_manage` tool contract.

## Why This Split

`employee_manage.rs` is already a clear maintenance hotspot:

- it mixes tool schema definition with multiple use cases
- it contains reusable parsing and normalization helpers
- it performs employee lookup and matching logic inline
- it owns create, update, and profile application orchestration
- it also carries a full test module that bloats the root file

This makes the tool hard to extend without growing another giant helper file inside the root.

## Recommended Split Shape

Create a small module cluster under `apps/runtime/src-tauri/src/agent/tools/employee_manage/`:

- `support.rs` for request parsing, normalization, matching, and default-path helpers
- `actions.rs` for `list_skills`, `list_employees`, `create_employee`, `update_employee`, and `apply_profile`
- `schema.rs` for the `Tool` input schema and static action metadata
- `tests.rs` for the current test module once the root is thin enough

Keep `employee_manage.rs` as the visible `Tool` entrypoint that wires those child modules together.

## Responsibility Split

### Tool entrypoint

- expose `EmployeeManageTool`
- keep the `Tool` trait implementation
- forward each action to a focused child function
- keep the block-on bridge only if it must remain for the synchronous tool API

### Support layer

- parse string arrays, optional strings, optional bools, and profile answers
- normalize employee IDs
- derive default work directories
- dedupe and match skill IDs
- resolve employee records by `employee_db_id` or `employee_id`

### Action layer

- list skills
- list employees
- create employee
- update employee
- apply agent profile

### Schema layer

- own the input schema JSON
- own the action enum and field descriptions
- keep schema metadata in one place instead of crowding the root file

### Test layer

- move the current test module out of the root file once the extraction is stable
- keep behavior assertions focused on create/update/profile application and schema visibility

## Smallest Safe Path

The first low-risk implementation batch should be:

1. extract `support.rs`
2. extract `actions.rs`
3. keep the `Tool` trait implementation in the root file as a thin shell
4. leave tests in place for the first pass unless they become the main source of root bloat

This gives us the biggest clarity gain without immediately breaking the tool contract across many files.

## Risks

- Changing employee matching behavior when extracting `resolve_employee`
- Accidentally changing the default `primary_skill_id` or `enabled_scopes` behavior
- Creating a new giant action file instead of a cohesive split
- Moving tests too early before the root shell is stable

## Success Criteria

- the root file becomes a thin tool shell
- parsing and lookup helpers no longer crowd the root
- create/update/profile behavior remains unchanged
- targeted tests still pass
