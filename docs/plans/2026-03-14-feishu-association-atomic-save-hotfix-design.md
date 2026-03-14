# Feishu Association Atomic Save Hotfix Design

## Context

The Feishu employee association refactor is now on `main`, but two post-merge issues remain:

1. Saving Feishu reception is not atomic. The UI updates `enabled_scopes` first and then mutates IM routing bindings with separate calls. If a later routing call fails, employee scope and routing state can drift apart.
2. Employee Feishu status in the list only checks `enabled_scopes.includes("feishu")`. Historical records that still have valid Feishu bindings but no matching scope are shown as "未关联飞书接待".

## Goal

Close the data consistency gap for Feishu reception saves and make Feishu status compatible with legacy binding-only data.

## Recommended Approach

Move the save flow behind a dedicated backend command in `employee_agents.rs`, and keep the frontend focused on form state.

### Why this approach

- The change spans both employee persistence and IM routing persistence, so the backend is the right place to guarantee consistency.
- `employee_agents.rs` already owns employee save side effects such as Feishu relay reconciliation.
- The frontend becomes simpler and easier to test because it no longer orchestrates multi-step binding updates itself.

## Design

### 1. Atomic save command

Add a new command/helper pair:

- helper: `save_feishu_employee_association_with_pool(...)`
- tauri command: `save_feishu_employee_association(...)`

The helper will:

- load the current employee row
- compute the next `enabled_scopes`
- update the employee row inside the same database transaction
- remove the employee's old Feishu bindings
- remove conflicting default or scoped Feishu bindings from other employees
- insert the replacement Feishu binding when reception stays enabled
- commit only if the whole sequence succeeds

The tauri command wrapper will preserve the existing employee-save side effects:

- reconcile Feishu employee connections
- restart or refresh the Feishu event relay

### 2. Legacy-compatible status resolution

Update `EmployeeHubView` status logic so an employee is treated as receiving Feishu when either of these is true:

- `enabled_scopes` contains `feishu`
- the employee has at least one Feishu routing binding

This keeps the list view and the employee detail section consistent during migration or older local database states.

### 3. Frontend save simplification

`EmployeeHubView` will stop calling:

- `onSaveEmployee`
- `list_im_routing_bindings`
- `delete_im_routing_binding`
- `upsert_im_routing_binding`

for the Feishu association flow.

Instead it will invoke the new backend command once, then refresh the employee list and bindings snapshot for display.

## Files Expected To Change

- `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- `apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx`
- `apps/runtime/src/types.ts`
- `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- `apps/runtime/src-tauri/src/lib.rs`
- `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`

## Test Strategy

### Frontend

- status stays non-gray when legacy Feishu bindings exist without `enabled_scopes`
- Feishu save path uses the new backend command instead of the old multi-call flow

### Backend

- saving a default Feishu receiver replaces the previous default inside one transaction
- saving a scoped Feishu receiver replaces conflicting scoped bindings inside one transaction
- failing the save does not persist partial employee scope changes

## Non-Goals

- redesigning the employee-side Feishu form
- changing WeCom flow
- introducing multi-connection Feishu selection in this hotfix
