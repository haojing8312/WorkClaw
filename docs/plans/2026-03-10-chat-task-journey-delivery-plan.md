# Chat Task Journey And Delivery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rework the chat UI so users can understand task progress and final deliverables directly from the main conversation flow.

**Architecture:** Add a session-level task journey view model on top of existing messages and tool calls, then render a timeline and delivery summary in the main chat area. Keep the side panel as a secondary inspection surface and reuse `ToolIsland` for drill-down details.

**Tech Stack:** React 18, TypeScript, Vitest, Testing Library, existing Tauri event/invoke integration

---

### Task 1: Extend the view model with task journey and delivery data

**Files:**
- Modify: `apps/runtime/src/components/chat-side-panel/view-model.ts`
- Modify: `apps/runtime/src/components/chat-side-panel/view-model.test.ts`

**Step 1: Write the failing tests**

- Add tests for:
  - deriving current task when no `todo_write` exists
  - grouping repeated adjacent failures
  - extracting deliverables and warnings from tool calls

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/chat-side-panel/view-model.test.ts`

Expected: FAIL for missing task journey / deliverable behavior.

**Step 3: Write minimal implementation**

- Add `TaskJourneyViewModel` types and builders
- Reuse existing flattened tool call extraction
- Implement repeated failure grouping and deliverable classification

**Step 4: Run test to verify it passes**

Run: `pnpm vitest run apps/runtime/src/components/chat-side-panel/view-model.test.ts`

Expected: PASS

### Task 2: Add main-area timeline and delivery summary components

**Files:**
- Create: `apps/runtime/src/components/chat-journey/TaskJourneyTimeline.tsx`
- Create: `apps/runtime/src/components/chat-journey/DeliverySummaryCard.tsx`

**Step 1: Write the failing test**

- Extend existing chat tests to expect:
  - a visible task journey section in the main chat area
  - a delivery summary card with generated files and warnings

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL because components do not exist yet.

**Step 3: Write minimal implementation**

- Build lightweight presentational components from the new view model
- Keep detail rendering simple and compatible with current styling

**Step 4: Run test to verify it passes**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS for new main-area expectations

### Task 3: Integrate task journey into ChatView

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: Write the failing test**

- Add assertions that the main conversation shows:
  - current phase / current task
  - grouped failure summary instead of repeated raw failures
  - delivery summary after completion

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL because `ChatView` does not render the new blocks.

**Step 3: Write minimal implementation**

- Build the journey model from the current session messages
- Render `TaskJourneyTimeline` above assistant history
- Render `DeliverySummaryCard` near the latest completed assistant result
- Preserve existing side panel and `ToolIsland` behavior

**Step 4: Run test to verify it passes**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS

### Task 4: Reduce ToolIsland’s narrative responsibility

**Files:**
- Modify: `apps/runtime/src/components/ToolIsland.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing test**

- Add assertion that the primary status language is user-oriented and that raw details remain secondary.

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL on outdated rendering / wording expectations.

**Step 3: Write minimal implementation**

- Adjust summary wording and keep raw JSON/output inside expandable details only

**Step 4: Run test to verify it passes**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS

### Task 5: Verify integrated behavior

**Files:**
- Modify as needed based on failures above

**Step 1: Run focused tests**

Run: `pnpm vitest run apps/runtime/src/components/chat-side-panel/view-model.test.ts apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS

**Step 2: Run broader chat component tests if impacted**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.theme.test.tsx apps/runtime/src/components/__tests__/ChatView.find-skills-install.test.tsx`

Expected: PASS
