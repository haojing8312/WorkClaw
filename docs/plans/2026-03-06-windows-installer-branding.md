# Windows Installer Branding Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Brand the Windows installers with WorkClaw visuals and localize the installer UI to Simplified Chinese.

**Architecture:** Keep the implementation inside Tauri bundler configuration so release automation remains unchanged. Add a small config test to lock the expected NSIS and WiX settings, then generate static bitmap assets consumed by the installer config.

**Tech Stack:** Tauri v2 bundler config, Node test runner, Windows installer assets.

---

### Task 1: Lock installer config requirements with a failing test

**Files:**
- Create: `scripts/check-installer-branding.test.mjs`
- Modify: `package.json`
- Test: `scripts/check-installer-branding.test.mjs`

**Step 1: Write the failing test**

Create a Node test that reads `apps/runtime/src-tauri/tauri.conf.json` and asserts:
- `bundle.windows.nsis.installerIcon` exists
- `bundle.windows.nsis.languages` contains only `SimpChinese`
- `bundle.windows.nsis.displayLanguageSelector` is `false`
- `bundle.windows.nsis.headerImage` and `sidebarImage` exist
- `bundle.windows.wix.language` is `zh-CN`
- `bundle.windows.wix.bannerPath` and `dialogImagePath` exist

**Step 2: Run test to verify it fails**

Run: `node --test scripts/check-installer-branding.test.mjs`

Expected: FAIL because the current Tauri config does not define these installer settings.

**Step 3: Add a package script**

Add a root script for the new test:

```json
"test:installer": "node --test scripts/check-installer-branding.test.mjs"
```

**Step 4: Run the test again**

Run: `pnpm test:installer`

Expected: FAIL for the same missing config reasons.

### Task 2: Add branded installer assets and localized bundler config

**Files:**
- Create: `apps/runtime/src-tauri/icons/installer/nsis-header.bmp`
- Create: `apps/runtime/src-tauri/icons/installer/nsis-sidebar.bmp`
- Create: `apps/runtime/src-tauri/icons/installer/wix-banner.bmp`
- Create: `apps/runtime/src-tauri/icons/installer/wix-dialog.bmp`
- Modify: `apps/runtime/src-tauri/tauri.conf.json`

**Step 1: Generate the bitmap assets**

Create BMP assets sized to the Tauri recommendations:
- NSIS header: `150x57`
- NSIS sidebar: `164x314`
- WiX banner: `493x58`
- WiX dialog: `493x312`

Use the WorkClaw logo as the source brand element and keep the assets static in the repo.

**Step 2: Update the Tauri bundler config**

Add:
- `bundle.windows.nsis.installerIcon`
- `bundle.windows.nsis.languages`
- `bundle.windows.nsis.displayLanguageSelector`
- `bundle.windows.nsis.headerImage`
- `bundle.windows.nsis.sidebarImage`
- `bundle.windows.wix.language`
- `bundle.windows.wix.bannerPath`
- `bundle.windows.wix.dialogImagePath`

**Step 3: Run the installer config test**

Run: `pnpm test:installer`

Expected: PASS.

### Task 3: Verify local Windows build still works

**Files:**
- Modify: none
- Test: build pipeline only

**Step 1: Run release-related verification**

Run:
- `pnpm release:check-version v0.2.2`
- `corepack pnpm@9 --dir apps/runtime build:tauri`

Expected:
- version check passes
- Tauri build completes with the updated installer config and assets

**Step 2: Commit**

Stage only the installer branding files and config changes, then commit with a release-focused message.
