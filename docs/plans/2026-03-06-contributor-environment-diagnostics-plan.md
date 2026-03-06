# Contributor Environment Diagnostics Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Windows-first contributor diagnostics workflow that reduces repeat setup issues and improves support triage quality.

**Architecture:** Keep the first version narrow and additive. Publish a documented support baseline, add a read-only `pnpm doctor:windows` script for common Windows-native failures, and require structured environment evidence in GitHub bug reports. Do not replace the current `pnpm app` flow in this phase.

**Tech Stack:** Markdown docs, Node.js scripts, pnpm, GitHub issue forms, Tauri/Rust contributor workflow

---

### Task 1: Add structured setup/build issue intake

**Files:**
- Create: `.github/ISSUE_TEMPLATE/bug-report.yml`
- Create: `.github/ISSUE_TEMPLATE/config.yml`
- Modify: `SUPPORT.md`

**Step 1: Write the failing test**

Define the required evidence for setup/build reports:
- OS and architecture
- Node and pnpm versions
- `rustc -vV`
- `rustup show`
- `where link`
- Visual Studio stable vs Insiders
- `pnpm doctor:windows` output
- exact failing command and error excerpt

**Step 2: Run test to verify it fails**

Run:
- `dir /b .github`
- `rg -n "doctor:windows|rustc -vV|where link|Visual Studio" SUPPORT.md`

Expected:
- `.github/ISSUE_TEMPLATE` does not exist yet
- `SUPPORT.md` does not require the specific evidence above

**Step 3: Write minimal implementation**

Create:
- a GitHub issue form dedicated to bug/setup reports
- a `config.yml` that points contributors to docs/support guidance

Update `SUPPORT.md` to:
- separate setup/build issues from feature questions
- ask reporters to run `pnpm doctor:windows` before filing Windows build issues

**Step 4: Run test to verify it passes**

Run:
- `dir /b .github\\ISSUE_TEMPLATE`
- `rg -n "doctor:windows|rustc -vV|where link|Visual Studio" SUPPORT.md .github/ISSUE_TEMPLATE/bug-report.yml`

Expected:
- issue template files exist
- all required evidence prompts are present

**Step 5: Commit**

```bash
git add .github/ISSUE_TEMPLATE/bug-report.yml .github/ISSUE_TEMPLATE/config.yml SUPPORT.md
git commit -m "docs(support): structure build issue intake"
```

### Task 2: Add a Windows-first doctor command

**Files:**
- Create: `scripts/doctor-windows.mjs`
- Create: `scripts/doctor-windows.test.mjs`
- Modify: `package.json`

**Step 1: Write the failing test**

Create a Node test that imports pure helper functions from `scripts/doctor-windows.mjs` and verifies:
- a healthy toolchain result prints no blocking findings
- a missing `link.exe` finding is classified as blocking
- an `LNK1104` / `msvcrt.lib` sample is mapped to MSVC workload or Windows SDK guidance
- an Insiders-only Visual Studio installation is flagged as best-effort rather than supported baseline

**Step 2: Run test to verify it fails**

Run:
- `node --test scripts/doctor-windows.test.mjs`

Expected:
- FAIL because the script and helpers do not exist yet

**Step 3: Write minimal implementation**

Implement `scripts/doctor-windows.mjs` with:
- read-only environment checks
- compact pass/warn/fail output
- deterministic remediation text
- an exit code of `0` for informational warnings and non-zero only for blocking failures

Add a root script:

```json
"doctor:windows": "node scripts/doctor-windows.mjs"
```

Prefer a structure where the CLI wrapper calls testable helper functions.

**Step 4: Run test to verify it passes**

Run:
- `pnpm doctor:windows`
- `node --test scripts/doctor-windows.test.mjs`

Expected:
- doctor command prints the check summary
- test file passes

**Step 5: Commit**

```bash
git add scripts/doctor-windows.mjs scripts/doctor-windows.test.mjs package.json
git commit -m "chore(dx): add windows doctor command"
```

### Task 3: Publish the contributor environment baseline and troubleshooting guide

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `CONTRIBUTING.md`
- Modify: `SUPPORT.md`
- Create: `docs/troubleshooting/windows-dev-setup.md`

**Step 1: Write the failing test**

List the documentation gaps to close:
- supported Windows contributor prerequisites are not explicit
- user install/start instructions are mixed with contributor source-build assumptions
- there is no dedicated Windows development troubleshooting page
- the new `doctor:windows` command is not discoverable

**Step 2: Run test to verify it fails**

Run:
- `rg -n "doctor:windows|Build Tools|Windows SDK|msvcrt.lib|windows-dev-setup" README.md README.en.md CONTRIBUTING.md SUPPORT.md docs/troubleshooting`

Expected:
- no contributor-facing Windows prerequisite section
- no `windows-dev-setup.md`
- no mention of `doctor:windows`

**Step 3: Write minimal implementation**

Update docs to:
- define the supported Windows contributor baseline
- clearly separate "use the release installer" from "build from source"
- add a troubleshooting page for Windows-native build issues
- include a short table for common failures and likely causes
- point to `pnpm doctor:windows` and issue filing requirements

The troubleshooting page should include:
- `LNK1104: msvcrt.lib`
- `link.exe` not found
- wrong Rust target
- port 5174 occupied
- WebView2 check note

**Step 4: Run test to verify it passes**

Run:
- `rg -n "doctor:windows|Build Tools|Windows SDK|msvcrt.lib|windows-dev-setup" README.md README.en.md CONTRIBUTING.md SUPPORT.md docs/troubleshooting/windows-dev-setup.md`

Expected:
- all docs contain the expected references
- the troubleshooting page exists and includes the common failure names

**Step 5: Commit**

```bash
git add README.md README.en.md CONTRIBUTING.md SUPPORT.md docs/troubleshooting/windows-dev-setup.md
git commit -m "docs(dx): add contributor environment troubleshooting"
```

### Task 4: Reconcile docs and script behavior with the real support baseline

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `CONTRIBUTING.md`
- Modify: `docs/troubleshooting/windows-dev-setup.md`
- Modify: `scripts/doctor-windows.mjs`
- Test: `scripts/doctor-windows.test.mjs`

**Step 1: Run full verification**

Run:
- `pnpm doctor:windows`
- `node --test scripts/doctor-windows.test.mjs`
- `rg -n "doctor:windows|Visual Studio|Windows SDK|Insiders|best effort" README.md README.en.md CONTRIBUTING.md SUPPORT.md docs/troubleshooting/windows-dev-setup.md`

Expected:
- doctor script output matches the documented support policy
- docs consistently state stable Visual Studio Build Tools as the supported baseline

**Step 2: Fix any mismatch**

If the script, docs, or issue form disagree on terminology, normalize them now. The same failure class should not have different wording in different places.

**Step 3: Commit**

```bash
git add README.md README.en.md CONTRIBUTING.md SUPPORT.md docs/troubleshooting/windows-dev-setup.md scripts/doctor-windows.mjs scripts/doctor-windows.test.mjs .github/ISSUE_TEMPLATE/bug-report.yml .github/ISSUE_TEMPLATE/config.yml package.json
git commit -m "chore(dx): align contributor diagnostics workflow"
```

Plan complete and saved to `docs/plans/2026-03-06-contributor-environment-diagnostics-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
