# Landing Attachments And Workdir Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `附件` and `工作目录` controls to the homepage task composer and carry their selected context into the newly created chat session.

**Architecture:** Keep the change frontend-only and reuse existing chat/session contracts. Extend `NewSessionLanding` to collect pending attachments and an optional work directory, then let `App` pass that state through the existing session creation and initial message flow so `ChatView` receives it as initial composer context.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, Tauri dialog/plugin APIs already used in runtime UI

---

### Task 1: Add failing landing tests for the new controls

**Files:**
- Modify: `apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`

**Step 1: Write the failing tests**

Add tests that verify:
- The landing composer renders `附件` and `选择工作目录` controls.
- Clicking the directory control calls the provided picker and shows the selected path.
- Choosing files updates the landing UI with the attachment summary.
- Submitting the landing form passes message, attachments, and workdir together.

**Step 2: Run the test to verify it fails**

Run: `pnpm --filter runtime test -- --run apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`

Expected: FAIL because `NewSessionLanding` does not yet expose the new controls or callback shape.

### Task 2: Add failing App handoff tests for session bootstrap context

**Files:**
- Modify: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`

**Step 1: Write the failing tests**

Add tests that verify:
- `NewSessionLanding` can call the app with `initialMessage`, `attachments`, and `workDir`.
- `App` creates the session with the explicit landing workdir.
- `App` passes the initial composer attachments into `ChatView`.

**Step 2: Run the test to verify it fails**

Run: `pnpm --filter runtime test -- --run apps/runtime/src/__tests__/App.session-create-flow.test.tsx`

Expected: FAIL because `App` currently only forwards a string message and has no pending attachment bootstrap state.

### Task 3: Implement the minimal landing composer support

**Files:**
- Modify: `apps/runtime/src/components/NewSessionLanding.tsx`
- Modify: `apps/runtime/src/types.ts`

**Step 1: Add minimal landing context types**

Define a landing submit payload that includes:
- `initialMessage`
- `attachments`
- `workDir`

Reuse `PendingAttachment` from existing chat code instead of introducing a second attachment model.

**Step 2: Add landing UI and state**

Implement:
- Hidden file input + `附件` trigger
- Attachment summary + remove actions
- Workdir picker button using an injected callback
- Landing submit callback that sends the full payload

**Step 3: Keep the implementation narrow**

Do not add:
- New execution modes
- New protocol fields in backend session creation
- Any new runtime permissions

### Task 4: Implement App context propagation into the first chat render

**Files:**
- Modify: `apps/runtime/src/App.tsx`

**Step 1: Extend landing callback handling**

Update the landing callback and pending state so `App` stores:
- pending initial message
- pending initial attachments
- explicit landing workdir

**Step 2: Reuse existing session creation flow**

Use the explicit landing workdir as the preferred workdir for `resolveSessionLaunchWorkDir`. Keep the existing `create_session` invoke contract unchanged.

**Step 3: Pass bootstrap context to ChatView**

Provide `ChatView` with:
- `initialMessage`
- `initialAttachments`

and clear the pending bootstrap state after consumption.

### Task 5: Teach ChatView to hydrate composer attachments from initial context

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx` if needed

**Step 1: Add an optional prop for initial attachments**

Allow `ChatView` to receive initial pending attachments for a brand-new session.

**Step 2: Hydrate once and preserve existing behavior**

When the session first mounts with initial attachments:
- seed `attachedFiles`
- keep existing send/remove logic
- clear the bootstrap state after `App` is notified

### Task 6: Verify the focused surfaces

**Files:**
- Modify: none

**Step 1: Run targeted tests**

Run:
- `pnpm --filter runtime test -- --run apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`
- `pnpm --filter runtime test -- --run apps/runtime/src/__tests__/App.session-create-flow.test.tsx`

**Step 2: Run the broader runtime coverage only if required by failures or touched behavior**

If the targeted tests expose adjacent regressions, run:
- `pnpm test:e2e:runtime`

**Step 3: Summarize verification honestly**

Report:
- commands run
- pass/fail
- changed surface covered
- any still-unverified areas
