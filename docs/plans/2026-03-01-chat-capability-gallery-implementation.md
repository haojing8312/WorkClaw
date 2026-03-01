# Chat Capability Gallery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a four-card "common task scenarios" gallery below the chat no-session landing page so users can click a scenario to fill the input template before explicitly starting a new session.

**Architecture:** Keep all phase-2 logic local to `NewSessionLanding` and avoid backend/API changes. Add static card metadata, local selected state, and input-fill interaction that focuses and scrolls to the input block. Reuse existing app-level create-session callback unchanged.

**Tech Stack:** React 18, TypeScript, Tailwind CSS, Vitest, Testing Library.

---

### Task 1: Add Failing Tests for Scenario Gallery Behavior

**Files:**
- Modify: `apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`

**Step 1: Write failing tests**

Add test cases:
- renders exactly 4 scenario cards with expected titles
- clicking a scenario fills textarea with its `promptTemplate`
- clicking a scenario does not call `onCreateSessionWithInitialMessage`
- hint text appears after scenario fill
- explicit click on `开始新会话` still triggers create callback with filled value

Example test snippet:
```tsx
test("fills input from scenario card", () => {
  const onCreate = vi.fn();
  render(
    <NewSessionLanding
      sessions={[]}
      creating={false}
      onSelectSession={() => {}}
      onCreateSessionWithInitialMessage={onCreate}
    />
  );
  fireEvent.click(screen.getByRole("button", { name: /文件整理助手/i }));
  expect(screen.getByPlaceholderText("先描述你要完成什么任务...")).toHaveValue(
    "请帮我整理下载目录，把文件按类型分类到子文件夹，并按近30天和更早文件分开。先告诉我你的整理方案。"
  );
  expect(onCreate).not.toHaveBeenCalled();
});
```

**Step 2: Run tests to verify failure**

Run: `cd apps/runtime && npm test -- NewSessionLanding`  
Expected: FAIL on new scenario-related assertions (cards/hint/fill not implemented)

**Step 3: Commit failing test baseline (optional if team allows)**

```bash
git add apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx
git commit -m "test(runtime): add failing tests for landing scenario gallery"
```

### Task 2: Implement Static Scenario Cards and Fill Interaction

**Files:**
- Modify: `apps/runtime/src/components/NewSessionLanding.tsx`

**Step 1: Implement minimal code to satisfy failing tests**

Add:
- static `SCENARIO_CARDS` constant with 4 cards:
  - 文件整理助手
  - 本地数据汇总
  - 浏览器信息采集
  - 代码问题排查
- `selectedScenarioId` state
- `showFilledHint` state
- textarea `ref`
- `handleSelectScenario`:
  - set input to `promptTemplate`
  - set selected id
  - set hint true
  - focus textarea
  - smooth scroll input block into view

Render scenario section below recent sessions with responsive grid and selected styling.

**Step 2: Run targeted tests**

Run: `cd apps/runtime && npm test -- NewSessionLanding`  
Expected: PASS all landing component tests

**Step 3: Commit**

```bash
git add apps/runtime/src/components/NewSessionLanding.tsx apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx
git commit -m "feat(runtime): add landing scenario gallery with input template fill"
```

### Task 3: Add Accessibility and UI State Assertions

**Files:**
- Modify: `apps/runtime/src/components/NewSessionLanding.tsx`
- Modify: `apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`

**Step 1: Add/adjust tests**

Add assertions for:
- selected card exposes `aria-pressed=true`
- non-selected cards expose `aria-pressed=false`
- `开始新会话` still works after scenario fill and manual edit

**Step 2: Run tests to verify fail-then-pass cycle**

Run: `cd apps/runtime && npm test -- NewSessionLanding`  
Expected: FAIL first if aria states missing, then PASS after implementation

**Step 3: Implement minimal accessibility updates**

In card buttons:
- add `aria-pressed={selectedScenarioId === card.id}`
- preserve keyboard accessibility defaults

**Step 4: Re-run targeted tests**

Run: `cd apps/runtime && npm test -- NewSessionLanding`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/NewSessionLanding.tsx apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx
git commit -m "fix(runtime): improve scenario card accessibility state"
```

### Task 4: Regression Verification Across App Landing Flow

**Files:**
- No new files required unless regression fix is needed

**Step 1: Run app-level landing tests**

Run:
```bash
cd apps/runtime && npm test -- App.chat-landing
cd apps/runtime && npm test -- App.session-create-flow
```
Expected: both PASS

**Step 2: Run full frontend tests**

Run: `cd apps/runtime && npm test`  
Expected: all tests pass

**Step 3: Build verification**

Run: `cd apps/runtime && npm run build`  
Expected: TypeScript + Vite build success

**Step 4: Commit (if any regression fixes were applied)**

```bash
git add apps/runtime/src
git commit -m "test(runtime): verify landing gallery regressions"
```

### Task 5: Documentation Sync for Phase 2 UX

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`

**Step 1: Add concise UX note**

Document that no-session landing now includes:
- capability intro
- recent sessions
- common task scenario cards that fill input templates

**Step 2: Verify docs only change expected sections**

Run: `git diff -- README.md README.zh-CN.md`

**Step 3: Commit**

```bash
git add README.md README.zh-CN.md
git commit -m "docs: describe chat landing scenario gallery behavior"
```

