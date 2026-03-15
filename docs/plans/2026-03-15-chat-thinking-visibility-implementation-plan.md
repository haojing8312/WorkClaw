# Chat Thinking Visibility Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Keep the transient `思考中` state visible at the bottom of long chat sessions by rendering it with the active streaming assistant bubble.

**Architecture:** Adjust `ChatView.tsx` so the transient thinking-only UI is no longer rendered before historical messages. Instead, reuse the existing bottom streaming bubble for both thinking-only and thinking-plus-answer streaming states. Lock the behavior with a focused Vitest test that verifies ordering against existing chat history.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Capture the regression with a failing test

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.thinking-block.test.tsx`

**Step 1: Write the failing test**

Add a test that renders a chat with existing message history, emits a new thinking state without answer tokens, and asserts the transient `思考中` block appears after the historical messages in document order.

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.thinking-block.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: FAIL because the transient thinking-only block is currently rendered before `messages.map(...)`.

### Task 2: Move the transient thinking block into the bottom streaming bubble

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: Write minimal implementation**

Remove the standalone thinking-only render branch that sits above the message history and widen the existing bottom streaming bubble condition so it also renders when the assistant is thinking without answer tokens yet.

**Step 2: Run the focused test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.thinking-block.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: PASS

### Task 3: Verify nearby chat behavior remains stable

**Files:**
- Verify: `apps/runtime/src/components/__tests__/ChatView.thinking-block.test.tsx`
- Verify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Run targeted chat tests**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.thinking-block.test.tsx src/components/__tests__/ChatView.session-resilience.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: PASS

**Step 2: Run typecheck**

Run: `pnpm --dir apps/runtime exec tsc --noEmit`

Expected: PASS
