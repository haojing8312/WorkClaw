# WorkClaw Logo Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace all tracked WorkClaw logo assets used by the desktop app, bundle icons, and README with the new designer-provided logo set.

**Architecture:** Keep every existing asset path stable and overwrite the tracked binary assets in place. Use the designer-provided transparent PNG and ICO files as the canonical source, generate the required PNG/icon derivatives for Tauri packaging, then verify that every known reference still resolves to an existing file.

**Tech Stack:** PowerShell, existing Windows shell utilities, image generation tooling available in the local environment, Tauri asset layout, Markdown docs

---

### Task 1: Document the approved design

**Files:**
- Create: `docs/plans/2026-03-11-logo-refresh-design.md`

**Step 1: Write the design doc**

Capture scope, non-goals, approach, asset mapping, and verification strategy.

**Step 2: Verify the file exists**

Run: `Get-Item docs/plans/2026-03-11-logo-refresh-design.md`
Expected: file metadata is returned

### Task 2: Inventory source assets and target logo paths

**Files:**
- Inspect: `temp/WorkClaw桌面图标/WorkClaw桌面图标/*`
- Inspect: `apps/runtime/src-tauri/icons/**`
- Inspect: `apps/runtime/src/assets/branding/workclaw-logo.png`
- Inspect: `docs/workclaw_logo_w.png`

**Step 1: List the designer-provided source files**

Run: `Get-ChildItem -LiteralPath 'temp/WorkClaw桌面图标/WorkClaw桌面图标'`
Expected: SVG, transparent PNG, source PNG, and multiple ICO sizes are present

**Step 2: List the currently tracked icon targets**

Run: `Get-ChildItem -Recurse -LiteralPath 'apps/runtime/src-tauri/icons'`
Expected: desktop icon derivatives and platform-specific icon directories are present

**Step 3: Find code and docs references**

Run: `rg -n --hidden -S "workclaw-logo|workclaw_logo_w|icon.ico|icon.icns|128x128.png" README.md README.en.md docs apps/runtime`
Expected: references point to existing stable paths

### Task 3: Generate and overwrite the bundle icon assets

**Files:**
- Modify: `apps/runtime/src-tauri/icons/icon.ico`
- Modify: `apps/runtime/src-tauri/icons/icon.png`
- Modify: `apps/runtime/src-tauri/icons/32x32.png`
- Modify: `apps/runtime/src-tauri/icons/64x64.png`
- Modify: `apps/runtime/src-tauri/icons/128x128.png`
- Modify: `apps/runtime/src-tauri/icons/128x128@2x.png`
- Modify: `apps/runtime/src-tauri/icons/Square30x30Logo.png`
- Modify: `apps/runtime/src-tauri/icons/Square44x44Logo.png`
- Modify: `apps/runtime/src-tauri/icons/Square71x71Logo.png`
- Modify: `apps/runtime/src-tauri/icons/Square89x89Logo.png`
- Modify: `apps/runtime/src-tauri/icons/Square107x107Logo.png`
- Modify: `apps/runtime/src-tauri/icons/Square142x142Logo.png`
- Modify: `apps/runtime/src-tauri/icons/Square150x150Logo.png`
- Modify: `apps/runtime/src-tauri/icons/Square284x284Logo.png`
- Modify: `apps/runtime/src-tauri/icons/Square310x310Logo.png`
- Modify: `apps/runtime/src-tauri/icons/StoreLogo.png`
- Modify: `apps/runtime/src-tauri/icons/icon.icns`
- Modify: `apps/runtime/src-tauri/icons/ios/*`
- Modify: `apps/runtime/src-tauri/icons/android/mipmap-*/ic_launcher*.png`

**Step 1: Detect available image tooling**

Run: `magick -version`
Expected: ImageMagick version information, or use a fallback tool if unavailable

**Step 2: Generate PNG derivatives from the transparent source**

Use the transparent PNG or SVG as the source and overwrite the required target sizes in `apps/runtime/src-tauri/icons`.

**Step 3: Overwrite the Windows ICO**

Copy or regenerate `apps/runtime/src-tauri/icons/icon.ico` from the designer-provided ICO source.

**Step 4: Generate the macOS ICNS**

Create `apps/runtime/src-tauri/icons/icon.icns` from the generated PNG set if an icon conversion tool is available; otherwise report the limitation explicitly.

**Step 5: Regenerate iOS and Android launch icons**

Write the required icon files at the existing target paths so mobile bundle assets remain visually consistent.

### Task 4: Replace in-app and README branding images

**Files:**
- Modify: `apps/runtime/src/assets/branding/workclaw-logo.png`
- Modify: `docs/workclaw_logo_w.png`

**Step 1: Overwrite the in-app branding image**

Use the transparent source asset so the sidebar logo matches the new design.

**Step 2: Overwrite the README logo image**

Generate a README-safe logo image at the existing path without changing Markdown references.

### Task 5: Verify asset coverage

**Files:**
- Inspect: `apps/runtime/src-tauri/tauri.conf.json`
- Inspect: `README.md`
- Inspect: `README.en.md`

**Step 1: Re-run reference search**

Run: `rg -n --hidden -S "workclaw-logo|workclaw_logo_w|icon.ico|icon.icns|128x128.png" README.md README.en.md docs apps/runtime`
Expected: references still point to unchanged paths

**Step 2: Check critical files exist with fresh timestamps or sizes**

Run: `Get-Item apps/runtime/src-tauri/icons/icon.ico, apps/runtime/src-tauri/icons/icon.icns, apps/runtime/src/assets/branding/workclaw-logo.png, docs/workclaw_logo_w.png`
Expected: all files exist

**Step 3: Inspect git diff summary**

Run: `git status --short`
Expected: modified logo asset files appear alongside any pre-existing unrelated worktree changes
