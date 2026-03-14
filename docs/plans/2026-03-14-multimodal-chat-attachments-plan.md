# Multi-Attachment Vision And Text Files Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add multi-attachment chat input that supports image attachments and text-like file attachments, routes image-bearing turns to `vision`, and preserves attachments in session history.

**Architecture:** The chat composer will produce structured message parts instead of flattening attachments into prompt text. The Tauri runtime will persist structured user content, derive capability from message parts, fold text-file attachments into text context, and pass image parts through provider adapters as native multimodal content.

**Tech Stack:** React + TypeScript frontend, Tauri commands in Rust, SQLite-backed chat persistence, OpenAI-compatible and Anthropic-compatible provider adapters.

---

### Task 1: Define shared attachment and message-part types

**Files:**
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing type-level and UI usage updates**

Add explicit frontend types for:

```ts
export type PendingAttachment =
  | {
      id: string;
      kind: "image";
      name: string;
      mimeType: string;
      size: number;
      data: string;
      previewUrl: string;
    }
  | {
      id: string;
      kind: "text-file";
      name: string;
      mimeType: string;
      size: number;
      text: string;
      truncated?: boolean;
    };

export type ChatMessagePart =
  | { type: "text"; text: string }
  | { type: "image"; name: string; mimeType: string; size: number; data: string }
  | {
      type: "file_text";
      name: string;
      mimeType: string;
      size: number;
      text: string;
      truncated?: boolean;
    };

export type SendMessageRequest = {
  sessionId: string;
  parts: ChatMessagePart[];
};
```

Update `ChatView` imports/usages so old `FileAttachment` paths fail to compile where they no longer fit.

**Step 2: Run typecheck to verify breakage is real**

Run: `cmd.exe /c pnpm exec tsc --noEmit`

Expected: FAIL with errors around `FileAttachment`, `attachedFiles`, or `send_message` payload shape.

**Step 3: Implement the new shared types**

Replace the old flat attachment type with the new discriminated unions and update local state declarations in `ChatView` to use `PendingAttachment[]`.

**Step 4: Run typecheck again**

Run: `cmd.exe /c pnpm exec tsc --noEmit`

Expected: FAIL later in the send flow and rendering logic, but not on the new type declarations themselves.

**Step 5: Commit**

```bash
git add apps/runtime/src/types.ts apps/runtime/src/components/ChatView.tsx
git commit -m "refactor(chat): define structured attachment message types"
```

### Task 2: Add multi-attachment selection, validation, and preview in ChatView

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write failing ChatView tests**

Add tests for:

```tsx
it("renders multiple pending attachments and removes one by id", async () => {
  // attach two files, click remove on one, assert one remains
});

it("rejects unsupported attachment types and oversize files", async () => {
  // simulate file input with mp4/pdf or > limit files
});

it("shows mixed image and text-file attachment previews", async () => {
  // assert image thumbnail + text file card labels
});
```

**Step 2: Run the targeted frontend test**

Run: `cmd.exe /c pnpm exec vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL because the current composer only supports the old attachment flow.

**Step 3: Implement multi-attachment input**

In `ChatView.tsx`:
- Allow multi-select from the hidden file input.
- Accept only:
  - images: `png`, `jpg`, `jpeg`, `webp`
  - text/code/config files: `txt`, `md`, `json`, `yaml`, `yml`, `xml`, `csv`, `tsv`, `log`, `ini`, `conf`, `env`, `js`, `jsx`, `ts`, `tsx`, `py`, `rs`, `go`, `java`, `c`, `cpp`, `h`, `cs`, `sh`, `ps1`, `sql`
- Enforce limits:
  - max 5 attachments
  - max 3 images
  - image max 5MB each
  - text-file max 1MB each
- Read images as base64/data URL and text files as UTF-8 text.
- Store attachments in order and support remove-by-id.
- Render attachment list preview with:
  - thumbnail for images
  - file card for text files
  - truncated badge when applicable

**Step 4: Run the targeted frontend test again**

Run: `cmd.exe /c pnpm exec vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS for the new attachment interactions.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx
git commit -m "feat(chat): add multi-attachment composer previews"
```

### Task 3: Replace flattened attachment prompts with structured send payloads

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/types.ts`
- Test: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write failing send-payload tests**

Add tests for:

```tsx
it("sends text plus mixed attachment parts in user order", async () => {
  // expect invoke("send_message", { request: { sessionId, parts: [...] } })
});

it("injects default prompt when attachments exist and input is empty", async () => {
  // expect synthesized text part before attachment parts
});
```

**Step 2: Run the targeted test**

Run: `cmd.exe /c pnpm exec vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL because `ChatView` still sends `userMessage: string`.

**Step 3: Implement structured send assembly**

In `ChatView.tsx`:
- Delete the `fullContent` markdown-building path.
- Build `ChatMessagePart[]` from text + attachments.
- Generate default text for empty-input cases:
  - image only: `请结合这些图片描述主要内容，并提取可见文字。`
  - text files only: `请阅读这些附件并总结关键信息。`
  - mixed: `请结合这些图片和文本附件一起分析，并给出结论。`
- Send with:

```ts
await invoke("send_message", {
  request: {
    sessionId,
    parts,
  },
});
```

**Step 4: Re-run the targeted test**

Run: `cmd.exe /c pnpm exec vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS for structured payload assertions.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/types.ts apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx
git commit -m "refactor(chat): send structured attachment message parts"
```

### Task 4: Add structured chat request parsing and capability routing in Tauri

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_policy.rs`
- Modify: `packages/runtime-chat-app/src/service.rs`
- Test: `packages/runtime-chat-app/tests/capability.rs`
- Test: `packages/runtime-chat-app/tests/execution_assembly.rs`

**Step 1: Write failing backend tests**

Add tests for:

```rust
#[test]
fn infer_capability_prefers_vision_when_parts_contain_image() {
    // structured parts with image => "vision"
}

#[tokio::test]
async fn prepare_execution_folds_file_text_and_preserves_images() {
    // expect text context built from file_text and images carried separately
}
```

**Step 2: Run the targeted Rust/unit tests**

Run: `cmd.exe /c cargo test -p runtime-chat-app capability -- --nocapture`

Run: `cmd.exe /c cargo test -p runtime-chat-app execution_assembly -- --nocapture`

Expected: FAIL because current APIs only accept plain user-message strings.

**Step 3: Implement structured request handling**

In `chat.rs`:
- Introduce `SendMessageRequest` and Tauri-deserializable part structs.
- Accept `request` instead of `session_id` + `user_message`.
- Build:
  - `content` summary string for legacy UI/search
  - `content_json` serialized parts for persistence

In `chat_policy.rs` and `packages/runtime-chat-app/src/service.rs`:
- Add structured-part-aware capability inference:
  - any image part => `vision`
  - otherwise fallback to existing text inference

In `chat_send_message_flow.rs`:
- Convert `file_text` parts into a stable text block:

```text
附件文本文件：
## name.ext
```lang
content
```
```

- Preserve image parts as model image inputs.

**Step 4: Re-run the targeted tests**

Run: `cmd.exe /c cargo test -p runtime-chat-app capability -- --nocapture`

Run: `cmd.exe /c cargo test -p runtime-chat-app execution_assembly -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs apps/runtime/src-tauri/src/commands/chat_policy.rs packages/runtime-chat-app/src/service.rs packages/runtime-chat-app/tests/capability.rs packages/runtime-chat-app/tests/execution_assembly.rs
git commit -m "feat(runtime): route structured image turns to vision"
```

### Task 5: Persist structured user content and reconstruct attachment-aware history

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_runtime_io.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_session_io.rs`
- Modify: `apps/runtime/src/types.ts`
- Test: `apps/runtime/src-tauri/tests/test_chat_repo.rs`

**Step 1: Write failing persistence tests**

Add tests for:

```rust
#[tokio::test]
async fn stores_and_loads_content_json_for_user_attachment_messages() {
    // insert structured message, fetch it, assert content_json preserved
}
```

**Step 2: Run the targeted persistence test**

Run: `cmd.exe /c cargo test --test test_chat_repo -- --nocapture`

Expected: FAIL because structured content is not stored or loaded.

**Step 3: Implement storage and history reconstruction**

- Add/consume a `content_json` field in the session message model.
- Store serialized `ChatMessagePart[]` for user messages.
- Generate a backward-compatible summary string such as:
  - `[图片 2 张] [文本文件 1 个] 帮我分析`
- When loading message history, expose enough metadata for the frontend to render attachment cards again.

**Step 4: Re-run the targeted persistence test**

Run: `cmd.exe /c cargo test --test test_chat_repo -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat_runtime_io.rs apps/runtime/src-tauri/src/commands/chat_session_io.rs apps/runtime/src/types.ts apps/runtime/src-tauri/tests/test_chat_repo.rs
git commit -m "feat(chat): persist structured attachment content in history"
```

### Task 6: Add multimodal provider payload support for OpenAI-compatible and Anthropic-compatible backends

**Files:**
- Modify: `apps/runtime/src-tauri/src/adapters/openai.rs`
- Modify: `apps/runtime/src-tauri/src/adapters/anthropic.rs`
- Test: `apps/runtime/src-tauri/tests/test_registry.rs`
- Test: provider adapter tests near `apps/runtime/src-tauri/src/adapters`

**Step 1: Write failing adapter tests**

Add tests for:

```rust
#[test]
fn openai_adapter_builds_text_and_image_url_content_array() {
    // expect image parts => image_url data URI
}

#[test]
fn anthropic_adapter_builds_text_and_base64_image_blocks() {
    // expect image parts => source.base64 image block
}
```

**Step 2: Run the targeted adapter tests**

Run: `cmd.exe /c cargo test openai_adapter -- --nocapture`

Run: `cmd.exe /c cargo test anthropic_adapter -- --nocapture`

Expected: FAIL because adapters currently assume string content only.

**Step 3: Implement multimodal conversion**

In `openai.rs`:
- Accept internal model messages with `parts`.
- Convert image parts to:

```json
{ "type": "image_url", "image_url": { "url": "data:<mime>;base64,<data>" } }
```

In `anthropic.rs`:
- Convert image parts to:

```json
{
  "type": "image",
  "source": {
    "type": "base64",
    "media_type": "<mime>",
    "data": "<base64>"
  }
}
```

- Keep `file_text` content out of adapters by ensuring it has already been folded into text before this layer.

**Step 4: Re-run the targeted adapter tests**

Run: `cmd.exe /c cargo test openai_adapter -- --nocapture`

Run: `cmd.exe /c cargo test anthropic_adapter -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/adapters/openai.rs apps/runtime/src-tauri/src/adapters/anthropic.rs
git commit -m "feat(multimodal): add native image payloads for chat providers"
```

### Task 7: Render structured attachments in chat history

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write failing rendering tests**

Add tests for:

```tsx
it("renders stored image attachments in user history messages", async () => {
  // history item with image part => thumbnail
});

it("renders stored text-file attachments as file cards", async () => {
  // history item with file_text part => file card
});
```

**Step 2: Run the targeted frontend test**

Run: `cmd.exe /c pnpm exec vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: FAIL because history still renders from flat content only.

**Step 3: Implement attachment-aware history rendering**

In `ChatView.tsx`:
- Parse message `content_json` when present.
- Render:
  - user text block
  - image thumbnails for `image`
  - compact file cards for `file_text`
- Keep assistant/history rendering unchanged otherwise.

**Step 4: Re-run the targeted frontend test**

Run: `cmd.exe /c pnpm exec vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx
git commit -m "feat(chat): render structured attachments in session history"
```

### Task 8: Full verification and cleanup

**Files:**
- Modify: any touched files from prior tasks as needed

**Step 1: Run frontend verification**

Run: `cmd.exe /c pnpm exec vitest run apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

Expected: PASS.

**Step 2: Run backend verification**

Run: `cmd.exe /c cargo test -p runtime-chat-app -- --nocapture`

Run: `cmd.exe /c cargo test --test test_chat_repo -- --nocapture`

Expected: PASS.

**Step 3: Run repository typecheck/build verification**

Run: `cmd.exe /c pnpm exec tsc --noEmit`

Expected: PASS.

**Step 4: Manual sanity check**

Run the desktop app and verify:
- upload 2 images + 1 log file
- upload 3 text files without images
- remove one attachment before send
- send empty text with attachments
- history reload preserves attachments

**Step 5: Commit final fixes**

```bash
git add apps/runtime/src apps/runtime/src-tauri packages/runtime-chat-app docs/plans/2026-03-14-multimodal-chat-attachments-plan.md
git commit -m "feat(chat): support multimodal image and text-file attachments"
```
