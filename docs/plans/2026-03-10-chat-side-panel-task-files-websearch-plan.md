# Chat Side Panel Task Files WebSearch Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the chat side panel with three focused tabs for current task progress, workspace files, and session-scoped web searches.

**Architecture:** Keep message-area tool playback intact, but replace the current right panel with a view-model driven shell. Frontend aggregates session tool calls into task and web-search state, while new Tauri commands provide workspace file listing and preview data. Safe external link opening reuses the existing backend command.

**Tech Stack:** React, TypeScript, Tauri commands in Rust, Vitest, Testing Library

---

### Task 1: Lock down current side-panel replacement in ChatView tests

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.risk-flow.test.tsx`
- Create: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing tests**

Add tests that assert:

- old tabs `附件与工具` and `自动路由` are not rendered
- new tabs `当前任务`, `文件`, `Web 搜索` are rendered
- a chat session with `todo_write` tool calls shows task summary and task rows
- a chat session with `web_search` tool calls shows search history list and detail pane
- a workspace-backed session can render file list UI and empty preview state

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL because the new tabs and task/files/search panel behaviors do not exist.

**Step 3: Make minimal test cleanup edits**

Remove or rewrite assertions in older tests that depend on the deleted `自动路由` panel.

**Step 4: Run affected tests again**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx apps/runtime/src/components/__tests__/ChatView.risk-flow.test.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL only on missing redesign behavior, not on stale test assumptions.

### Task 2: Add reusable side-panel parsing and view-model helpers

**Files:**
- Create: `apps/runtime/src/components/chat-side-panel/view-model.ts`
- Create: `apps/runtime/src/components/chat-side-panel/view-model.test.ts`
- Modify: `apps/runtime/src/types.ts`

**Step 1: Write the failing tests**

Cover pure helpers for:

- extracting the latest todo list from `todo_write`
- counting completed/in-progress/pending tasks
- extracting session-created or updated file paths from `write_file` and `edit`
- extracting parsed web-search entries from `web_search` output
- handling malformed tool outputs safely

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/chat-side-panel/view-model.test.ts`

Expected: FAIL because helper module does not exist.

**Step 3: Implement minimal helpers**

Create a pure module that exports small functions and typed view models for:

- `buildTaskPanelViewModel(messages)`
- `extractSessionTouchedFiles(messages)`
- `buildWebSearchViewModel(messages)`

Add only the fields the panel needs.

**Step 4: Run helper tests**

Run: `pnpm vitest run apps/runtime/src/components/chat-side-panel/view-model.test.ts`

Expected: PASS

### Task 3: Add backend workspace file listing and preview commands

**Files:**
- Create: `apps/runtime/src-tauri/tests/test_workspace_file_preview.rs`
- Modify: `apps/runtime/src-tauri/src/commands/dialog.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Create: `apps/runtime/src-tauri/src/commands/workspace_files.rs`

**Step 1: Write the failing backend tests**

Cover:

- listing files under a workspace recursively with stable sorting
- preview payload for text and markdown files
- preview payload for html with text source returned
- binary/docx fallback to metadata-only preview
- reject paths outside the configured workspace

**Step 2: Run test to verify it fails**

Run: `cargo test test_workspace_file_preview --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture`

Expected: FAIL because the command module does not exist.

**Step 3: Implement minimal backend commands**

Add commands:

- `list_workspace_files(workspace: String) -> Vec<...>`
- `read_workspace_file_preview(workspace: String, relative_path: String) -> ...`

Return a compact structure with:

- relative path
- file name
- size
- modified time
- preview kind: `text | markdown | html | binary`
- source content when safe and small enough

Reuse `open_external_url` without changing its semantics.

**Step 4: Run backend tests**

Run: `cargo test test_workspace_file_preview --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture`

Expected: PASS

### Task 4: Build the Current Task panel with TDD

**Files:**
- Create: `apps/runtime/src/components/chat-side-panel/TaskPanel.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing test**

Add UI assertions for:

- summary card with total, completed, in-progress
- highlighted current task title
- empty state when no todo exists
- badges summarizing file outputs and web searches

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx -t "current task"`

Expected: FAIL because the panel is missing.

**Step 3: Implement minimal component**

Render only the view-model fields needed by the tests. Keep styling compact and readable.

**Step 4: Run the focused test**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx -t "current task"`

Expected: PASS

### Task 5: Build the Files panel with list, filters, and preview

**Files:**
- Create: `apps/runtime/src/components/chat-side-panel/WorkspaceFilesPanel.tsx`
- Create: `apps/runtime/src/components/chat-side-panel/FilePreviewPane.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: Write the failing tests**

Cover:

- file list renders items returned by `list_workspace_files`
- session-touched files show a visual marker
- selecting `.md` shows rendered preview
- selecting `.html` allows switching between `页面预览` and `源码预览`
- empty preview state appears before selection

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx -t "files"`

Expected: FAIL because file browsing and preview do not exist.

**Step 3: Implement minimal files panel**

Wire `ChatView` to:

- fetch workspace file list when the panel opens or workspace changes
- fetch preview details on file selection
- keep preview selection state within the files panel

**Step 4: Run the focused test**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx -t "files"`

Expected: PASS

### Task 6: Build the Web Search panel with detail pane and safe open flow

**Files:**
- Create: `apps/runtime/src/components/chat-side-panel/WebSearchPanel.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: Write the failing tests**

Cover:

- multiple `web_search` calls produce multiple history items
- selecting a search shows parsed result details
- clicking a result opens a confirmation dialog
- confirming calls `open_external_url`

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx -t "web search"`

Expected: FAIL because the search panel and dialog are missing.

**Step 3: Implement minimal web-search panel**

Render:

- left-side search history list
- right-side detail pane
- confirmation dialog before opening a link

Keep parsing tolerant of inconsistent web-search output formats.

**Step 4: Run the focused test**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx -t "web search"`

Expected: PASS

### Task 7: Replace the old side panel shell in ChatView

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/ToolIsland.tsx`

**Step 1: Write the failing integration test**

Add a top-level test that exercises:

- opening the panel
- switching among `当前任务`, `文件`, `Web 搜索`
- absence of old side-panel content
- message-area `ToolIsland` still renders for assistant messages

**Step 2: Run test to verify it fails**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL on ChatView integration.

**Step 3: Implement minimal ChatView integration**

Refactor the right panel to:

- use the new tab labels
- remove route-events-driven content from the panel
- preserve message-area stream rendering
- preserve workspace loading already present in ChatView

Only touch `ToolIsland` if a small API change is required.

**Step 4: Run the integration test**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS

### Task 8: Run verification sweep

**Files:**
- No source changes required unless failures are found

**Step 1: Run frontend tests**

Run: `pnpm vitest run apps/runtime/src/components/chat-side-panel/view-model.test.ts apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx apps/runtime/src/components/__tests__/ChatView.risk-flow.test.tsx`

Expected: PASS

**Step 2: Run backend tests**

Run: `cargo test test_workspace_file_preview --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture`

Expected: PASS

**Step 3: Run a broader smoke check if time allows**

Run: `pnpm vitest run apps/runtime/src/components/__tests__/ChatView.*.test.tsx`

Expected: PASS or only unrelated pre-existing failures.

### Task 9: Final polish and review handoff

**Files:**
- Modify only if verification exposes issues

**Step 1: Review the UX against the design**

Check:

- task summary readability
- file preview empty/error states
- html preview/source switch clarity
- web search result open confirmation wording

**Step 2: If changes were needed, rerun focused tests**

Run only the smallest affected test commands first, then rerun the verification sweep.

**Step 3: Prepare review handoff**

Summarize:

- old panel capabilities removed
- new data sources introduced
- remaining limitations such as `.docx` preview fallback
