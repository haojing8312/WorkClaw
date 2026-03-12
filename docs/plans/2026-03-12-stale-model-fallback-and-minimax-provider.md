# Stale Model Fallback And MiniMax Provider Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prevent chats from failing silently when a session references a deleted model, and preserve the MiniMax provider identity when using proxy base URLs.

**Architecture:** Add a backend fallback path in chat execution preparation so a session with a stale `model_id` can resolve to the current default usable model instead of aborting before routing. Keep provider identity stable by allowing the frontend catalog selection to drive `provider_key`, rather than inferring it only from the base URL.

**Tech Stack:** React, TypeScript, Tauri, Rust, sqlx, Vitest, cargo test

---

### Task 1: Document fallback behavior in tests

**Files:**
- Modify: `packages/runtime-chat-app/tests/route_candidates.rs`
- Test: `packages/runtime-chat-app/tests/route_candidates.rs`

**Step 1: Write the failing test**

Add a case where `load_session_model()` fails for the requested `model_id`, while `resolve_default_usable_model_id()` and a second `load_session_model()` for the default model succeed. Assert that route preparation still returns a candidate from the fallback model instead of returning an error.

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime-chat-app stale_model -- --nocapture`
Expected: FAIL because the service currently aborts on the missing session model.

**Step 3: Write minimal implementation**

Update chat preparation logic to retry with the default usable model when the requested model no longer exists.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime-chat-app stale_model -- --nocapture`
Expected: PASS

### Task 2: Cover MiniMax provider key preservation

**Files:**
- Modify: `apps/runtime/src/__tests__/model-provider-catalog.test.ts`
- Modify: `apps/runtime/src/components/__tests__/SettingsView.model-providers.test.tsx`
- Test: `apps/runtime/src/components/__tests__/SettingsView.model-providers.test.tsx`

**Step 1: Write the failing test**

Add a UI-level save test showing that when the user selects the official `minimax-openai` preset and overrides the base URL with a proxy endpoint, the synced provider config still uses `provider_key: "minimax"`.

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- SettingsView.model-providers.test.tsx`
Expected: FAIL because provider key is currently inferred from the proxy URL and becomes `openai`.

**Step 3: Write minimal implementation**

Carry the selected catalog provider key through the save flow and use it when syncing provider configs.

**Step 4: Run test to verify it passes**

Run: `pnpm --filter runtime test -- SettingsView.model-providers.test.tsx`
Expected: PASS

### Task 3: Verify end-to-end regression surface

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_repo.rs`
- Modify: `packages/runtime-chat-app/src/service.rs`
- Modify: `apps/runtime/src/components/SettingsView.tsx`

**Step 1: Run focused backend tests**

Run: `cargo test -p runtime-chat-app route_candidates -- --nocapture`
Expected: PASS

**Step 2: Run focused frontend tests**

Run: `pnpm --filter runtime test -- model-provider-catalog.test.ts SettingsView.model-providers.test.tsx`
Expected: PASS

**Step 3: Sanity-check no behavior regression**

Confirm that existing route preparation still prefers explicit route policy providers and only falls back when the session model lookup fails.

