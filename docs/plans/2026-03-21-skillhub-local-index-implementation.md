# SkillHub Local Index Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the Expert Skills library open instantly from a local SkillHub index with stable popularity ordering, while keeping Find Skills on real-time network recommendations.

**Architecture:** Add a local SQLite-backed SkillHub catalog index in the Tauri layer and refresh it asynchronously on a 6-hour cadence. The library view will read paged data from that local index and append items without re-sorting on the client, while Find Skills will continue using live network recommendations with local fallback only when needed later.

**Tech Stack:** Tauri Rust, SQLx SQLite, React, Vitest, existing WorkClaw cache and command wiring.

---

### Task 1: Add failing Rust tests for local index behavior

**Files:**
- Modify: `apps/runtime/src-tauri/tests/test_clawhub_library_cache.rs`
- Create: `apps/runtime/src-tauri/tests/test_skillhub_index_sync.rs`

**Step 1: Write the failing tests**

- Add a test proving SkillHub catalog items are globally sorted by `downloads desc, stars desc, name asc` before pagination.
- Add a test proving the sync path stores normalized SkillHub records locally and returns metadata with a `last_synced_at` timestamp.
- Add a test proving stale local index data is still returned when refresh fails.

**Step 2: Run tests to verify they fail**

Run: `pnpm test:rust-fast -- --test test_clawhub_library_cache --test test_skillhub_index_sync`

Expected: FAIL because the local SkillHub index table and sync functions do not exist yet.

**Step 3: Commit**

```bash
git add apps/runtime/src-tauri/tests/test_clawhub_library_cache.rs apps/runtime/src-tauri/tests/test_skillhub_index_sync.rs
git commit -m "test: define skillhub local index behavior"
```

### Task 2: Implement local SkillHub index storage and sync in Rust

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Modify: `apps/runtime/src-tauri/src/commands/clawhub.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`

**Step 1: Add schema and migration support**

- Create a `skillhub_catalog_index` table for normalized catalog rows.
- Create a lightweight sync metadata table or reuse a dedicated cache key for `last_synced_at` and sync status.

**Step 2: Implement sync helpers**

- Add helpers to fetch the full SkillHub catalog, normalize it, and replace the local index in one transaction.
- Add a helper that checks whether the 6-hour TTL has expired.
- Add a non-blocking refresh trigger that runs in the background and does not block app startup or page rendering.

**Step 3: Implement local-library query path**

- Update `list_clawhub_library_with_pool` to read from the local index first.
- Return a stable `next_cursor` plus sync metadata for the UI.
- Keep the existing ClawHub HTTP fallback only for first-run/no-index scenarios.

**Step 4: Expose refresh hooks**

- Add commands or helper paths that allow the UI to trigger a manual refresh and to kick off a silent refresh when the app starts or the experts area opens.

**Step 5: Run Rust tests**

Run: `pnpm test:rust-fast -- --test test_clawhub_library_cache --test test_skillhub_index_sync`

Expected: PASS

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/commands/clawhub.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_clawhub_library_cache.rs apps/runtime/src-tauri/tests/test_skillhub_index_sync.rs
git commit -m "feat: add local skillhub catalog index"
```

### Task 3: Add failing frontend tests for stable library rendering

**Files:**
- Modify: `apps/runtime/src/components/experts/__tests__/SkillLibraryView.translation.test.tsx`

**Step 1: Write the failing tests**

- Add a test proving the library view preserves server order instead of re-sorting appended pages.
- Add a test proving the sync status text is shown from backend metadata.
- Add a test proving the manual refresh button calls the refresh command and keeps existing items visible.

**Step 2: Run tests to verify they fail**

Run: `pnpm vitest apps/runtime/src/components/experts/__tests__/SkillLibraryView.translation.test.tsx`

Expected: FAIL because the component does not render sync metadata or manual refresh controls yet.

**Step 3: Commit**

```bash
git add apps/runtime/src/components/experts/__tests__/SkillLibraryView.translation.test.tsx
git commit -m "test: define stable skill library ui behavior"
```

### Task 4: Implement the library UI changes

**Files:**
- Modify: `apps/runtime/src/components/experts/SkillLibraryView.tsx`
- Modify: `apps/runtime/src/components/experts/ExpertsView.tsx`
- Modify: `apps/runtime/src/App.tsx`

**Step 1: Update library response handling**

- Extend the response type with sync metadata.
- Stop client-side popularity sorting.
- Append unique items in backend order only.

**Step 2: Add sync affordances**

- Show last sync time in the library header.
- Add a manual refresh button that triggers a background refresh command and reloads the first page afterward.

**Step 3: Trigger silent refresh at the right time**

- Kick off a background refresh when WorkClaw opens or when the experts area becomes active, without blocking navigation.

**Step 4: Run frontend tests**

Run: `pnpm vitest apps/runtime/src/components/experts/__tests__/SkillLibraryView.translation.test.tsx`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/experts/SkillLibraryView.tsx apps/runtime/src/components/experts/ExpertsView.tsx apps/runtime/src/App.tsx apps/runtime/src/components/experts/__tests__/SkillLibraryView.translation.test.tsx
git commit -m "feat: stabilize expert skills library browsing"
```

### Task 5: Verify the integrated behavior

**Files:**
- Modify: `apps/runtime/e2e/skill-library-cache.spec.ts`

**Step 1: Add or update e2e coverage**

- Cover cached-library startup behavior.
- Cover stable append behavior across multiple pages.
- Cover manual refresh keeping the library usable during refresh.

**Step 2: Run verification**

Run: `pnpm test:rust-fast`
Expected: PASS

Run: `pnpm test:e2e:runtime`
Expected: PASS

**Step 3: Commit**

```bash
git add apps/runtime/e2e/skill-library-cache.spec.ts
git commit -m "test: verify local skillhub library experience"
```
