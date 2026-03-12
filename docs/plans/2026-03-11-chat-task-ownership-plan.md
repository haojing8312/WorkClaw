# Chat Task Ownership Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the right-side panel the only source of truth for current task state, and replace the top-of-chat task progress card with time-ordered key event cards only.

**Architecture:** Re-scope the existing task journey UI into two layers: a persistent right-side task console and message-stream event cards rendered only at meaningful milestones. Keep the current view-model pipeline, but split “running state” from “event milestones” so empty sessions and guided employee-creator sessions no longer render misleading progress UI.

**Tech Stack:** React, TypeScript, Vite, Vitest, Testing Library, Tauri runtime events

---

### Task 1: Lock the target behavior in tests

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.theme.test.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing tests**

Add tests that assert:

- no top “任务进度” card is rendered for empty sessions
- no top “任务进度” card is rendered for employee-assistant guided empty state
- right panel remains the place where current task status is shown
- key event card text appears only when a milestone model is provided

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL because the current view still renders top-level task journey UI for empty/default states.

**Step 3: Write minimal implementation**

Do not implement UI yet. Only update test fixtures and assertions until they clearly describe the target behavior.

**Step 4: Run test to verify it still fails for the right reason**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL with assertions tied to the obsolete top task card.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx apps/runtime/src/components/__tests__/ChatView.theme.test.tsx
git commit -m "test: define chat task ownership behavior"
```

### Task 2: Separate persistent task state from key event milestones

**Files:**
- Modify: `apps/runtime/src/components/chat-side-panel/view-model.ts`
- Test: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing test**

Add or extend tests around view-model behavior to cover:

- empty tool-call history produces no message-stream milestone
- empty employee-creator guided sessions resolve to a right-panel status like `等待收集需求`
- running tool calls still produce right-panel status without auto-generating a top summary card

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL because `buildTaskJourneyViewModel` still defaults to `处理中` and `completed`.

**Step 3: Write minimal implementation**

Refactor `view-model.ts` to introduce explicit concepts:

- persistent task status for side panel
- key milestone events for message stream
- empty-state detection with no fallback to `处理中`

Ensure no model returns contradictory default labels.

**Step 4: Run test to verify it passes**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS for the new empty-state and ownership assertions.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/chat-side-panel/view-model.ts apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx
git commit -m "refactor: split task status from journey milestones"
```

### Task 3: Remove the persistent top-of-chat task journey summary

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/chat-journey/TaskJourneySummary.tsx`
- Modify: `apps/runtime/src/components/chat-journey/TaskJourneyTimeline.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing test**

Add UI assertions that:

- the top-of-chat summary block is absent during normal empty/guided sessions
- the message list can still render milestone cards inline when such events exist

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL because `ChatView` still renders `TaskJourneySummary` before the message loop.

**Step 3: Write minimal implementation**

Update `ChatView.tsx` so that:

- the old persistent `TaskJourneySummary` block is removed from the message header area
- milestone cards, if retained, are rendered as time-ordered items in the message stream
- employee assistant entry state shows only the intent banner and guidance copy, not a task-progress card

Trim or repurpose `TaskJourneySummary.tsx` and `TaskJourneyTimeline.tsx` to match the new responsibilities, deleting dead UI if no longer needed.

**Step 4: Run test to verify it passes**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS and no top task-progress card remains.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/chat-journey/TaskJourneySummary.tsx apps/runtime/src/components/chat-journey/TaskJourneyTimeline.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx
git commit -m "refactor: remove persistent chat task journey summary"
```

### Task 4: Add explicit employee-assistant guided empty state

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/App.tsx`
- Test: `apps/runtime/src/__tests__/App.employee-creator-skill-flow.test.tsx`
- Test: `apps/runtime/src/__tests__/App.employee-assistant-update-flow.test.tsx`

**Step 1: Write the failing test**

Add tests that assert:

- employee-creator sessions show guidance copy before the first useful interaction
- no execution-state words like `处理中` or `已完成` appear on entry
- the starter prompt still triggers the intended assistant behavior without surfacing misleading global progress UI

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/__tests__/App.employee-creator-skill-flow.test.tsx apps/runtime/src/__tests__/App.employee-assistant-update-flow.test.tsx`

Expected: FAIL because the current entry experience still implies immediate execution state.

**Step 3: Write minimal implementation**

Adjust the entry-state rendering and, if needed, the initial starter prompt framing so the first screen communicates:

- “先问 1-2 个关键问题”
- “给出配置草案”
- “确认后再创建”

without any persistent task-progress framing.

**Step 4: Run test to verify it passes**

Run: `pnpm vitest run apps/runtime/src/__tests__/App.employee-creator-skill-flow.test.tsx apps/runtime/src/__tests__/App.employee-assistant-update-flow.test.tsx`

Expected: PASS with the new guided empty-state behavior.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/App.tsx apps/runtime/src/__tests__/App.employee-creator-skill-flow.test.tsx apps/runtime/src/__tests__/App.employee-assistant-update-flow.test.tsx
git commit -m "feat: add guided empty state for employee assistant"
```

### Task 5: Verify side-panel ownership and regression coverage

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing test**

Add regression coverage that:

- “当前任务” appears only in the side panel context
- milestone cards do not duplicate side-panel labels
- group/delegation panels still render independently of the right-panel task ownership changes

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL if any duplicated ownership text remains in the main stream.

**Step 3: Write minimal implementation**

Tighten label rendering and conditional display logic until ownership is unambiguous and delegation/group run UI remains intact.

**Step 4: Run test to verify it passes**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx apps/runtime/src/components/ChatView.tsx
git commit -m "test: enforce side panel as current task owner"
```

### Task 6: Run focused verification and document residual risks

**Files:**
- Modify: `docs/plans/2026-03-11-chat-task-ownership-design.md`
- Modify: `docs/plans/2026-03-11-chat-task-ownership-plan.md`

**Step 1: Run focused tests**

Run:

```bash
pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx apps/runtime/src/__tests__/App.employee-creator-skill-flow.test.tsx apps/runtime/src/__tests__/App.employee-assistant-update-flow.test.tsx
```

Expected: PASS.

**Step 2: Run broader chat regression if needed**

Run:

```bash
pnpm vitest run apps/runtime/src/components/__tests__/ChatView.theme.test.tsx
```

Expected: PASS.

**Step 3: Update docs if implementation diverged**

Record any small implementation deviations directly in the design and plan docs so the next engineer does not inherit stale intent.

**Step 4: Commit**

```bash
git add docs/plans/2026-03-11-chat-task-ownership-design.md docs/plans/2026-03-11-chat-task-ownership-plan.md
git commit -m "docs: finalize chat task ownership rollout notes"
```
