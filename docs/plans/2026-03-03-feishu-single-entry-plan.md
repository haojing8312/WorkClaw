# Feishu Single Entry Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the Settings "飞书协作" entry and keep Feishu credential setup only in the "智能体员工" page.

**Architecture:** Keep backend Feishu capabilities unchanged, but remove the Settings UI entry and section that expose gateway/routing/thread concepts. Employee-level Feishu credentials remain in `EmployeeHubView` as the only user-facing entry. Update docs and tests to match the new information architecture.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Update tests first (TDD Red)

**Files:**
- Modify: `apps/runtime/src/components/__tests__/SettingsView.feishu.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/SettingsView.feishu-routing-wizard.test.tsx`

**Step 1: Write failing tests for new behavior**

- Replace legacy expectations that click `飞书协作`.
- Add assertions that settings tabs do not include `飞书协作`.

**Step 2: Run tests to verify failure**

Run:

```bash
pnpm --dir apps/runtime test -- src/components/__tests__/SettingsView.feishu.test.tsx src/components/__tests__/SettingsView.feishu-routing-wizard.test.tsx
```

Expected: Tests fail because current UI still contains `飞书协作` tab.

### Task 2: Remove settings-level Feishu entry (TDD Green)

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`

**Step 1: Write minimal implementation**

- Remove `飞书协作` from `activeTab` union and tab button list.
- Remove `activeTab === "feishu"` rendering block.

**Step 2: Run focused tests**

Run:

```bash
pnpm --dir apps/runtime test -- src/components/__tests__/SettingsView.feishu.test.tsx src/components/__tests__/SettingsView.feishu-routing-wizard.test.tsx
```

Expected: Tests pass.

### Task 3: Update documentation

**Files:**
- Modify: `README.zh-CN.md`
- Modify: `README.md`

**Step 1: Adjust Feishu setup instructions**

- Replace "设置 -> 飞书协作" and global credential wording with employee-page credential setup wording.
- Explicitly mention only `智能体员工` is required for ordinary users.

**Step 2: Verify docs are updated**

Run:

```bash
rg -n "设置 -> 飞书协作|App ID / App Secret|智能体员工" README.zh-CN.md README.md
```

Expected: No outdated setup flow remains.

### Task 4: Final verification

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/components/__tests__/SettingsView.feishu.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/SettingsView.feishu-routing-wizard.test.tsx`
- Modify: `README.zh-CN.md`
- Modify: `README.md`

**Step 1: Run targeted frontend tests**

```bash
pnpm --dir apps/runtime test -- src/components/__tests__/SettingsView.feishu.test.tsx src/components/__tests__/SettingsView.feishu-routing-wizard.test.tsx
```

**Step 2: Run a broader safety pass**

```bash
pnpm --dir apps/runtime test
```

Expected: pass (or report any unrelated pre-existing failures explicitly).
