# Task Files Entry Card Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restyle the completed-task files entry in the chat transcript into a compact completion-state card without changing its behavior.

**Architecture:** Keep the existing `TaskJourneySummary` render guard and click handler intact. Update only the summary card markup and styling so the control reads as a delivery card rather than a generic oversized button, then lock the new information hierarchy in the existing transcript regression tests.

**Tech Stack:** React 18 + TypeScript, Tailwind CSS utilities, Vitest + Testing Library

---

### Task 1: Lock the new completion-card copy in transcript tests

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing test assertions**

Update the existing files-entry tests to assert the transcript card now includes:

- the existing button name `查看此任务中的所有文件`
- a new completion hint `任务已完成，点击查看本次产出文件`
- a deliverable count such as `共 2 个文件`

**Step 2: Run the focused test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: FAIL because the current component only renders the title line.

### Task 2: Implement the compact completion-state card

**Files:**
- Modify: `apps/runtime/src/components/chat-journey/TaskJourneySummary.tsx`

**Step 1: Write the minimal UI implementation**

Update the summary button so it renders:

- a blue-tinted icon badge on the left
- a title and subtitle in the middle
- a subtle right-arrow indicator
- the deliverable count derived from `model.deliverables.length`

Use lighter blue-gray styling, a tighter vertical rhythm, and accessible `focus-visible` states while keeping the click behavior unchanged.

**Step 2: Run the focused test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

### Task 3: Final targeted verification

**Files:**
- No additional files unless fixes are needed

**Step 1: Run the targeted transcript suite**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

**Step 2: Run a frontend build smoke check if practical**

Run: `pnpm --dir apps/runtime build`
Expected: PASS
