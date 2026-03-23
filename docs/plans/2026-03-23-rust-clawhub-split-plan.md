# Rust ClawHub Split Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Turn `apps/runtime/src-tauri/src/commands/clawhub.rs` into the next formal Rust command-splitting template by extracting public DTOs and a narrow pure-helper layer first, then following up with the heavier SQL, download, and translation flows in later batches.

**Architecture:** Keep the current Tauri command interface and sibling-call surface stable. The root file should end as a thin shell that owns macro-visible command wrappers, small compatibility glue, and public re-exports. The first batch only extracts the safest static helpers so the root file stays behaviorally stable while the module boundary is established.

**Tech Stack:** Rust, Tauri commands, sqlx, SQLite, reqwest, WorkClaw runtime tests

---

## Status

This implementation plan has been executed through the fifth batch.

Completed batches:

- `types.rs`
- `support.rs`
- `repo.rs`
- `search_service.rs`
- `detail_service.rs`
- `translation_service.rs`
- `download_service.rs`
- `install_service.rs`

The completed batches now include install/update orchestration, so the root file has effectively converged to a thin shell.

---

### Task 1: Create the ClawHub module skeleton

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/clawhub/types.rs`
- Create: `apps/runtime/src-tauri/src/commands/clawhub/support.rs`
- Modify: `apps/runtime/src-tauri/src/commands/clawhub.rs`

**Step 1: Add module declarations**

- Declare the two new child modules from the root command file
- Re-export the public DTOs from `types.rs`
- Re-export the narrow helper layer from `support.rs` for the root module and existing tests

**Step 2: Move the public DTOs**

- Move the ClawHub and SkillHub DTOs plus the small catalog index row struct into `types.rs`
- Keep field names and serde shapes unchanged

**Step 3: Run a compile check**

Run: `cargo check --manifest-path apps/runtime/src-tauri/Cargo.toml`
Expected: PASS

Status: completed

---

### Task 2: Extract the narrow pure-helper layer

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/clawhub.rs`
- Modify: `apps/runtime/src-tauri/src/commands/clawhub/support.rs`

**Step 1: Move the low-risk helpers**

- Move:
  - ClawHub and SkillHub base URL helpers
  - download URL builders
  - cursor, limit, sort, and cache-key helpers
  - slug sanitization
  - small catalog text/tag helpers
  - sync staleness helper

**Step 2: Keep the behavior identical**

- Preserve the current cache key format
- Preserve the current default values and normalization behavior
- Do not touch download, translation, or SQL code yet

**Step 3: Run focused regression tests**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_clawhub_library_cache -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_clawhub_search -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_skillhub_index_sync -- --nocapture`

Expected: PASS

Status: partially completed

Notes:
- `test_clawhub_library_cache` passed
- `test_clawhub_search` is currently blocked by an unrelated unclosed delimiter in `apps/runtime/src-tauri/src/commands/chat_runtime_io/workspace_skills.rs`
- `test_skillhub_index_sync` has not been rerun yet because the crate-level compile is blocked by the same unrelated issue

---

### Task 3: Verify the first batch and record follow-on split targets

**Files:**
- No new code files expected
- Update: `apps/runtime/src-tauri/src/commands/clawhub.rs` only if a minor import cleanup is needed after the extraction

**Step 1: Run the Rust fast path**

Run: `pnpm test:rust-fast`
Expected: PASS

**Step 2: Inspect the remaining file shape**

- Confirm the root file is still stable for callers
- Record which remaining concern should be split next:
  - `repo.rs`
  - `search_service.rs`
  - `detail_service.rs`
  - `translation_service.rs`
  - `download_service.rs`
  - `install_service.rs`

**Step 3: Stop after the first batch**

- Do not start the heavier SQL or download extraction in this pass
- Leave the follow-on split as the next checkpoint

Status: pending until the crate-level blocker in `chat_runtime_io/workspace_skills.rs` is resolved

---

### Task 4: Extract search and detail orchestration

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/clawhub/search_service.rs`
- Create: `apps/runtime/src-tauri/src/commands/clawhub/detail_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/clawhub.rs`
- Modify: `apps/runtime/src-tauri/tests/test_clawhub_search.rs`

**Step 1: Move search and recommendation logic**

- Move catalog search, recommendation shaping, and library-response helpers into `search_service.rs`
- Keep the root file as the macro-visible Tauri wrapper

**Step 2: Move detail lookup and repo resolution**

- Move detail cache lookup, search fallback, repo URL resolution, and detail normalization flow into `detail_service.rs`
- Keep install/update callers stable by preserving the root-level call surface

**Step 3: Re-run focused ClawHub verification**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_clawhub_library_cache -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_clawhub_search -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_skillhub_index_sync -- --nocapture`
- `pnpm test:rust-fast`

Expected: PASS

Status: completed

Notes:
- `test_clawhub_search` now passes after pinning `SKILLHUB_CATALOG_URL` in the test to avoid falling through to the real remote catalog
- `test_clawhub_library_cache`, `test_clawhub_search`, `test_skillhub_index_sync`, and `pnpm test:rust-fast` all passed after the second batch
- Root `clawhub.rs` is now about `1092` lines, down from `2469`

---

### Task 5: Follow-on split targets

**Next candidates:**
- final command-shell cleanup

Status: pending

---

### Task 6: Extract translation orchestration

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/clawhub/translation_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/clawhub.rs`

**Step 1: Move translation helpers**

- Move language normalization, translation-engine selection, Google fallback parsing, and model translation orchestration into `translation_service.rs`
- Keep the root file as the public wrapper for `translate_texts_with_preferences_with_pool`

**Step 2: Preserve cache behavior**

- Keep the existing translation cache-key shape
- Keep the same `immersive_translation_enabled`, engine mode, and fallback behavior

**Step 3: Re-run focused translation verification**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_clawhub_translation_preferences -- --nocapture`
- `pnpm test:rust-fast`

Expected: PASS

Status: completed

Notes:
- Existing translation preference tests stayed green after the extraction
- Root `clawhub.rs` is now about `798` lines, which brings it inside the current `<= 800` governance target

---

### Task 7: Extract download and archive helpers

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/clawhub/download_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/clawhub.rs`
- Modify: `apps/runtime/src-tauri/src/commands/clawhub/detail_service.rs`

**Step 1: Move archive download and extraction helpers**

- Move GitHub archive URL construction, SkillHub/ClawHub fallback download logic, zip extraction, and repo discovery helpers into `download_service.rs`
- Keep root public helper names stable for external callers such as the GitHub repo tool

**Step 2: Preserve install/update call seams**

- Keep `install_clawhub_skill`, `check_clawhub_skill_update`, and `install_github_skill_repo` behavior unchanged
- Preserve detail-service repo URL validation by reusing the moved archive URL helper

**Step 3: Re-run focused verification**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib build_github_archive_urls_supports_standard_repo_urls -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib extract_repo_url_from_detail_body_prefers_direct_and_falls_back_to_owner_handle -- --nocapture`
- `pnpm test:rust-fast`

Expected: PASS

Status: completed

Notes:
- Root `clawhub.rs` is now about `402` lines after moving the download/archive layer
- `install/update` orchestration is now the main remaining concern to extract

---

### Task 8: Extract install and update orchestration

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/clawhub/install_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/clawhub.rs`

**Step 1: Move install/update orchestration**

- Move `install_clawhub_skill`, `install_github_skill_repo`, `check_clawhub_skill_update`, and `update_clawhub_skill` into `install_service.rs`
- Keep root Tauri command names and return types unchanged

**Step 2: Preserve existing seams**

- Keep manifest generation, MCP dependency checks, local hash comparison, and GitHub repo import behavior unchanged
- Keep root exports stable so command registration and external callers do not need to move

**Step 3: Re-run focused verification**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib parse_clawhub_skill_id_accepts_valid_skill_id -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib parse_clawhub_skill_id_rejects_invalid_prefix -- --nocapture`
- `pnpm test:rust-fast`

Expected: PASS

Status: completed

Notes:
- Added a tiny install-service unit seam around `parse_clawhub_skill_id` so the extracted module has direct coverage
- Root `clawhub.rs` is now about `266` lines, down from `2469`
- At this point the remaining work is optional shell polish rather than major responsibility extraction
