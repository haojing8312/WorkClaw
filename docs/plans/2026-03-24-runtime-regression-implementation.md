# Runtime Regression Repair Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore the frontend/runtime build and failing regression coverage after the recent refactor wave.

**Architecture:** Fix the smallest cross-cutting compile errors first, then restore risk-flow UI behavior, then repair the Feishu IM bridge state machine using failing tests as the harness. Keep contracts stable and avoid unrelated refactors while narrowing each change to the module that owns the behavior.

**Tech Stack:** React, TypeScript, Vitest, Playwright, Tauri runtime shell

---

### Task 1: Repair compile blockers

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/components/settings/desktop/DesktopSettingsSection.tsx`
- Modify: `apps/runtime/src/scenes/chat/useChatStreamController.ts`
- Test: `pnpm build:runtime`

**Step 1: Inspect the failing type sites**

Run: `rg -n "renderInstallCandidates|loadCapabilityRoutingPolicy|loadRouteTemplates|operation_permission_mode|pendingApprovalsRef.current" apps/runtime/src`

Expected: exact compile-failure locations and nearby owners.

**Step 2: Write or adjust the smallest failing test where behavior changed**

Use existing failing test files first if the compile issue is behavior-backed. If the issue is type-only, skip straight to minimal code repair.

**Step 3: Implement minimal type-safe fixes**

- Narrow `renderInstallCandidates` input to the actual callback contract.
- Restore/import routing loader helpers or remove dead calls if the feature is intentionally hidden.
- Normalize desktop permission mode to the strict union expected by the save API.
- Guard nullable approval refs before filtering.

**Step 4: Run targeted validation**

Run: `pnpm --dir apps/runtime test -- --run src/components/__tests__/SettingsView.risk-flow.test.tsx`

Expected: the touched risk-flow test file at least executes.

**Step 5: Run build**

Run: `pnpm build:runtime`

Expected: no TypeScript compile errors.

### Task 2: Restore ChatView approval/risk flow

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/scenes/chat/useChatStreamController.ts`
- Test: `apps/runtime/src/components/__tests__/ChatView.risk-flow.test.tsx`

**Step 1: Run the failing ChatView risk tests**

Run: `pnpm --dir apps/runtime test -- --run src/components/__tests__/ChatView.risk-flow.test.tsx`

Expected: failures for missing dialog and missing queued approval content.

**Step 2: Trace the approval data flow**

Inspect how pending approvals are created, stored, filtered, and rendered between `useChatStreamController` and `ChatView`.

**Step 3: Implement minimal fix**

Restore rendering conditions so critical approvals surface as a dialog and queued approvals advance correctly after remote resolution.

**Step 4: Re-run targeted test**

Run: `pnpm --dir apps/runtime test -- --run src/components/__tests__/ChatView.risk-flow.test.tsx`

Expected: green.

### Task 3: Align Settings desktop permission and translation expectations

**Files:**
- Modify: `apps/runtime/src/components/settings/desktop/DesktopSettingsSection.tsx`
- Modify: `apps/runtime/src/components/__tests__/SettingsView.risk-flow.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/SettingsView.translation-preferences.test.tsx`

**Step 1: Run the two failing Settings test files**

Run: `pnpm --dir apps/runtime test -- --run src/components/__tests__/SettingsView.risk-flow.test.tsx src/components/__tests__/SettingsView.translation-preferences.test.tsx`

Expected: the current button-label mismatch and translation model assertion mismatch.

**Step 2: Decide behavior vs. stale assertion**

Confirm whether the new copy and empty `translation_model_id` are intentional outputs of the current UI logic.

**Step 3: Implement minimal fix**

- If behavior is correct, update the tests.
- If behavior is wrong, restore prior save payload / confirmation behavior in production code.

**Step 4: Re-run targeted tests**

Run the same command from Step 1.

Expected: green.

### Task 4: Repair Feishu IM bridge regressions

**Files:**
- Modify: `apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx` only if assertions are stale
- Modify: the owning runtime IM bridge/controller files discovered during tracing
- Test: `apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx`

**Step 1: Run the failing Feishu bridge file in isolation**

Run: `pnpm --dir apps/runtime test -- --run src/__tests__/App.im-feishu-bridge.test.tsx`

Expected: 5 failing tests covering refresh, token filtering, delayed replies, closed-loop clarification, and retry limit.

**Step 2: Trace each failure to one shared root cause where possible**

Check recent refactor-induced changes to:

- session list refresh after IM dispatch
- stream-item/token filtering before Feishu delivery
- delayed assistant reply polling lifecycle
- retry bookkeeping and max-attempt termination

**Step 3: Implement the smallest shared fix**

Prefer one state-management or event-sequencing repair over five isolated patches.

**Step 4: Re-run isolated Feishu bridge test**

Run: `pnpm --dir apps/runtime test -- --run src/__tests__/App.im-feishu-bridge.test.tsx`

Expected: green.

### Task 5: Full verification

**Files:**
- Test only

**Step 1: Run runtime tests**

Run: `pnpm --dir apps/runtime test`

Expected: all tests pass.

**Step 2: Run desktop build**

Run: `pnpm build:runtime`

Expected: build succeeds.

**Step 3: If runtime-facing flows changed materially, re-run E2E**

Run: `pnpm test:e2e:runtime`

Expected: green.
