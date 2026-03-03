# Expert Skills Hub Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement an Expert Skills hub with navigation entry, "my skills" list, and a two-column guided creation page that saves `SKILL.md` to a user-selected directory (or fallback path).

**Architecture:** Add route-like page states in frontend (`start-task`, `experts`, `experts-new`) and keep existing chat/session flow intact. Introduce two new React components for list and create screens. Add a new Tauri skill command to write local `SKILL.md` and reuse existing `import_local_skill` for DB registration.

**Tech Stack:** React 18 + TypeScript, Tauri commands (Rust), SQLite-backed installed skills table, Vitest + Testing Library.

---

### Task 1: Add Failing Frontend Tests for Expert Navigation and Views

**Files:**
- Create: `apps/runtime/src/__tests__/App.experts-routing.test.tsx`

**Step 1: Write failing tests**

Cover:
- sidebar nav includes `开始任务` and `专家技能`
- clicking `专家技能` shows experts view
- experts view shows `我的技能` and hides `技能社区`
- clicking `创建` navigates to create page

**Step 2: Run tests to verify failure**

Run: `cd apps/runtime && npm test -- App.experts-routing`  
Expected: FAIL because routing + views are not implemented

**Step 3: Commit (optional)**

```bash
git add apps/runtime/src/__tests__/App.experts-routing.test.tsx
git commit -m "test(runtime): add failing tests for experts routing"
```

### Task 2: Implement Sidebar Navigation Refactor

**Files:**
- Modify: `apps/runtime/src/components/Sidebar.tsx`

**Step 1: Implement minimal nav updates**

- replace `对话` label with `开始任务`
- add `专家技能` nav action
- keep `设置`
- remove standalone homepage packaging entry
- keep session list behavior only under start-task context

**Step 2: Run focused tests**

Run: `cd apps/runtime && npm test -- App.experts-routing`  
Expected: still FAIL until App routing is wired

### Task 3: Implement App Route-like State and Experts View

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Create: `apps/runtime/src/components/experts/ExpertsView.tsx`

**Step 1: Add failing view-level tests (if needed)**

Extend `App.experts-routing.test.tsx` for:
- `/experts` equivalent state renders experts list
- `技能社区` tab not rendered

**Step 2: Implement minimal code**

- add `activeMainView` variant for experts page(s)
- render `ExpertsView` when active section is experts
- pass skills list to `ExpertsView`
- include actions:
  - create
  - packaging (moved here)

**Step 3: Run tests**

Run: `cd apps/runtime && npm test -- App.experts-routing`  
Expected: PASS for list-view routing assertions

### Task 4: Add Failing Tests for Create Expert Skill Page

**Files:**
- Create: `apps/runtime/src/components/experts/__tests__/ExpertCreateView.test.tsx`

**Step 1: Write failing tests**

Cover:
- two-column layout renders
- editing left form updates right preview
- choose-directory callback updates path display
- save callback called with structured payload

**Step 2: Run tests and verify fail**

Run: `cd apps/runtime && npm test -- ExpertCreateView`  
Expected: FAIL because component does not exist

### Task 5: Implement `ExpertCreateView` Component

**Files:**
- Create: `apps/runtime/src/components/experts/ExpertCreateView.tsx`

**Step 1: Implement minimal two-column UI**

Left:
- name/description/when-to-use input controls
- choose-directory button
- save button

Right:
- live preview markdown/string builder for `SKILL.md`

**Step 2: Run targeted tests**

Run: `cd apps/runtime && npm test -- ExpertCreateView`  
Expected: PASS

### Task 6: Add Backend Command for Local Skill Creation

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/skills.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Optional tests: `apps/runtime/src-tauri/tests/test_skill_create_local.rs`

**Step 1: Write failing backend test**

Cover:
- when `target_dir` empty -> fallback `~/.workclaw/skills/`
- creates `<slug>/SKILL.md`
- writes valid frontmatter + content

**Step 2: Run backend test (fail)**

Run: `cd apps/runtime/src-tauri && cargo test test_skill_create_local -- --nocapture`  
Expected: FAIL before command implementation

**Step 3: Implement minimal command**

Add command:
- `create_local_skill(name, description, content_sections, target_dir?) -> created_path`

Rules:
- slugify name safely
- ensure base dir exists
- write file atomically if possible

Register in invoke handler.

**Step 4: Run backend test (pass)**

Run: `cd apps/runtime/src-tauri && cargo test test_skill_create_local -- --nocapture`  
Expected: PASS

### Task 7: Wire Create Page Save Flow in App

**Files:**
- Modify: `apps/runtime/src/App.tsx`

**Step 1: Add failing integration tests**

In `App.experts-routing.test.tsx`:
- save from create page calls `create_local_skill`
- then calls `import_local_skill`
- returns to experts list

**Step 2: Run test (fail)**

Run: `cd apps/runtime && npm test -- App.experts-routing`  
Expected: FAIL before save flow wiring

**Step 3: Implement flow**

- in App, handle create payload submit:
  - call `create_local_skill`
  - call `import_local_skill` with created folder
  - reload skills
  - navigate back to experts list

Fallback:
- if no selected dir from UI, pass empty and let backend fallback.

**Step 4: Run tests**

Run: `cd apps/runtime && npm test -- App.experts-routing`  
Expected: PASS

### Task 8: Regression Verification

**Files:**
- Existing frontend tests only (unless fixes needed)

**Step 1: Run all frontend tests**

Run: `cd apps/runtime && npm test`  
Expected: PASS

**Step 2: Run frontend build**

Run: `cd apps/runtime && npm run build`  
Expected: PASS

**Step 3: Run key backend tests (if touched)**

Run: `cd apps/runtime/src-tauri && cargo test`  
Expected: PASS for touched command scope (or full suite if feasible)

### Task 9: Update Docs

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`

**Step 1: Add concise product notes**

- nav now includes Expert Skills
- packaging entry moved into Expert Skills page
- create flow path selection + default fallback path

**Step 2: Verify docs diff**

Run: `git diff -- README.md README.zh-CN.md`  
Expected: only targeted wording changes

