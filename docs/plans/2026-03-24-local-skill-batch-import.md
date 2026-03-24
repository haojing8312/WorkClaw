# Local Skill Batch Import Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow the expert skill installer to import either a single local skill directory or a local skills root directory containing multiple skills, scanning at most one extra directory level.

**Architecture:** Keep directory selection in the React install dialog, but move batch discovery and aggregation into the Tauri backend so scan depth, partial-success handling, duplicate-name behavior, and MCP dependency aggregation all stay in one place. Preserve the existing local install entrypoint while expanding its response shape to support multiple imported skills.

**Tech Stack:** React, TypeScript, Vitest, Tauri, Rust, SQLx

---

### Task 1: Add failing Rust tests for batch local import

**Files:**
- Modify: `apps/runtime/src-tauri/tests/test_skill_commands.rs`
- Modify: `apps/runtime/src-tauri/tests/helpers/mod.rs` only if a reusable helper is needed
- Test: `pnpm test:rust-fast`

**Step 1: Add a test for importing a single selected skill directory**

Write a Rust test that calls the local import command/service with a directory containing `SKILL.md` and asserts:

- one installed result
- zero failed results
- installed manifest id/name match expectations

**Step 2: Add a test for importing multiple skills from a root directory**

Create a temporary directory shaped like:

- `skills/a/SKILL.md`
- `skills/b/SKILL.md`

Assert that selecting `skills/` imports both entries.

**Step 3: Add a test for one-level nested discovery**

Create a temporary directory shaped like:

- `skills/group-1/a/SKILL.md`
- `skills/group-2/b/SKILL.md`

Assert that selecting `skills/` discovers both nested skill directories.

**Step 4: Add a test proving deeper levels are ignored**

Create:

- `skills/group/deeper/c/SKILL.md`

Assert that selecting `skills/` does not import `c` and returns the expected “no importable skill found” or zero-discovery behavior for that structure.

**Step 5: Add a test for partial success**

Create one valid skill plus one duplicate-name or malformed skill and assert:

- valid skill imports
- invalid skill lands in `failed`
- command still returns success with one installed entry

**Step 6: Run the targeted Rust test file and confirm RED**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_skill_commands`

Expected: the new tests fail because batch import behavior and response shape do not exist yet.

### Task 2: Implement backend batch discovery and result aggregation

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/skills/types.rs`
- Modify: `apps/runtime/src-tauri/src/commands/skills/local_skill_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/skills.rs`

**Step 1: Add batch import result types**

Introduce result structs for:

- installed local skill entry
- failed local skill entry
- batch import response

Keep field names explicit and serialization-friendly for the frontend.

**Step 2: Add a helper that discovers importable skill directories**

Implement a helper in `local_skill_service.rs` that:

- accepts the selected root path
- returns the selected path itself if it contains `SKILL.md` or `skill.md`
- otherwise scans direct children and grandchildren only
- returns unique discovered skill directories in a stable order

**Step 3: Refactor the existing single-directory import logic into a reusable helper**

Keep the current single skill import path intact behind an internal helper so the batch path can call it repeatedly without duplicating manifest/MCP logic.

**Step 4: Implement the public batch-aware import function**

For each discovered directory:

- attempt import
- push successes into `installed`
- push failures into `failed`
- merge and dedupe `missing_mcp`

If nothing is discovered or everything fails, return an honest error aligned with the chosen UX.

**Step 5: Re-run the Rust test file and confirm GREEN**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_skill_commands`

Expected: the new and existing command tests pass.

### Task 3: Add failing frontend tests for local batch import UX

**Files:**
- Add or Modify: `apps/runtime/src/components/__tests__/InstallDialog.local-directory.test.tsx`
- Test: `pnpm --dir apps/runtime test -- --run src/components/__tests__/InstallDialog.local-directory.test.tsx`

**Step 1: Add a test for button copy and invoke payload**

Render `InstallDialog`, switch to “本地目录”, pick a directory, and assert:

- the chooser copy reflects “Skill 目录或 skills 根目录”
- clicking install invokes `import_local_skill` with the selected path

**Step 2: Add a test for multiple successful installs**

Mock the Tauri response with two `installed` items and assert:

- `onInstalled` is called with the first installed id
- the dialog closes

**Step 3: Add a test for partial success with missing MCP**

Mock one success, one failure, and non-empty `missing_mcp`, then assert:

- warning area renders the missing MCP names
- the dialog stays open if that is the current local-flow behavior after warning

**Step 4: Run the isolated frontend test file and confirm RED**

Run: `pnpm --dir apps/runtime test -- --run src/components/__tests__/InstallDialog.local-directory.test.tsx`

Expected: failures due to the old single-manifest response assumptions and old copy.

### Task 4: Update the install dialog to consume batch results

**Files:**
- Modify: `apps/runtime/src/components/InstallDialog.tsx`

**Step 1: Update local install result typing**

Replace the current single-manifest shape with the batch response shape.

**Step 2: Adjust local install success logic**

Handle:

- zero installed entries as an error
- one or more installed entries as success
- first installed skill id feeding the existing `onInstalled` callback
- missing MCP warnings reusing the existing warning UI

**Step 3: Update copy for local directory mode**

Change the picker label and descriptive text to explain:

- single skill directory import
- bulk root directory import
- max one extra level of scanning

**Step 4: Re-run the isolated frontend test file and confirm GREEN**

Run: `pnpm --dir apps/runtime test -- --run src/components/__tests__/InstallDialog.local-directory.test.tsx`

Expected: green.

### Task 5: Run focused verification for the changed surfaces

**Files:**
- Test only

**Step 1: Run Rust fast-path verification**

Run: `pnpm test:rust-fast`

Expected: changed Tauri command surface stays green.

**Step 2: Run the affected runtime component tests**

Run: `pnpm --dir apps/runtime test -- --run src/components/__tests__/InstallDialog.local-directory.test.tsx src/components/__tests__/InstallDialog.industry-pack.test.tsx`

Expected: local and neighboring install dialog tests pass.

**Step 3: If the runtime install flow wiring proves broader than expected, expand verification**

Run: `pnpm --dir apps/runtime test -- --run src/__tests__/App.experts-routing.test.tsx`

Expected: experts routing remains green if touched indirectly.
