# Unified Design Language Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a complete semantic theme system for Runtime UI and migrate the 4 core views (`App`, `Sidebar`, `ChatView`, `SettingsView`) to unified visual language without changing behavior.

**Architecture:** Introduce global design tokens in `index.css`, then build semantic primitives (`sm-*` classes) as a stable style contract. Migrate each target view incrementally with TDD-style contract tests that verify semantic classes and critical visual states. Keep all data flow, commands, and event handling untouched.

**Tech Stack:** React 18, TypeScript, Tailwind CSS v4 (CSS-first), Vitest, Testing Library, Tauri runtime API mocks

---

Related skills: `@test-driven-development`, `@verification-before-completion`

### Task 1: Add Sidebar Theme Contract Test

**Files:**
- Create: `apps/runtime/src/components/__tests__/Sidebar.theme.test.tsx`
- Test: `apps/runtime/src/components/__tests__/Sidebar.theme.test.tsx`

**Step 1: Write the failing test**

```tsx
import { render, screen } from "@testing-library/react";
import { Sidebar } from "../Sidebar";

test("uses semantic theme classes for shell and controls", () => {
  render(
    <Sidebar
      activeMainView="start-task"
      onOpenStartTask={() => {}}
      onOpenExperts={() => {}}
      onOpenEmployees={() => {}}
      selectedSkillId="builtin-general"
      sessions={[]}
      selectedSessionId={null}
      onSelectSession={() => {}}
      newSessionPermissionMode="accept_edits"
      onChangeNewSessionPermissionMode={() => {}}
      onDeleteSession={() => {}}
      onSettings={() => {}}
      onSearchSessions={() => {}}
      onExportSession={() => {}}
      onCollapse={() => {}}
      collapsed={false}
    />
  );

  expect(screen.getByText("WorkClaw").closest("div")).toHaveClass("sm-surface");
  expect(screen.getByRole("button", { name: "开始任务" })).toHaveClass("sm-btn");
  expect(screen.getByPlaceholderText("搜索会话...")).toHaveClass("sm-input");
});
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime && pnpm test -- src/components/__tests__/Sidebar.theme.test.tsx`
Expected: FAIL with missing semantic classes.

**Step 3: Write minimal implementation**

Modify `Sidebar.tsx` to use semantic classes for container/nav buttons/search input.

```tsx
<div className="sm-surface ...">
...
<button className="sm-btn sm-btn-secondary ...">开始任务</button>
...
<input className="sm-input ..." />
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime && pnpm test -- src/components/__tests__/Sidebar.theme.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/__tests__/Sidebar.theme.test.tsx apps/runtime/src/components/Sidebar.tsx
git commit -m "test(ui): add sidebar semantic theme contract"
```

### Task 2: Add Global Tokens and Primitives in index.css

**Files:**
- Modify: `apps/runtime/src/index.css`
- Test: `apps/runtime/src/components/__tests__/Sidebar.theme.test.tsx`

**Step 1: Write the failing test**

Extend existing test to assert token-backed class names are present in DOM:

```tsx
expect(screen.getByRole("combobox")).toHaveClass("sm-select");
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime && pnpm test -- src/components/__tests__/Sidebar.theme.test.tsx`
Expected: FAIL because `sm-select` and primitives are not defined or not applied.

**Step 3: Write minimal implementation**

Add token and semantic primitive layer in `index.css`.

```css
:root {
  --sm-bg: #f5f7fb;
  --sm-surface: #ffffff;
  --sm-surface-muted: #f8fafc;
  --sm-text: #0f172a;
  --sm-text-muted: #64748b;
  --sm-border: #e2e8f0;
  --sm-primary: #2563eb;
  --sm-primary-strong: #1d4ed8;
  --sm-success: #16a34a;
  --sm-warn: #d97706;
  --sm-danger: #dc2626;
  --sm-radius-sm: 8px;
  --sm-radius-md: 12px;
  --sm-radius-lg: 16px;
  --sm-duration-fast: 150ms;
  --sm-duration-base: 220ms;
  --sm-focus-ring: 0 0 0 2px rgba(37, 99, 235, 0.35);
}

@layer components {
  .sm-surface { background: var(--sm-surface); color: var(--sm-text); border-color: var(--sm-border); }
  .sm-input { border: 1px solid var(--sm-border); background: var(--sm-surface-muted); color: var(--sm-text); border-radius: var(--sm-radius-sm); }
  .sm-select { border: 1px solid var(--sm-border); background: var(--sm-surface-muted); color: var(--sm-text); border-radius: var(--sm-radius-sm); }
  .sm-btn { border-radius: var(--sm-radius-sm); transition: background-color var(--sm-duration-fast) ease, color var(--sm-duration-fast) ease; }
  .sm-btn-primary { background: var(--sm-primary); color: #fff; }
  .sm-btn-secondary { background: var(--sm-surface-muted); color: var(--sm-text); border: 1px solid var(--sm-border); }
}
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime && pnpm test -- src/components/__tests__/Sidebar.theme.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/index.css apps/runtime/src/components/__tests__/Sidebar.theme.test.tsx
git commit -m "feat(ui): add global design tokens and semantic primitives"
```

### Task 3: Migrate App + Sidebar to Semantic Theme Layer

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/components/Sidebar.tsx`
- Test: `apps/runtime/src/__tests__/App.chat-landing.test.tsx`

**Step 1: Write the failing test**

Extend app test to assert root semantic app shell class:

```tsx
expect(document.querySelector(".sm-app")).toBeInTheDocument();
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime && pnpm test -- src/__tests__/App.chat-landing.test.tsx`
Expected: FAIL due missing `sm-app`.

**Step 3: Write minimal implementation**

Use semantic container/text classes in `App.tsx`, normalize remaining sidebar utility palette usage.

```tsx
<div className="sm-app flex h-screen overflow-hidden">
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime && pnpm test -- src/__tests__/App.chat-landing.test.tsx src/components/__tests__/Sidebar.theme.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/components/Sidebar.tsx apps/runtime/src/__tests__/App.chat-landing.test.tsx
git commit -m "refactor(ui): migrate app shell and sidebar to semantic theme classes"
```

### Task 4: Add ChatView Theme Contract Test

**Files:**
- Create: `apps/runtime/src/components/__tests__/ChatView.theme.test.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.theme.test.tsx`

**Step 1: Write the failing test**

```tsx
import { render, screen } from "@testing-library/react";
import { ChatView } from "../ChatView";

test("uses semantic classes in input shell and action buttons", () => {
  render(
    <ChatView
      skill={{ id: "builtin-general", name: "General", description: "", version: "1", author: "", recommended_model: "", tags: [], created_at: new Date().toISOString() }}
      models={[{ id: "m1", name: "M1", api_format: "openai", base_url: "https://example.com", model_name: "m1", is_default: true }]}
      sessionId="s1"
    />
  );
  expect(screen.getByPlaceholderText("输入消息，Shift+Enter 换行...")).toHaveClass("sm-textarea");
  expect(screen.getByRole("button", { name: "发送" })).toHaveClass("sm-btn-primary");
});
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime && pnpm test -- src/components/__tests__/ChatView.theme.test.tsx`
Expected: FAIL with missing semantic class assertions.

**Step 3: Write minimal implementation**

Refactor chat input shell/buttons/cards to semantic primitives while preserving behavior.

```tsx
<div className="sm-panel ...">
  <textarea className="sm-textarea ..." />
  <button className="sm-btn sm-btn-primary ...">发送</button>
</div>
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime && pnpm test -- src/components/__tests__/ChatView.theme.test.tsx src/components/__tests__/ChatView.im-routing-panel.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView.theme.test.tsx
git commit -m "refactor(ui): migrate chat view to semantic theme classes"
```

### Task 5: Add SettingsView Theme Contract Test

**Files:**
- Create: `apps/runtime/src/components/__tests__/SettingsView.theme.test.tsx`
- Test: `apps/runtime/src/components/__tests__/SettingsView.theme.test.tsx`

**Step 1: Write the failing test**

```tsx
import { render, screen } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

test("uses semantic classes for tabs and primary actions", async () => {
  render(<SettingsView onClose={() => {}} />);
  expect(screen.getByRole("button", { name: "模型" })).toHaveClass("sm-btn");
  expect(screen.getByRole("button", { name: "保存配置" })).toHaveClass("sm-btn-primary");
});
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime && pnpm test -- src/components/__tests__/SettingsView.theme.test.tsx`
Expected: FAIL due missing semantic classes.

**Step 3: Write minimal implementation**

Migrate settings tabs/forms/notices/buttons to semantic classes.

```tsx
<button className="sm-btn sm-btn-secondary ...">模型</button>
...
<button className="sm-btn sm-btn-primary ...">保存配置</button>
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime && pnpm test -- src/components/__tests__/SettingsView.theme.test.tsx src/components/__tests__/SettingsView.feishu.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/__tests__/SettingsView.theme.test.tsx
git commit -m "refactor(ui): migrate settings view to semantic theme classes"
```

### Task 6: Full Verification and Cleanup

**Files:**
- Modify: `apps/runtime/src/index.css` (only if final token adjustments needed)
- Modify: `docs/plans/2026-03-03-unified-design-language-design.md` (optional acceptance check notes)
- Test: `apps/runtime/src/__tests__/*.test.tsx`, `apps/runtime/src/components/__tests__/*.test.tsx`

**Step 1: Write failing guard test (optional if missing)**

Add one global guard asserting semantic app class exists:

```tsx
expect(document.querySelector(".sm-app")).toBeTruthy();
```

**Step 2: Run full tests to capture current failures**

Run: `cd apps/runtime && pnpm test`
Expected: FAIL only if remaining hardcoded class assumptions conflict.

**Step 3: Write minimal implementation/fixes**

Patch remaining class regressions (focus-visible, disabled state, warning/error badges) with semantic primitives only.

```css
.sm-btn:focus-visible { outline: none; box-shadow: var(--sm-focus-ring); }
```

**Step 4: Run full tests to verify pass**

Run: `cd apps/runtime && pnpm test`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/index.css apps/runtime/src/App.tsx apps/runtime/src/components/Sidebar.tsx apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/__tests__ apps/runtime/src/__tests__
git commit -m "feat(ui): complete unified semantic design language for runtime core views"
```
