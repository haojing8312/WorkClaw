# Build Cache Governance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add repo-managed build cache governance that auto-prunes stale Rust incremental caches on local git actions and blocks oversized debug dependency caches before commit/push and in CI.

**Architecture:** A shared Node script will inspect `cargo-targets/workclaw/debug`, prune only `incremental` on local runs, and report/deny oversized `deps`. Local git hooks will call the shared script, `prepare` will install hooks via `core.hooksPath`, and GitHub Actions will run the same script in read-only CI mode.

**Tech Stack:** Node.js scripts, `node:test`, Git hooks, GitHub Actions

---

### Task 1: Specify package entrypoints and hook installation

**Files:**
- Modify: `package.json`
- Create: `scripts/install-git-hooks.mjs`
- Test: `scripts/check-build-cache.test.mjs`

- [ ] Add package scripts for cache checking, manual cleaning, and hook installation.
- [ ] Add an idempotent hook installer that configures local `core.hooksPath` to `.githooks` when running inside a git worktree.
- [ ] Cover the package and installer contract with `node:test`.

### Task 2: Add build cache governance logic with TDD

**Files:**
- Create: `scripts/check-build-cache.mjs`
- Test: `scripts/check-build-cache.test.mjs`

- [ ] Write failing tests for threshold defaults, local incremental pruning, CI read-only behavior, and oversized `deps` failures.
- [ ] Implement the minimal shared cache inspection/pruning logic to satisfy those tests.
- [ ] Keep the script importable for tests and executable from package scripts/hooks.

### Task 3: Wire hooks and CI

**Files:**
- Create: `.githooks/pre-commit`
- Create: `.githooks/pre-push`
- Create: `.github/workflows/build-cache-governance.yml`
- Modify: `docs/development/windows-contributor-guide.md`

- [ ] Add local hooks that invoke the shared cache governance script before commit and push.
- [ ] Add a lightweight CI workflow that runs the same script in `--ci` mode on push and pull request.
- [ ] Document the hook behavior, thresholds, and the manual remediation command.

### Task 4: Verify the new governance surface

**Files:**
- Test: `scripts/check-build-cache.test.mjs`
- Test: `.github/workflows/build-cache-governance.yml`

- [ ] Run targeted `node:test` coverage for the new script and package contract.
- [ ] Run the new cache check script once against the current workspace to verify output shape.
- [ ] Report which verification commands covered local scripts, hooks, and CI wiring.
