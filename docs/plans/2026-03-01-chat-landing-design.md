# 2026-03-01 Chat Landing Design

## 1. Background

Current runtime UX enters chat mainly through sidebar session operations. For first-time or light users, this does not clearly explain product value or the kinds of tasks the assistant can handle.

Goal for phase 1 is to introduce a new-session landing page in the `chat` main view, inspired by MiniMax layout structure but not visual copy. The UI language must remain in WorkClaw's existing blue style system.

## 2. Product Positioning and Terminology

- This page targets general users.
- Do not use internal terms like "Skill" on the landing page.
- Language should explain practical capabilities directly:
  - create/modify files
  - analyze local file data
  - organize files
  - operate browser workflows

## 3. Scope

### In Scope (Phase 1)

- Add a new-session landing page for `chat` view when no session is selected.
- Provide a single large task input entry.
- Show simple capability introduction text (static display, non-interactive).
- Show recent sessions list (3-6 items), clickable to enter.
- Keep existing session creation flow and chat view behavior.

### Out of Scope (Phase 1)

- No quick-action chips.
- No expert/marketplace changes yet.
- No expert creation page changes yet.
- No side navigation architecture rewrite.

## 4. IA and View States

Under `activeMainView === "chat"`:

1. No-session state: render `NewSessionLanding`.
2. Session-selected state: render existing `ChatView`.

Under `activeMainView === "packaging"`:

- Keep current behavior unchanged.

## 5. Landing Content Design

### 5.1 Hero Copy

- Title: `把你的电脑任务，交给 AI 助手协作完成`
- Subtitle: `一句话描述需求，它可以帮你创建和修改文件、分析本地数据、整理文件、操作浏览器，并持续反馈执行过程。`

### 5.2 Capability Display

Static capability tags/cards only (not shortcuts):

- 创建/修改文件
- 分析本地文件
- 文件整理
- 浏览器操作

### 5.3 Core Action Area

- Large multiline input.
- Primary CTA: `开始新会话`.
- Input behavior:
  - `Enter` to submit
  - `Shift+Enter` newline

### 5.4 Recent Sessions

- Show latest sessions (max 6).
- Empty text: `暂无会话，从上方输入任务开始`.
- Click item to open session directly.

## 6. Interaction Flow

### Flow A: Input + Create + Auto-send

1. User enters task in landing input.
2. Click `开始新会话` (or Enter).
3. Reuse existing create-session process (including workspace selection).
4. On success, auto-send first message.
5. Enter normal chat view.

### Flow B: Empty Input Create

1. User clicks `开始新会话` with empty input.
2. Create empty session only.
3. Enter normal chat view.

### Flow C: Workspace Select Cancel

1. User starts creation.
2. Cancels folder dialog.
3. Stay on landing with no error toast.

## 7. Component and State Changes

## 7.1 New Component

- Add `apps/runtime/src/components/NewSessionLanding.tsx`.
- Props (planned):
  - `sessions`
  - `onSelectSession`
  - `onCreateSessionWithInitialMessage`
  - `creating`
  - `error`

### 7.2 App-level Integration

In `App.tsx`:

- Replace current no-session placeholder button with `NewSessionLanding` under chat view.
- Extend create-session handler to support optional `initialMessage`.

### 7.3 Error/Loading States

- `creatingSession`: disable submit and show `正在创建...`.
- `createError`: show inline helper text under input.
- Auto-send failure after creation: keep session entered, append recoverable hint in chat.

## 8. Visual Direction

- Keep existing blue primary color and neutral gray surfaces.
- Use layout inspiration only from MiniMax:
  - central focus entry
  - generous whitespace
  - low-noise hierarchy
- Avoid near-1:1 structure or style cloning.

## 9. Testing and Verification

Manual test matrix for phase 1:

1. Input task -> create session -> auto-send -> enter chat.
2. Empty input -> create session -> enter empty chat.
3. Cancel workspace picker -> remain landing.
4. Click recent session -> enter selected chat.
5. Regression check:
   - packaging view
   - settings view
   - sidebar collapse/expand

## 10. Rollout Sequencing

1. Phase 1: chat landing page (this design).
2. Phase 2: expert/community marketplace section (mapped to internal skills).
3. Phase 3: creation workspace with left-right authoring/preview flow.

