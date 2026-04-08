# WorkClaw OEM Branding Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make OEM branding configurable from one config file or one build flag while keeping the default brand as `workclaw`, and ensure brand-driven runtime root paths plus de-branded user-facing storage copy.

**Architecture:** Extend the existing `apply-brand` build-time pipeline into a single OEM entrypoint that resolves the active brand from CLI, config, or fallback default. Generate one frontend branding module and one Rust branding module from the same brand manifest so packaging, runtime root paths, bootstrap storage, autostart identity, and user-visible labels stay aligned.

**Tech Stack:** Node.js build scripts, React + Vitest, Tauri Rust runtime, existing WorkClaw branding manifests.

---

### Task 1: Normalize workspace state before OEM work

**Files:**
- Modify: `apps/runtime/package.json`
- Modify: `apps/runtime/src-tauri/Cargo.toml`
- Modify: `.github/release-windows-notes.md`

**Step 1: Restore the paused release-only changes**

Run: `git restore -- apps/runtime/package.json apps/runtime/src-tauri/Cargo.toml .github/release-windows-notes.md`

**Step 2: Verify only OEM-related changes remain**

Run: `git status --short`
Expected: branding/script/generated-brand files remain; paused release version/doc files are clean.

### Task 2: Add a single OEM brand selector contract

**Files:**
- Create: `branding/brand-selection.json`
- Modify: `scripts/apply-brand.mjs`
- Test: `scripts/apply-brand.test.mjs`

**Step 1: Write failing tests**

Add tests that prove:
- no CLI/config/env override resolves to `workclaw`
- config file can select `bifclaw`
- CLI `--brand bifclaw` overrides config

**Step 2: Run targeted tests to verify failure**

Run: `node --test scripts/apply-brand.test.mjs`
Expected: new OEM-selection tests fail before implementation.

**Step 3: Write minimal implementation**

Implement brand resolution priority:
- `--brand`
- `WORKCLAW_BRAND`
- `branding/brand-selection.json`
- fallback `workclaw`

**Step 4: Run tests to verify pass**

Run: `node --test scripts/apply-brand.test.mjs`
Expected: pass.

### Task 3: Generate a shared Rust branding module from the same brand manifest

**Files:**
- Modify: `scripts/apply-brand.mjs`
- Test: `scripts/apply-brand.test.mjs`
- Create: `apps/runtime/src-tauri/src/branding_generated.rs`

**Step 1: Write failing tests**

Assert `apply-brand` writes a Rust branding module containing:
- brand key
- product name
- bundle identifier
- local storage prefix
- protocol scheme
- runtime root dir name

**Step 2: Run tests to verify failure**

Run: `node --test scripts/apply-brand.test.mjs`
Expected: fail because the Rust branding file is missing.

**Step 3: Write minimal implementation**

Generate `branding_generated.rs` from the manifest and selection result.

**Step 4: Run tests to verify pass**

Run: `node --test scripts/apply-brand.test.mjs`
Expected: pass.

### Task 4: Make runtime root, bootstrap, and default work dir brand-driven

**Files:**
- Modify: `apps/runtime/src-tauri/src/runtime_paths.rs`
- Modify: `apps/runtime/src-tauri/src/runtime_bootstrap.rs`
- Modify: `apps/runtime/src-tauri/src/commands/runtime_preferences/service.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/repo.rs`
- Modify: `apps/runtime/src-tauri/src/commands/runtime_preferences/types.rs`
- Test: `apps/runtime/src-tauri/src/runtime_paths.rs`
- Test: `apps/runtime/src-tauri/src/runtime_bootstrap.rs`
- Test: `apps/runtime/src-tauri/src/commands/runtime_preferences/service.rs`

**Step 1: Write failing tests**

Add coverage that OEM brand constants produce:
- default runtime root `C:\Users\<user>\.bifclaw`
- bootstrap store under OEM bundle identifier
- default workspace under OEM root, not `WorkClaw\workspace`

**Step 2: Run targeted Rust tests to verify failure**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml runtime_paths -- --nocapture`
Expected: new tests fail before implementation.

**Step 3: Write minimal implementation**

Move path-name derivation to shared branding constants and keep fallback behavior stable.

**Step 4: Run targeted Rust tests to verify pass**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml runtime_paths -- --nocapture`
Expected: pass.

### Task 5: Remove hard-coded product names from high-priority storage UI

**Files:**
- Modify: `apps/runtime/src/components/settings/desktop/DesktopSettingsSection.tsx`
- Test: `apps/runtime/src/components/__tests__/SettingsView.data-retention.test.tsx`
- Test: `apps/runtime/src/components/__tests__/SettingsView.desktop-system-tab.test.tsx`

**Step 1: Write failing tests**

Change expectations so the settings UI says:
- `数据根目录`
- `选择数据根目录`
- uninstall warning without `WorkClaw`

**Step 2: Run targeted frontend tests to verify failure**

Run: `pnpm --filter runtime test -- src/components/__tests__/SettingsView.data-retention.test.tsx src/components/__tests__/SettingsView.desktop-system-tab.test.tsx`
Expected: fail before implementation.

**Step 3: Write minimal implementation**

Update only the high-priority user-facing copy to brand-neutral wording.

**Step 4: Run targeted frontend tests to verify pass**

Run: `pnpm --filter runtime test -- src/components/__tests__/SettingsView.data-retention.test.tsx src/components/__tests__/SettingsView.desktop-system-tab.test.tsx`
Expected: pass.

### Task 6: Re-materialize default brand outputs and verify release-sensitive behavior

**Files:**
- Modify: `apps/runtime/src/branding.generated.ts`
- Modify: `apps/runtime/src-tauri/tauri.conf.json`
- Modify: `apps/runtime/src-tauri/icons/**/*`

**Step 1: Re-apply the default brand**

Run: `node scripts/apply-brand.mjs`
Expected: generated outputs return to default `workclaw`.

**Step 2: Run verification**

Run:
- `pnpm test:apply-brand`
- `pnpm test:installer`
- `pnpm test:rust-fast`
- `pnpm build:runtime`

Expected: pass, with installers defaulting to `workclaw` unless `--brand` or config selects another brand.
