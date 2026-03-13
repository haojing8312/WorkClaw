# Chat Thinking Block Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a first-class `思考中` thinking block to chat replies that streams real reasoning content, keeps it separate from the final answer, and preserves it for history replay.

**Architecture:** Extend the runtime so providers can emit reasoning as a dedicated stream instead of mixing it into answer text, persist the resulting reasoning metadata alongside assistant replies, and upgrade the chat UI to render a composite assistant message with a collapsible `ThinkingBlock` above the main answer. Keep the existing high-level `agent-state-event` for lightweight status, but use dedicated reasoning events as the source of truth for expandable thought content.

**Tech Stack:** Rust, Tauri events, React, TypeScript, Vitest, Testing Library

---

### Task 1: Lock the target UX with failing frontend tests

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`
- Create or modify: `apps/runtime/src/components/__tests__/ChatView.thinking-block.test.tsx`

**Step 1: Write the failing test**

Add tests that assert:
- `思考中` appears when a thinking phase starts
- the expand control does not appear before reasoning text exists
- reasoning text can be expanded separately from the answer body
- completed reasoning shows `已思考 x.xs`
- historical messages with persisted reasoning render a collapsed thinking block

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- --run src/components/__tests__/ChatView.thinking-block.test.tsx`
Expected: FAIL because the current chat UI only shows the generic status banner and has no reasoning block.

**Step 3: Keep the failure focused**

If the failure is caused by unrelated setup issues, fix the test harness only. Do not implement production code in this task.

**Step 4: Run the focused tests again**

Run the same command and confirm the failure now points to the missing thinking-block behavior.

**Step 5: Commit**

Do not commit yet. This task establishes the red state only.

### Task 2: Add backend reasoning events without changing final-answer behavior

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Modify: `apps/runtime/src-tauri/src/adapters/openai.rs`
- Modify: `packages/runtime-chat-app/src/types.rs`
- Modify: `packages/runtime-chat-app/src/service.rs`
- Test: `apps/runtime/src-tauri/tests/test_chat_commands.rs`
- Test: `apps/runtime/src-tauri/src/adapters/openai.rs`

**Step 1: Write the failing backend tests**

Add tests that verify:
- reasoning output is emitted as dedicated reasoning events or structured payloads
- answer text continues to flow through the existing answer path
- provider responses containing `<think>`-style content or reasoning fields no longer merge reasoning into answer text

**Step 2: Run targeted tests to verify failure**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml openai -- --nocapture`
Expected: FAIL because the current implementation does not emit first-class reasoning events.

**Step 3: Write the minimal implementation**

- teach the provider adapter to separate reasoning from answer output
- add runtime event payloads for reasoning started, delta, completed, and interrupted
- keep existing answer token behavior intact

**Step 4: Run the targeted backend tests**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml openai -- --nocapture`
Expected: PASS for the new reasoning separation tests.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/executor.rs apps/runtime/src-tauri/src/adapters/openai.rs packages/runtime-chat-app/src/types.rs packages/runtime-chat-app/src/service.rs
git commit -m "feat(runtime): stream chat reasoning separately"
```

### Task 3: Persist reasoning with assistant replies for history replay

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_repo.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_session_io.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src/types.ts` or the shared frontend type source that defines `Message`
- Test: `apps/runtime/src-tauri/tests/test_chat_repo.rs`
- Test: `apps/runtime/src/__tests__/App.employee-assistant-update-flow.test.tsx`

**Step 1: Write the failing persistence tests**

Add tests that verify:
- an assistant message can store reasoning text, status, and duration
- reloading a session returns reasoning data along with the assistant answer
- messages without reasoning remain unchanged

**Step 2: Run targeted tests to verify failure**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_chat_repo -- --nocapture`
Expected: FAIL because the repository layer does not yet store reasoning metadata.

**Step 3: Write the minimal implementation**

- extend the message persistence model to include reasoning payload
- save completed or interrupted reasoning when a run ends
- return reasoning fields from message-loading commands

**Step 4: Run the targeted persistence tests**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_chat_repo -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat_repo.rs apps/runtime/src-tauri/src/commands/chat_session_io.rs apps/runtime/src-tauri/src/commands/chat.rs
git commit -m "feat(runtime): persist assistant reasoning metadata"
```

### Task 4: Build the ThinkingBlock UI and composite assistant message rendering

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Create: `apps/runtime/src/components/ThinkingBlock.tsx`
- Optional create: `apps/runtime/src/components/AssistantMessage.tsx`
- Modify: `apps/runtime/src/types.ts`
- Test: `apps/runtime/src/components/__tests__/ChatView.thinking-block.test.tsx`

**Step 1: Use the failing UI tests from Task 1**

Do not add new product behavior until the existing tests fail for the intended missing UI states.

**Step 2: Run the focused UI tests**

Run: `pnpm --filter runtime test -- --run src/components/__tests__/ChatView.thinking-block.test.tsx`
Expected: FAIL

**Step 3: Write the minimal implementation**

- subscribe to the new reasoning events in `ChatView`
- track in-memory reasoning state per assistant message
- render a collapsed `ThinkingBlock` above the answer
- enable expansion only when reasoning content exists
- display `思考中`, `已思考 x.xs`, and `思考中断` according to state

**Step 4: Run the focused UI tests**

Run: `pnpm --filter runtime test -- --run src/components/__tests__/ChatView.thinking-block.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/ThinkingBlock.tsx apps/runtime/src/types.ts apps/runtime/src/components/__tests__/ChatView.thinking-block.test.tsx
git commit -m "feat(ui): add collapsible chat thinking block"
```

### Task 5: Verify answer/reasoning separation and regression coverage

**Files:**
- Modify as needed: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`
- Modify as needed: `apps/runtime/src/components/__tests__/App.chat-landing.test.tsx`
- Verify: `apps/runtime/src/components/ToolIsland.tsx`

**Step 1: Add any missing regression assertions**

Cover these behaviors:
- answer text does not contain reasoning text
- failure states preserve received reasoning
- models without reasoning do not render an empty thinking block

**Step 2: Run the relevant frontend suite**

Run: `pnpm --filter runtime test -- --run src/components/__tests__/ChatView.session-resilience.test.tsx src/components/__tests__/ChatView.thinking-block.test.tsx`
Expected: PASS

**Step 3: Run the relevant backend suite**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib openai -- --nocapture`
Expected: PASS

**Step 4: Run a broader smoke pass**

Run: `pnpm --filter runtime test -- --run src/components/__tests__/App.chat-landing.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx apps/runtime/src/components/__tests__/App.chat-landing.test.tsx
git commit -m "test(ui): cover reasoning and answer separation"
```

### Task 6: Document the new chat reasoning capability

**Files:**
- Modify: `docs/plans/2026-03-13-chat-thinking-block-design.md`
- Modify: `docs/plans/2026-03-13-chat-thinking-block-implementation.md`
- Optional modify: `docs/development/windows-contributor-guide.md`

**Step 1: Add any implementation discoveries**

If event names, persistence fields, or provider boundaries changed during implementation, update the design and plan docs to match the shipped behavior.

**Step 2: Run a doc sanity check**

Run: `rg -n "思考中|reasoning|thinking block" docs/plans/2026-03-13-chat-thinking-block-design.md docs/plans/2026-03-13-chat-thinking-block-implementation.md`
Expected: both documents contain the final terminology and implementation notes.

**Step 3: Commit**

```bash
git add docs/plans/2026-03-13-chat-thinking-block-design.md docs/plans/2026-03-13-chat-thinking-block-implementation.md
git commit -m "docs: capture chat thinking block design and plan"
```
