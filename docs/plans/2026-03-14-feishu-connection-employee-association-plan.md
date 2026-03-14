# Feishu Connection And Employee Association Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Split Feishu setup into a standalone connection flow and an employee-side association flow, while preserving the existing routing and multi-employee runtime behavior.

**Architecture:** Keep the existing Feishu adapter and routing runtime intact, then refactor the UI and state model so connection lifecycle lives under settings and employee assignment lives under employee detail. Reuse current routing persistence and bridge metadata wherever possible instead of introducing a new Feishu-specific backend layer.

**Tech Stack:** React 18, TypeScript, Tauri runtime, existing runtime sidecar channel adapter APIs, existing test stack under `apps/runtime/src/components/**/__tests__` and `apps/runtime/src/__tests__`

---

### Task 1: Confirm current Feishu UI and state touchpoints

**Files:**
- Inspect: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Inspect: `apps/runtime/src/components/employees/FeishuRoutingWizard.tsx`
- Inspect: `apps/runtime/src/components/__tests__/SettingsView.feishu.test.tsx`
- Inspect: `apps/runtime/src/components/__tests__/SettingsView.feishu-routing-wizard.test.tsx`
- Inspect: `apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx`
- Inspect: `docs/integrations/feishu-routing.md`

**Step 1: Read the existing employee-side Feishu UI**

Run: `Get-Content -Path 'apps/runtime/src/components/employees/EmployeeHubView.tsx' -TotalCount 260`
Expected: Identify where Feishu connection status and routing entry points are currently shown.

**Step 2: Read the current routing wizard implementation**

Run: `Get-Content -Path 'apps/runtime/src/components/employees/FeishuRoutingWizard.tsx' -TotalCount 320`
Expected: Understand current binding fields, save behavior, and simulation flow.

**Step 3: Read the Feishu settings tests**

Run: `Get-Content -Path 'apps/runtime/src/components/__tests__/SettingsView.feishu.test.tsx' -TotalCount 260`
Expected: See what settings-level behavior already exists and where the new standalone connection UX should land.

**Step 4: Capture findings in the plan branch notes**

Expected: You can name the concrete components and tests that must change before touching implementation.

**Step 5: Commit**

```bash
git add docs/plans/2026-03-14-feishu-connection-employee-association-plan.md docs/plans/2026-03-14-feishu-connection-employee-association-design.md
git commit -m "docs: add feishu connection and employee association plan"
```

### Task 2: Add failing tests for standalone Feishu connection entry

**Files:**
- Modify: `apps/runtime/src/components/__tests__/SettingsView.feishu.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/SettingsView.feishu-routing-wizard.test.tsx`

**Step 1: Write a failing test for a dedicated Feishu connection section in settings**

Add assertions covering:
- Feishu connection status card is rendered under settings
- Connection-level actions appear there
- Employee assignment is not edited on this screen

**Step 2: Run the focused test to verify it fails**

Run: `pnpm --dir apps/runtime test -- SettingsView.feishu.test.tsx`
Expected: FAIL because the current settings UI does not yet present the finalized standalone connection structure.

**Step 3: Write a failing test that employee routing is no longer presented as the primary connection setup path**

Run: `pnpm --dir apps/runtime test -- SettingsView.feishu-routing-wizard.test.tsx`
Expected: FAIL where old assumptions about direct routing-first entry are no longer correct.

**Step 4: Commit**

```bash
git add apps/runtime/src/components/__tests__/SettingsView.feishu.test.tsx apps/runtime/src/components/__tests__/SettingsView.feishu-routing-wizard.test.tsx
git commit -m "test: cover standalone feishu connection entry"
```

### Task 3: Add failing tests for employee-side Feishu association

**Files:**
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx`
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.thread-binding.test.tsx`
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx`

**Step 1: Add a test for selecting a Feishu connection on an employee**

Cover:
- Employee can enable Feishu reception
- Employee can pick one Feishu connection
- UI distinguishes default receiver vs scoped rules

**Step 2: Add a test for single default receiver enforcement**

Cover:
- One connection can only have one default receiving employee
- Saving a second default produces a visible validation error or replacement flow

**Step 3: Add a test for scoped group or thread rules**

Cover:
- Employee can handle only specific groups or sessions
- Scoped rules are shown as advanced configuration

**Step 4: Run the focused tests to verify they fail**

Run: `pnpm --dir apps/runtime test -- EmployeeHubView.feishu-connection-status.test.tsx EmployeeHubView.thread-binding.test.tsx EmployeeHubView.group-orchestrator.test.tsx`
Expected: FAIL because the employee page does not yet expose the finalized association model.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.thread-binding.test.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx
git commit -m "test: cover employee feishu association flow"
```

### Task 4: Refactor settings UI to host Feishu connection management

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/components/employees/FeishuRoutingWizard.tsx` or split reusable pieces into a new connection-focused component
- Create: `apps/runtime/src/components/settings/FeishuConnectionPanel.tsx`
- Update related types as needed in `apps/runtime/src/types.ts`

**Step 1: Implement the new connection panel**

Include:
- Status card
- Install / bind / authorize / verify actions
- Basic toggles
- Linked employee summary

**Step 2: Keep employee assignment out of the settings panel**

Expected behavior:
- Settings can show which employee is default
- Settings cannot edit full employee routing relationships directly

**Step 3: Wire panel into settings navigation**

Expected behavior:
- User can reach Feishu connection from settings without entering employee hub first

**Step 4: Run settings tests**

Run: `pnpm --dir apps/runtime test -- SettingsView.feishu.test.tsx SettingsView.feishu-routing-wizard.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/settings/FeishuConnectionPanel.tsx apps/runtime/src/components/employees/FeishuRoutingWizard.tsx apps/runtime/src/types.ts
git commit -m "feat: add standalone feishu connection management"
```

### Task 5: Refactor employee UI to own Feishu association

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Create: `apps/runtime/src/components/employees/EmployeeFeishuAssociationSection.tsx`
- Modify supporting employee hooks or state stores discovered in Task 1

**Step 1: Implement the employee-side association section**

Include:
- Enable Feishu reception toggle
- Feishu connection selector
- Default receiver option
- Scoped group or session rules under advanced configuration

**Step 2: Add default receiver validation**

Expected behavior:
- Only one employee per connection can be marked default
- Conflicts are surfaced with actionable guidance

**Step 3: Add scoped rule conflict handling**

Expected behavior:
- Overlapping group or session scopes are detected before save or clearly flagged after load

**Step 4: Run focused employee tests**

Run: `pnpm --dir apps/runtime test -- EmployeeHubView.feishu-connection-status.test.tsx EmployeeHubView.thread-binding.test.tsx EmployeeHubView.group-orchestrator.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/components/employees/EmployeeFeishuAssociationSection.tsx
git commit -m "feat: move feishu assignment into employee detail"
```

### Task 6: Adapt persistence and selectors without breaking runtime routing

**Files:**
- Modify: exact runtime state modules identified in Task 1
- Modify: exact Tauri command bridge or client state modules identified in Task 1
- Test: existing Feishu bridge and routing tests

**Step 1: Map new UI state onto existing binding persistence**

Expected behavior:
- Existing routing data still saves into current `im_routing_bindings` or equivalent store
- No parallel source of truth is introduced

**Step 2: Preserve current runtime dispatch behavior**

Expected behavior:
- Default employee fallback still routes correctly
- Scoped rules still win over defaults

**Step 3: Run routing and app bridge tests**

Run: `pnpm --dir apps/runtime test -- App.im-feishu-bridge.test.tsx`
Expected: PASS

**Step 4: Run any additional focused routing tests discovered in Task 1**

Expected: PASS with no regression in bridge or dispatch behavior.

**Step 5: Commit**

```bash
git add [state-and-runtime-files-from-task-1]
git commit -m "refactor: align feishu persistence with split connection flow"
```

### Task 7: Update docs and user guidance

**Files:**
- Modify: `docs/integrations/feishu-routing.md`
- Modify: `docs/integrations/feishu-im-bridge.md`
- Modify: `docs/user-manual/04-employee-hub.md`
- Optionally create: `docs/user-manual/feishu-connection.md`

**Step 1: Update integration docs**

Document:
- Settings owns Feishu connection
- Employee detail owns assignment
- Default receiver vs scoped rule semantics

**Step 2: Update user-facing manual**

Document:
- First-time setup flow
- How to choose a default employee
- How to assign a group to a specific employee

**Step 3: Run doc sanity review**

Expected: Terminology stays consistent across docs: `飞书连接`, `默认接待员工`, `指定处理范围`, `员工关联`.

**Step 4: Commit**

```bash
git add docs/integrations/feishu-routing.md docs/integrations/feishu-im-bridge.md docs/user-manual/04-employee-hub.md docs/user-manual/feishu-connection.md
git commit -m "docs: describe split feishu connection and employee assignment flow"
```

### Task 8: Final verification

**Files:**
- Verify the changed UI and test files from all previous tasks

**Step 1: Run the targeted runtime test suite**

Run: `pnpm --dir apps/runtime test -- SettingsView.feishu.test.tsx SettingsView.feishu-routing-wizard.test.tsx EmployeeHubView.feishu-connection-status.test.tsx EmployeeHubView.thread-binding.test.tsx EmployeeHubView.group-orchestrator.test.tsx App.im-feishu-bridge.test.tsx`
Expected: PASS

**Step 2: Run any broader app test command if the targeted suite passes quickly**

Run: `pnpm --dir apps/runtime test`
Expected: PASS, or known unrelated failures captured explicitly.

**Step 3: Review changed files**

Run: `git status --short`
Expected: Only intended files remain modified.

**Step 4: Commit**

```bash
git add .
git commit -m "feat: split feishu connection from employee assignment"
```
