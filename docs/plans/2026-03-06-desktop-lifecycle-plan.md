# Desktop Lifecycle Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add GitHub Releases based auto-update, phase-1 uninstall/data-management UX, and clearer Windows release distribution guidance for the WorkClaw desktop app.

**Architecture:** Integrate Tauri v2 updater on both Rust and frontend sides, extend persisted runtime preferences for update settings, add update/data management UI to the existing settings surface, and enhance the Windows release workflow to publish updater metadata plus clearer release notes. Keep `.exe` as the self-update path and `.msi` as manual enterprise deployment.

**Tech Stack:** Tauri 2, Rust, React 18, TypeScript, Vitest, GitHub Actions, tauri-action

---

### Task 1: Add updater dependencies and Tauri bootstrap

**Files:**
- Modify: `apps/runtime/src-tauri/Cargo.toml`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Modify: `apps/runtime/package.json`
- Modify: `apps/runtime/src-tauri/tauri.conf.json`

**Step 1: Write the failing config/bootstrap test**

Create a Node test that reads `tauri.conf.json` and asserts updater configuration keys exist, and a Rust-focused smoke check if there is already a pattern for config/bootstrap tests.

```js
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("tauri config enables updater", () => {
  const raw = fs.readFileSync("apps/runtime/src-tauri/tauri.conf.json", "utf8");
  const config = JSON.parse(raw);
  assert.ok(config.plugins?.updater || config.bundle?.createUpdaterArtifacts);
});
```

**Step 2: Run test to verify it fails**

Run: `node --test scripts/check-updater-config.test.mjs`  
Expected: FAIL because updater config/test file does not exist yet

**Step 3: Write minimal implementation**

- Add `tauri-plugin-updater` Rust dependency
- Add `@tauri-apps/plugin-updater` frontend dependency
- Register updater plugin in `lib.rs`
- Add updater config/public-key placeholder structure in `tauri.conf.json`

**Step 4: Run verification**

Run: `cargo check --manifest-path apps/runtime/src-tauri/Cargo.toml`  
Expected: PASS

Run: `node --test scripts/check-updater-config.test.mjs`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/Cargo.toml apps/runtime/src-tauri/src/lib.rs apps/runtime/package.json apps/runtime/src-tauri/tauri.conf.json scripts/check-updater-config.test.mjs
git commit -m "feat(runtime): add updater bootstrap"
```

### Task 2: Extend runtime preferences for update controls

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/runtime_preferences.rs`
- Modify: `apps/runtime/src/types.ts`
- Test: `apps/runtime/src/components/__tests__/SettingsView.translation-preferences.test.tsx`

**Step 1: Write the failing preference test**

Add assertions that runtime preferences payloads can load/save:
- `auto_update_enabled`
- `update_channel`
- `dismissed_update_version`
- `last_update_check_at`

```ts
expect(result.auto_update_enabled).toBe(true);
expect(result.update_channel).toBe("stable");
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- SettingsView.translation-preferences.test.tsx`  
Expected: FAIL because the new fields are missing

**Step 3: Write minimal implementation**

- Add new keys/constants in `runtime_preferences.rs`
- Extend `RuntimePreferences` and `RuntimePreferencesInput`
- Persist/load normalized values
- Extend `RuntimePreferences` interface in `types.ts`

**Step 4: Run verification**

Run: `pnpm --dir apps/runtime test -- SettingsView.translation-preferences.test.tsx`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/runtime_preferences.rs apps/runtime/src/types.ts apps/runtime/src/components/__tests__/SettingsView.translation-preferences.test.tsx
git commit -m "feat(runtime): persist updater preferences"
```

### Task 3: Build frontend updater service and state machine

**Files:**
- Create: `apps/runtime/src/lib/updater.ts`
- Create: `apps/runtime/src/hooks/useAppUpdater.ts`
- Create: `apps/runtime/src/hooks/__tests__/useAppUpdater.test.ts`

**Step 1: Write the failing hook test**

Model the states:
- `idle`
- `checking`
- `up_to_date`
- `update_available`
- `downloading`
- `ready_to_install`
- `failed`

```ts
it("transitions from checking to update_available", async () => {
  // mock updater API returning a new version
  expect(state.status).toBe("update_available");
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- useAppUpdater.test.ts`  
Expected: FAIL because hook/service do not exist

**Step 3: Write minimal implementation**

- Wrap `@tauri-apps/plugin-updater` APIs in `lib/updater.ts`
- Implement a hook with the agreed state machine
- Support:
  - startup check
  - manual check
  - dismiss current version
  - download/install
  - failure reset

**Step 4: Run verification**

Run: `pnpm --dir apps/runtime test -- useAppUpdater.test.ts`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/lib/updater.ts apps/runtime/src/hooks/useAppUpdater.ts apps/runtime/src/hooks/__tests__/useAppUpdater.test.ts
git commit -m "feat(runtime): add updater state machine"
```

### Task 4: Add update UI to settings without disrupting existing user work

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Create: `apps/runtime/src/components/__tests__/SettingsView.updater.test.tsx`

**Step 1: Write the failing UI test**

Cover:
- manual check button
- auto update toggle
- update available card
- downloading/installing status
- failure message

```ts
expect(screen.getByRole("button", { name: "检查更新" })).toBeInTheDocument();
expect(screen.getByText("自动检查更新")).toBeInTheDocument();
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- SettingsView.updater.test.tsx`  
Expected: FAIL because the updater UI does not exist

**Step 3: Write minimal implementation**

- Add `软件更新` section to `SettingsView.tsx`
- Read/write updater-related runtime preferences
- Show version state card and actions
- Keep UI localized in Simplified Chinese

**Step 4: Run verification**

Run: `pnpm --dir apps/runtime test -- SettingsView.updater.test.tsx`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/__tests__/SettingsView.updater.test.tsx
git commit -m "feat(runtime): add settings updater UI"
```

### Task 5: Add data and uninstall guidance UI

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Create: `apps/runtime/src/components/__tests__/SettingsView.data-retention.test.tsx`
- Modify or Create: backend command file to expose resolved app data/cache/log paths if needed

**Step 1: Write the failing UI test**

Cover:
- app data path
- workspace path
- cache/log path
- `打开目录`
- `清理缓存与日志`

```ts
expect(screen.getByText("数据与卸载")).toBeInTheDocument();
expect(screen.getByRole("button", { name: "清理缓存与日志" })).toBeInTheDocument();
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- SettingsView.data-retention.test.tsx`  
Expected: FAIL because the section does not exist

**Step 3: Write minimal implementation**

- Add a data-management section in settings
- Expose local paths from Rust if current frontend cannot resolve them safely
- Add a cache/log cleanup command if not already available
- Add clear uninstall guidance text

**Step 4: Run verification**

Run: `pnpm --dir apps/runtime test -- SettingsView.data-retention.test.tsx`  
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/__tests__/SettingsView.data-retention.test.tsx apps/runtime/src-tauri/src/commands/*.rs
git commit -m "feat(runtime): add uninstall data guidance"
```

### Task 6: Update release workflow to publish updater artifacts

**Files:**
- Modify: `.github/workflows/release-windows.yml`
- Create: `scripts/check-updater-config.test.mjs` if not already created in Task 1
- Create: `docs/maintainers/desktop-updates.md`

**Step 1: Write the failing workflow/config test**

Assert workflow contains updater-friendly release configuration and references release notes/template assets.

```js
assert.match(workflow, /tauri-action@v1|tauri-action@v0/);
assert.match(workflow, /updater/i);
```

**Step 2: Run test to verify it fails**

Run: `node --test scripts/check-updater-config.test.mjs`  
Expected: FAIL because workflow does not yet publish updater metadata

**Step 3: Write minimal implementation**

- Update workflow to generate/publish updater metadata and signatures
- Prefer NSIS artifacts for updater consumption
- Move release body to a reusable template or generated file
- Document required GitHub Secrets and key generation

**Step 4: Run verification**

Run: `node --test scripts/check-updater-config.test.mjs`  
Expected: PASS

**Step 5: Commit**

```bash
git add .github/workflows/release-windows.yml scripts/check-updater-config.test.mjs docs/maintainers/desktop-updates.md
git commit -m "ci(release): publish updater artifacts"
```

### Task 7: Improve release notes and public download guidance

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`
- Create or Modify: release notes template file under `.github/` or `scripts/`

**Step 1: Write the failing doc/content test**

Add a lightweight Node test that asserts public docs mention:
- `.exe` recommended download
- `.msi` enterprise deployment
- auto-update support path

```js
assert.match(readme, /\.exe.*推荐/);
assert.match(readme, /\.msi.*企业/);
```

**Step 2: Run test to verify it fails**

Run: `node --test scripts/check-release-docs.test.mjs`  
Expected: FAIL because docs/template do not yet include this guidance

**Step 3: Write minimal implementation**

- Update Chinese/English README release sections
- Add release notes template with asset guidance

**Step 4: Run verification**

Run: `node --test scripts/check-release-docs.test.mjs`  
Expected: PASS

**Step 5: Commit**

```bash
git add README.md README.en.md scripts/check-release-docs.test.mjs .github/
git commit -m "docs(release): clarify installer choices"
```

### Task 8: End-to-end verification and release rehearsal

**Files:**
- No new source files required unless fixes emerge from validation

**Step 1: Run targeted frontend tests**

Run:

```bash
pnpm --dir apps/runtime test -- useAppUpdater.test.ts SettingsView.updater.test.tsx SettingsView.data-retention.test.tsx SettingsView.translation-preferences.test.tsx
```

Expected: PASS

**Step 2: Run Rust/build verification**

Run:

```bash
cargo check --manifest-path apps/runtime/src-tauri/Cargo.toml
pnpm build:runtime
```

Expected: PASS

**Step 3: Rehearse release validation**

Run:

```bash
pnpm test:release
pnpm test:installer
node --test scripts/check-updater-config.test.mjs
node --test scripts/check-release-docs.test.mjs
```

Expected: PASS

**Step 4: Manual verification checklist**

- install a current NSIS build
- confirm update UI appears in settings
- confirm background update check does not block app startup
- confirm update-available state renders correctly against mocked/newer metadata
- verify MSI installs show manual upgrade guidance
- verify uninstall guidance text matches actual paths on disk

**Step 5: Commit final fixes if needed**

```bash
git add -A
git commit -m "test(runtime): verify desktop lifecycle flows"
```
