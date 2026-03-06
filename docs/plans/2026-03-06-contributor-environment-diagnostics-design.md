# Contributor Environment Diagnostics Design

**Date:** 2026-03-06

**Scope:** Open-source contributor onboarding for local development prerequisites, self-service diagnostics, and issue intake quality.

**Out of scope:**
- Changing the Tauri/Rust application architecture
- Replacing the existing `pnpm app` workflow
- Adding cross-platform doctor coverage beyond a Windows-first first pass

---

## Goal

Reduce repeated contributor setup failures by making three things explicit:
- what local environment is officially supported
- how contributors can self-diagnose common Windows build failures in minutes
- what evidence maintainers need when a setup problem still requires help

The immediate target is the class of failures where `cargo` reports a crate compile error, but the real root cause is missing or broken Windows MSVC toolchain prerequisites such as `msvcrt.lib`.

---

## Current State

### Already present

- [README.md](/e:/code/yzpd/skillhub/README.md) documents `pnpm install`, `pnpm app`, and Tauri local startup steps.
- [CONTRIBUTING.md](/e:/code/yzpd/skillhub/CONTRIBUTING.md) gives a very short development setup section.
- [SUPPORT.md](/e:/code/yzpd/skillhub/SUPPORT.md) asks reporters to include environment details.
- [release-windows.yml](/e:/code/yzpd/skillhub/.github/workflows/release-windows.yml) provides a Windows CI baseline.
- [docs/troubleshooting/skill-installation.md](/e:/code/yzpd/skillhub/docs/troubleshooting/skill-installation.md) shows a troubleshooting pattern already exists in the docs tree.

### Gaps

- No contributor-facing Windows prerequisite list for Rust + Tauri + MSVC.
- No documented support baseline such as "stable Visual Studio Build Tools is supported; preview/Insiders is best effort".
- No self-service `doctor` command for contributors.
- No troubleshooting page for common local build failures such as `LNK1104: cannot open file 'msvcrt.lib'`.
- No structured GitHub issue form that requires toolchain evidence.

---

## Approaches Considered

### 1. Docs only

Add prerequisite docs and a troubleshooting page, but no script or issue template changes.

Pros:
- fastest to ship
- lowest maintenance cost

Cons:
- contributors still need to translate low-level toolchain errors themselves
- maintainers still receive low-signal issues without consistent diagnostics

### 2. Docs + doctor script + structured issue intake

Add contributor docs, a Windows-first `doctor` command, and a GitHub issue form that asks for specific outputs.

Pros:
- best balance of effort and support reduction
- turns common setup failures into a repeatable workflow
- improves both self-service and maintainer triage

Cons:
- small ongoing maintenance burden for the script
- Windows-first scope leaves macOS/Linux for later

### 3. Hard preflight before `pnpm app`

Wrap the normal app startup with mandatory environment validation before any dev run.

Pros:
- catches problems earlier than a Rust linker failure
- reduces confusing terminal noise

Cons:
- higher integration risk
- more likely to frustrate contributors when checks become too strict
- better as a second phase after diagnostics stabilize

### Recommendation

Choose **Approach 2**.

It addresses the real support cost without changing the existing developer workflow too aggressively. It also creates a clean path to Approach 3 later if the `doctor` output proves reliable.

---

## Decision Summary

### 1. Support policy

Publish an explicit contributor environment baseline.

For Windows, phase 1 should state:
- supported: Windows 10/11 x64
- supported: stable Visual Studio 2022 Build Tools with `Desktop development with C++`
- required: MSVC x64/x86 build tools and Windows SDK
- supported: Rust stable with `x86_64-pc-windows-msvc`
- best effort only: Visual Studio preview/Insiders installations

Reason:
- many "project build failed" reports are actually unsupported or partial toolchain setups
- maintainers need a documented baseline to triage against

### 2. Windows-first doctor command

Add `pnpm doctor:windows` as the first self-service diagnostic command for local contributors.

The command should check:
- Node.js and pnpm presence
- Rust toolchain presence and active target
- `link.exe` discoverability
- `LIB` environment visibility
- likely Windows SDK / MSVC install paths
- optional recognition of known linker failures such as `msvcrt.lib`

Reason:
- this project's most expensive local failures happen at the native toolchain boundary
- a scripted checklist is faster and more consistent than asking contributors to run commands manually

### 3. Structured issue intake

Add a GitHub bug report form for setup/build issues.

Required evidence should include:
- OS version
- Node / pnpm versions
- `rustc -vV`
- `rustup show`
- `where link`
- whether Visual Studio stable or Insiders is installed
- `pnpm doctor:windows` output
- exact failing command and full error excerpt

Reason:
- maintainers need structured reproduction context
- build failures are otherwise easy to misclassify as application bugs

### 4. Keep startup flow unchanged in phase 1

Do not replace `pnpm app` yet.

Reason:
- the current workflow is already documented
- start-command gating is more intrusive and should follow proven diagnostics, not precede them

---

## User Flow

### Contributor self-service flow

1. Contributor follows `README` / `CONTRIBUTING` setup instructions.
2. If local start fails on Windows, contributor opens the troubleshooting guide.
3. Contributor runs `pnpm doctor:windows`.
4. The doctor command reports pass/fail/warn findings with actionable remediation text.
5. If the issue remains unresolved, contributor opens a GitHub issue and pastes the doctor output and failing error.

### Maintainer triage flow

1. Maintainer checks whether the report matches the documented support baseline.
2. Maintainer inspects doctor output first, not the crate name shown by `cargo`.
3. If the issue is environmental, maintainer responds with a standard remediation path.
4. If the environment is healthy and the failure reproduces on the supported baseline, escalate as a real project defect.

---

## Architecture

### A. Documentation layer

Update:
- `README.md`
- `README.en.md`
- `CONTRIBUTING.md`
- `SUPPORT.md`

Add:
- `docs/troubleshooting/windows-dev-setup.md`

Responsibilities:
- define supported local environment
- point contributors to the troubleshooting guide and doctor command
- distinguish user installation from contributor source builds

### B. Diagnostics layer

Add:
- `scripts/doctor-windows.mjs`
- `package.json` script entry `doctor:windows`

Responsibilities:
- run a small, deterministic set of environment checks
- emit simple human-readable statuses
- map known failures to concrete next steps

The script should stay conservative:
- no privileged operations
- no mutation of the system
- no automatic installation attempts

### C. Intake layer

Add:
- `.github/ISSUE_TEMPLATE/bug-report.yml`
- `.github/ISSUE_TEMPLATE/config.yml`

Responsibilities:
- capture structured environment evidence
- request doctor output before maintainer investigation
- route low-signal setup questions into a consistent format

---

## Failure Categories To Cover First

Phase 1 should explicitly cover these Windows issues:

- `LINK : fatal error LNK1104: cannot open file 'msvcrt.lib'`
- `link.exe` not found
- missing Rust toolchain or wrong target
- missing `Desktop development with C++` workload
- missing Windows SDK
- `Port 5174 is already in use`
- WebView2 not available or broken

Do not attempt to cover every possible Tauri, Node, or driver issue in phase 1.

---

## Testing Strategy

### Documentation verification

- ensure all entry docs link to the new troubleshooting path
- ensure the support baseline is visible from both README and CONTRIBUTING

### Script verification

- add focused Node tests around diagnosis helpers
- verify healthy and broken sample findings produce expected recommendations
- manually run `pnpm doctor:windows` on a known-good machine

### Issue intake verification

- inspect the GitHub issue form for required fields
- ensure requested evidence matches the troubleshooting guide and doctor command

---

## Rollout Plan

### Phase 1

- contributor support baseline
- Windows troubleshooting guide
- `pnpm doctor:windows`
- bug report form and support copy updates

### Phase 2

- optional preflight wrapper before `pnpm app`
- macOS and Linux doctor commands
- reusable maintainer canned responses linked to failure classes

---

## Success Criteria

- contributors can identify the `msvcrt.lib` class of failure without maintainer intervention
- maintainers receive issue reports with enough evidence to classify environment vs project bug quickly
- README and CONTRIBUTING no longer imply that `pnpm install` + `pnpm app` is sufficient on any Windows machine
- Windows CI becomes the explicit support baseline for source-build troubleshooting
