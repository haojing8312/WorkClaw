# Expert Skill Direct Chat Launch Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the expert skill card `开始任务` action create a new session immediately and open chat directly, matching the employee direct-chat experience.

**Architecture:** Keep the existing experts page and chat page structure. Change the `App.tsx` orchestration so the expert-skill launch path mirrors the existing employee direct-launch flow: resolve the chosen skill, create a session with the skill name as the initial title, update optimistic local session state, refresh canonical sessions, and select the new session. Reuse the existing chat header workspace picker instead of adding a new launch surface.

**Tech Stack:** React 18 + TypeScript, Tauri invoke API, Vitest + Testing Library, Playwright E2E

---

### Task 1: Lock the new launch behavior in app routing tests

**Files:**
- Modify: `apps/runtime/src/__tests__/App.experts-routing.test.tsx`

**Step 1: Add a failing test for direct chat launch**

Add a test that:

- opens experts view
- clicks the expert skill card launch button
- expects `create_session` to be called
- expects the app to render chat instead of `new-session-landing`

Example expectation shape:

```ts
expect(invokeMock).toHaveBeenCalledWith(
  "create_session",
  expect.objectContaining({
    skillId: "local-test-skill",
    title: "Local Test Skill",
    sessionMode: "general",
  }),
);
expect(screen.queryByTestId("new-session-landing")).not.toBeInTheDocument();
```

**Step 2: Run the focused test to verify failure**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.experts-routing.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: FAIL because the current handler only switches view state and lands on the no-session landing page.

**Step 3: Commit the failing test**

```bash
git add apps/runtime/src/__tests__/App.experts-routing.test.tsx
git commit -m "test(runtime): define expert skill direct-chat launch behavior"
```

### Task 2: Add a focused session creation test for the skill launch path

**Files:**
- Modify: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`

**Step 1: Add a failing test for launch payload details**

Add a test that verifies expert-skill launch uses:

- resolved default work dir
- `title = skill.name`
- `sessionMode = "general"`
- selected skill id

Also assert the optimistic chat state reflects the new work dir.

**Step 2: Run the focused test to verify failure**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.session-create-flow.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: FAIL because the current expert-skill path does not create a session.

**Step 3: Commit the failing test**

```bash
git add apps/runtime/src/__tests__/App.session-create-flow.test.tsx
git commit -m "test(runtime): cover expert skill launch session payload"
```

### Task 3: Rework expert skill launch orchestration in `App.tsx`

**Files:**
- Modify: `apps/runtime/src/App.tsx`

**Step 1: Implement the minimal direct-launch flow**

Change `handleStartTaskWithSkill(skillId)` so it:

- finds the target skill from `skills`
- resolves the default model id
- clears launch errors
- sets `selectedSkillId`
- creates a session immediately
- passes `title: skill.name`
- passes `sessionMode: "general"`
- passes `workDir: await resolveSessionLaunchWorkDir()`
- writes an optimistic session entry
- reloads sessions for that skill
- selects the created session
- lands in chat

Use the existing `handleStartTaskWithEmployee(...)` flow as the structural reference, but do not set `employeeId`.

**Step 2: Keep failure behavior minimal and explicit**

On failure:

- log the error
- set `createSessionError("创建会话失败，请稍后重试")`
- leave the user on the experts page instead of navigating to the landing page

**Step 3: Run the focused tests**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.experts-routing.test.tsx src/__tests__/App.session-create-flow.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

**Step 4: Commit the implementation**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/__tests__/App.experts-routing.test.tsx apps/runtime/src/__tests__/App.session-create-flow.test.tsx
git commit -m "feat(runtime): launch expert skills directly into chat"
```

### Task 4: Surface launch errors in the experts page

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/components/experts/ExpertsView.tsx`
- Test: `apps/runtime/src/components/experts/__tests__/ExpertsView.test.tsx`

**Step 1: Add a failing UI test**

Add a test that renders `ExpertsView` with an error prop and expects a visible inline error message near the header or action area.

**Step 2: Run the focused component test to verify failure**

Run: `pnpm --dir apps/runtime exec vitest run src/components/experts/__tests__/ExpertsView.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: FAIL because `ExpertsView` has no error display yet.

**Step 3: Implement a small error surface**

Add an optional `launchError?: string | null` prop to `ExpertsView` and render a compact inline error banner when present.

Plumb `createSessionError` from `App.tsx` into `ExpertsView` only for the experts branch so launch failures are visible in-context.

**Step 4: Run the focused tests**

Run: `pnpm --dir apps/runtime exec vitest run src/components/experts/__tests__/ExpertsView.test.tsx src/__tests__/App.experts-routing.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

**Step 5: Commit the UI polish**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/components/experts/ExpertsView.tsx apps/runtime/src/components/experts/__tests__/ExpertsView.test.tsx
git commit -m "feat(runtime): show expert skill launch errors in place"
```

### Task 5: Update E2E navigation expectations

**Files:**
- Modify: `apps/runtime/e2e/smoke.navigation.spec.ts`

**Step 1: Change the regression test to the new expected behavior**

Replace the current expectation that expert-skill launch “returns to landing” with:

- chat view becomes visible
- the new session id or chat marker appears
- landing hero is not visible

**Step 2: Run the focused E2E spec**

Run: `pnpm --dir apps/runtime exec playwright test e2e/smoke.navigation.spec.ts --grep "start a task from experts skill card"`
Expected: FAIL before the assertion update is satisfied end-to-end, PASS after implementation is complete.

**Step 3: Commit the E2E update**

```bash
git add apps/runtime/e2e/smoke.navigation.spec.ts
git commit -m "test(runtime): update experts launch e2e expectation"
```

### Task 6: Verify chat title and workspace behavior remain correct

**Files:**
- Modify if needed: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`
- Verify existing coverage: `apps/runtime/src/components/__tests__/ChatView.theme.test.tsx`
- Verify existing coverage: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Add one assertion if missing**

Ensure at least one test asserts the launched session title in local state uses the skill name and the chat work dir reflects the resolved default directory.

**Step 2: Run the targeted verification suite**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.session-create-flow.test.tsx src/components/__tests__/ChatView.theme.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

**Step 3: Commit if test files changed**

```bash
git add apps/runtime/src/__tests__/App.session-create-flow.test.tsx apps/runtime/src/components/__tests__/ChatView.theme.test.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx
git commit -m "test(runtime): verify title and workspace behavior for expert launches"
```

### Task 7: Update release documentation for the new flow

**Files:**
- Modify: `docs/user-manual/12-release-test-cases.md`

**Step 1: Adjust the manual test wording**

For `TC-P1-006 从技能入口启动任务`, explicitly state:

- click a skill card launch button
- app opens the corresponding chat session directly
- the session title shows the skill name
- workspace can be adjusted from the chat header

**Step 2: Review the doc diff**

Run: `git diff -- docs/user-manual/12-release-test-cases.md`
Expected: only wording changes related to the direct-launch flow.

**Step 3: Commit the doc update**

```bash
git add docs/user-manual/12-release-test-cases.md
git commit -m "docs: update expert skill launch acceptance flow"
```

### Task 8: Final verification

**Files:**
- No new files unless fixes are needed

**Step 1: Run the full targeted frontend suite**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.experts-routing.test.tsx src/__tests__/App.session-create-flow.test.tsx src/components/experts/__tests__/ExpertsView.test.tsx src/components/__tests__/ChatView.theme.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

**Step 2: Run the focused E2E spec**

Run: `pnpm --dir apps/runtime exec playwright test e2e/smoke.navigation.spec.ts --grep "start a task from experts skill card"`
Expected: PASS

**Step 3: Run a production build smoke check**

Run: `pnpm --dir apps/runtime build`
Expected: PASS

**Step 4: Commit any final cleanups**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/components/experts/ExpertsView.tsx apps/runtime/src/__tests__/App.experts-routing.test.tsx apps/runtime/src/__tests__/App.session-create-flow.test.tsx apps/runtime/e2e/smoke.navigation.spec.ts docs/user-manual/12-release-test-cases.md
git commit -m "chore: finalize expert skill direct-chat launch rollout"
```
