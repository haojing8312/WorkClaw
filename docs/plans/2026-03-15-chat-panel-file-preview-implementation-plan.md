# Chat Panel File Preview Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Stabilize Markdown and HTML preview in the chat side panel files tab, while upgrading the fixed narrow side panel into a resizable drawer and making long file names readable.

**Architecture:** Keep the current `ChatWorkspaceSidePanel -> WorkspaceFilesPanel -> FilePreviewPane -> Tauri workspace file commands` flow. Upgrade the shell to a resizable drawer with a default width reset on every open, then improve only the file-list readability, preview metadata, HTML base-path handling, iframe safety, and user-facing fallback states needed for a v1 that prioritizes static preview over full page execution.

**Tech Stack:** React 18, TypeScript, Tauri, Rust, Vitest, Testing Library

---

### Task 1: Lock the drawer behavior and file-list readability in frontend tests

**Files:**
- Modify: `apps/runtime/src/components/chat-side-panel/ChatWorkspaceSidePanel.test.tsx`
- Modify: `apps/runtime/src/components/chat-side-panel/WorkspaceFilesPanel.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing tests**

Add or extend assertions for:

- the side panel opens with the default drawer width
- the drawer width can be resized within a min/max range
- closing and reopening resets back to the default width
- long filenames preserve visible file type information
- file items expose the full filename or relative path on hover
- `.md` opens in `渲染预览` by default
- `.md` can switch to `源码预览`
- `.html` opens in `页面预览` by default
- `.html` can switch to `源码预览`
- truncated content hint is shown when preview payload says it was truncated
- html preview fallback notice is shown when preview payload carries a preview error

**Step 2: Run the focused frontend tests to verify they fail**

Run: `pnpm --dir apps/runtime exec vitest run src/components/chat-side-panel/WorkspaceFilesPanel.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: FAIL because the current drawer shell and files UI do not expose resize, reset, or richer filename behavior.

**Step 3: Make minimal mock updates**

Update mocked `read_workspace_file_preview` responses in both test files so they can cover:

- long filename entries for `.md` and `.html`
- normal markdown preview
- normal html preview
- truncated preview
- html preview with fallback error message

**Step 4: Run the focused frontend tests again**

Run: `pnpm --dir apps/runtime exec vitest run src/components/chat-side-panel/WorkspaceFilesPanel.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: FAIL only on missing implementation, not on broken test setup.

### Task 2: Add preview metadata coverage to backend tests first

**Files:**
- Modify: `apps/runtime/src-tauri/tests/test_workspace_file_preview.rs`

**Step 1: Write the failing backend tests**

Add tests for:

- markdown preview returns `kind = "markdown"`
- html preview returns `kind = "html"`
- large text preview marks `truncated = true`
- binary preview keeps `source = None`
- path traversal is still rejected

**Step 2: Run the backend test to verify it fails**

Run: `cargo test test_workspace_file_preview --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture`
Expected: FAIL because `WorkspaceFilePreview` does not yet include truncation metadata.

**Step 3: Keep the fixture set minimal**

Extend the temporary workspace fixture with one large markdown or html file over `256 KB` so the truncation path is deterministic.

**Step 4: Run the backend test again**

Run: `cargo test test_workspace_file_preview --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture`
Expected: FAIL only on missing implementation.

### Task 3: Extend backend preview payload with truncation and preview error metadata

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/workspace_files.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`

**Step 1: Implement the minimal Rust shape change**

Update `WorkspaceFilePreview` to include:

- `truncated: bool`
- `preview_error: Option<String>`

Set:

- `truncated = true` when file bytes exceed `MAX_PREVIEW_BYTES`
- `preview_error = None` on normal reads
- keep existing path guard and binary behavior

For the initial implementation:

- do not add new commands
- do not change file listing behavior
- keep markdown/html/text source reading logic simple

**Step 2: Run the backend test to verify it passes**

Run: `cargo test test_workspace_file_preview --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture`
Expected: PASS

**Step 3: Run a targeted Rust formatting check if practical**

Run: `cargo fmt --manifest-path apps/runtime/src-tauri/Cargo.toml --all`
Expected: PASS

### Task 4: Upgrade ChatWorkspaceSidePanel into a resizable drawer

**Files:**
- Modify: `apps/runtime/src/components/chat-side-panel/ChatWorkspaceSidePanel.tsx`
- Modify only if needed: `apps/runtime/src/components/ChatView.tsx`

**Step 1: Write the minimal drawer implementation**

Replace the fixed `320px` side panel behavior with a resizable drawer that:

- opens at a fixed default width every time
- allows pointer-based resizing within a min/max width
- resets to the default width whenever it reopens

Keep:

- the existing tabs
- the existing open/close entry point
- the existing panel content routing

**Step 2: Avoid persistence in v1**

Do not store drawer width in local storage, session state, or settings. The width change should last only for the current open cycle.

**Step 3: Run the drawer-focused test**

Run: `pnpm --dir apps/runtime exec vitest run src/components/chat-side-panel/ChatWorkspaceSidePanel.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

### Task 5: Improve WorkspaceFilesPanel file-row readability

**Files:**
- Modify: `apps/runtime/src/components/chat-side-panel/WorkspaceFilesPanel.tsx`
- Modify: `apps/runtime/src/components/chat-side-panel/WorkspaceFilesPanel.test.tsx`

**Step 1: Implement the minimal file-row redesign**

Update file items so they:

- preserve extension visibility
- show a file-type badge such as `MD` or `HTML`
- keep file size visible
- keep the `本轮生成` marker
- expose the full filename and relative path via hover title or equivalent accessible tooltip text

Prefer a small helper for splitting basename and extension rather than introducing a large formatting utility layer.

**Step 2: Keep the panel structure stable**

Do not redesign the search box, tree recursion, or selection model. This task is only about row readability and hover disclosure.

**Step 3: Run the focused panel test**

Run: `pnpm --dir apps/runtime exec vitest run src/components/chat-side-panel/WorkspaceFilesPanel.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

### Task 6: Harden FilePreviewPane for v1 Markdown and HTML preview

**Files:**
- Modify: `apps/runtime/src/components/chat-side-panel/FilePreviewPane.tsx`

**Step 1: Write the minimal UI implementation**

Update `WorkspaceFilePreview` frontend type to match backend metadata and then implement:

- truncation hint banner when `preview.truncated === true`
- preview error banner when `preview.previewError` exists
- markdown default rendered/source toggle unchanged
- html default page/source toggle unchanged
- `iframe` gets an explicit `sandbox` attribute appropriate for static preview

Also improve HTML base-path injection so relative assets resolve against the selected HTML file directory rather than an incorrect parent path.

**Step 2: Keep the HTML strategy intentionally narrow**

Do not attempt:

- a local preview web server
- script execution parity with a normal browser tab
- complex resource rewriting beyond base-path correction

**Step 3: Run the focused frontend tests**

Run: `pnpm --dir apps/runtime exec vitest run src/components/chat-side-panel/WorkspaceFilesPanel.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

### Task 7: Keep WorkspaceFilesPanel selection defaults and preview flow stable

**Files:**
- Modify: `apps/runtime/src/components/chat-side-panel/WorkspaceFilesPanel.tsx`

**Step 1: Verify and tighten default preview mode logic**

Ensure:

- markdown still defaults to `rendered`
- html still defaults to `rendered`
- all other text files default to `source`

If needed, keep selection stable when preview metadata changes and avoid resetting the selected file unnecessarily.

**Step 2: Surface preview-state messages without changing panel structure**

Do not redesign the files panel. Only keep the current:

- left file tree
- right preview pane
- search box
- open/copy actions

**Step 3: Run the focused panel test**

Run: `pnpm --dir apps/runtime exec vitest run src/components/chat-side-panel/WorkspaceFilesPanel.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

### Task 8: Verify the chat-level side panel integration still works

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
- Modify only if needed: `apps/runtime/src/components/ChatView.tsx`

**Step 1: Run the chat-side integration test**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

**Step 2: Only make integration changes if the new metadata requires them**

Prefer not to touch `ChatView.tsx` unless:

- typing changes require it
- mock wiring or state assumptions break under the richer preview payload

**Step 3: Re-run the chat-side integration test**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

### Task 9: Run a targeted verification sweep before claiming completion

**Files:**
- No source changes unless failures are found

**Step 1: Run frontend verification**

Run: `pnpm --dir apps/runtime exec vitest run src/components/chat-side-panel/ChatWorkspaceSidePanel.test.tsx src/components/chat-side-panel/WorkspaceFilesPanel.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx --pool forks --poolOptions.forks.singleFork`
Expected: PASS

**Step 2: Run backend verification**

Run: `cargo test test_workspace_file_preview --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture`
Expected: PASS

**Step 3: Run a lightweight type/build smoke check if practical**

Run: `pnpm --dir apps/runtime exec tsc --noEmit`
Expected: PASS

### Task 10: Review scope and handoff

**Files:**
- Modify docs only if verification exposes a scope mismatch

**Step 1: Confirm the v1 boundary remains intact**

Check that the implementation still follows:

- side panel opens with the same default drawer width each time
- drawer resizing is temporary, not persisted
- Markdown: rendered + source preview
- HTML: page + source preview
- static preview prioritized over full browser behavior
- no new preview server introduced

**Step 2: Summarize remaining limitations in the handoff**

Document the remaining limitations clearly:

- complex JS-driven pages may not fully behave inside preview
- some relative resource cases may still require opening the file externally
- large files are previewed only partially
- drawer width resets on every open by design

**Step 3: Prepare execution handoff**

Plan for execution with `superpowers:executing-plans` or `superpowers:subagent-driven-development` once implementation starts.
