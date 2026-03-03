# Find Skills A2UI Install Confirmation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在“找技能”聊天场景中，展示可安装候选并通过 A2UI 确认弹窗完成一键安装。

**Architecture:** 从 `ChatView` 的 `streamItems` 解析 `clawhub_search/recommend` 的结构化输出渲染候选卡片，点击后走本地确认弹窗并调用 `install_clawhub_skill`。安装成功通过 `onSkillInstalled` 回调让 `App` 刷新全局技能状态。

**Tech Stack:** React + TypeScript + Tauri invoke + Vitest + Testing Library

---

### Task 1: 定义候选解析与视图模型

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx`（或新增专用测试文件）

**Step 1: Write the failing test**

```tsx
it("renders install candidates from clawhub tool outputs", async () => {
  // mock assistant streamItems including clawhub_search tool_call output JSON
  // assert candidate card and install button are visible
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm -C apps/runtime test -- ChatView`  
Expected: FAIL（候选卡片未渲染）

**Step 3: Write minimal implementation**

```tsx
type ClawhubCandidate = { slug: string; name: string; description?: string; stars?: number; githubUrl?: string | null };
function extractClawhubCandidatesFromMessage(message: ChatMessage): ClawhubCandidate[] { /* parse streamItems */ }
```

**Step 4: Run test to verify it passes**

Run: `pnpm -C apps/runtime test -- ChatView`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView*.test.tsx
git commit -m "feat(chat): parse clawhub install candidates from tool outputs"
```

### Task 2: 渲染候选卡片与已安装态

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/types.ts`（如需补充类型）
- Test: `apps/runtime/src/components/__tests__/ChatView*.test.tsx`

**Step 1: Write the failing test**

```tsx
it("disables install button when skill already installed", async () => {
  // mock installed skills includes clawhub-{slug}
  // assert button shows 已安装 and disabled
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm -C apps/runtime test -- ChatView`  
Expected: FAIL

**Step 3: Write minimal implementation**

```tsx
const installedSet = new Set(skills.map(s => s.id));
const installed = installedSet.has(`clawhub-${candidate.slug}`);
```

**Step 4: Run test to verify it passes**

Run: `pnpm -C apps/runtime test -- ChatView`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView*.test.tsx apps/runtime/src/types.ts
git commit -m "feat(chat): render clawhub candidate cards with installed state"
```

### Task 3: 增加 A2UI 安装确认弹窗

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView*.test.tsx`

**Step 1: Write the failing test**

```tsx
it("opens confirmation dialog when clicking install", async () => {
  // click install
  // assert confirmation modal text visible
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm -C apps/runtime test -- ChatView`  
Expected: FAIL

**Step 3: Write minimal implementation**

```tsx
const [pendingInstallSkill, setPendingInstallSkill] = useState<ClawhubCandidate | null>(null);
const [showInstallConfirm, setShowInstallConfirm] = useState(false);
```

**Step 4: Run test to verify it passes**

Run: `pnpm -C apps/runtime test -- ChatView`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView*.test.tsx
git commit -m "feat(chat): add a2ui install confirmation dialog for clawhub skills"
```

### Task 4: 调用安装命令并回调 App 刷新

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/types.ts`（如需 props 类型）
- Test: `apps/runtime/src/components/__tests__/ChatView*.test.tsx`
- Test: `apps/runtime/src/__tests__/App.*.test.tsx`（必要时）

**Step 1: Write the failing test**

```tsx
it("calls install_clawhub_skill and triggers onSkillInstalled after confirm", async () => {
  // mock invoke success
  // assert invoke called with slug/githubUrl
  // assert callback called
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm -C apps/runtime test -- ChatView App`  
Expected: FAIL

**Step 3: Write minimal implementation**

```tsx
await invoke("install_clawhub_skill", { slug, githubUrl });
await onSkillInstalled?.(`clawhub-${slug}`);
```

**Step 4: Run test to verify it passes**

Run: `pnpm -C apps/runtime test -- ChatView App`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/App.tsx apps/runtime/src/types.ts apps/runtime/src/components/__tests__/ChatView*.test.tsx apps/runtime/src/__tests__/App*.test.tsx
git commit -m "feat(chat): install clawhub skill from chat and sync app state"
```

### Task 5: 失败处理与重试体验

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView*.test.tsx`

**Step 1: Write the failing test**

```tsx
it("shows install error and allows retry", async () => {
  // first invoke rejects, second succeeds
  // assert error shown then cleared after retry success
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm -C apps/runtime test -- ChatView`  
Expected: FAIL

**Step 3: Write minimal implementation**

```tsx
try { ... } catch (e) { setInstallError(msg); } finally { setInstallingSlug(null); }
```

**Step 4: Run test to verify it passes**

Run: `pnpm -C apps/runtime test -- ChatView`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView*.test.tsx
git commit -m "fix(chat): handle clawhub install failure with retry feedback"
```

### Task 6: 全量验证与文档更新

**Files:**
- Modify: `README.md`（如需补充使用说明）
- Modify: `README.zh-CN.md`（如需补充中文说明）

**Step 1: Run focused tests**

Run: `pnpm -C apps/runtime test -- ChatView App`
Expected: PASS

**Step 2: Run tauri lib tests**

Run: `cargo test --lib` (workdir: `apps/runtime/src-tauri`)
Expected: PASS

**Step 3: Manual smoke check**

```text
1) 找技能提问 -> 出现候选卡
2) 点击立即安装 -> 出现确认弹窗
3) 确认安装 -> 安装成功提示
4) 专家技能页可见新技能
```

**Step 4: Commit**

```bash
git add README.md README.zh-CN.md
git commit -m "docs: add chat install flow for find-skills"
```
