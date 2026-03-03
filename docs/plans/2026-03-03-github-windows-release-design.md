# GitHub Windows Release Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add tag-driven GitHub Release automation that builds downloadable Windows artifacts for the Tauri app.

**Architecture:** GitHub Actions listens for semantic version tags (`v*.*.*`), validates tag/version consistency, builds the Tauri Windows bundle on `windows-latest`, and uploads artifacts to GitHub Release via the official Tauri action.

**Tech Stack:** GitHub Actions, pnpm, Node.js, Tauri 2.x

---

### Task 1: Add release version validator

**Files:**
- Create: `scripts/check-release-version.mjs`
- Test: `.github/workflows/release-windows.yml` (validator execution)

**Step 1: Write the failing test**
- Validate missing semantic tag format should fail.

**Step 2: Run test to verify it fails**
- Run: `node scripts/check-release-version.mjs`
- Expected: fail with missing `GITHUB_REF_NAME`.

**Step 3: Write minimal implementation**
- Parse `apps/runtime/src-tauri/tauri.conf.json`.
- Compare `version` with stripped tag (remove leading `v`).

**Step 4: Run test to verify it passes**
- Run: `set GITHUB_REF_NAME=v0.1.0 && node scripts/check-release-version.mjs`
- Expected: pass and print consistency message.

**Step 5: Commit**
- `git add scripts/check-release-version.mjs`
- `git commit -m "chore(release): add tag/version consistency validator"`

### Task 2: Add Windows release workflow

**Files:**
- Create: `.github/workflows/release-windows.yml`

**Step 1: Write the failing test**
- Trigger workflow syntax and dry-run assumptions on local YAML inspection.

**Step 2: Run test to verify it fails**
- Not applicable locally without runner.

**Step 3: Write minimal implementation**
- Trigger on `push.tags: v*.*.*`.
- Install dependencies with pnpm.
- Run version validator.
- Build/publish with `tauri-apps/tauri-action`.

**Step 4: Run test to verify it passes**
- Validate YAML structure and repository scripts resolve.

**Step 5: Commit**
- `git add .github/workflows/release-windows.yml`
- `git commit -m "ci(release): add windows tag-driven github release workflow"`

### Task 3: Document first release runbook

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `README.en.md`

**Step 1: Write the failing test**
- Missing release instructions for tag-driven Windows release.

**Step 2: Run test to verify it fails**
- `rg -n "Windows Release|Windows 发布|tag" README*.md`
- Expected: no dedicated section.

**Step 3: Write minimal implementation**
- Add brief section with prerequisites and first release commands.

**Step 4: Run test to verify it passes**
- Confirm section exists in all three readmes.

**Step 5: Commit**
- `git add README.md README.zh-CN.md README.en.md`
- `git commit -m "docs(release): add windows github release runbook"`
