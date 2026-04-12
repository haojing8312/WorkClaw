# OpenClaw-Style Repo Checks Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add OpenClaw-style narrow repo checks for duplicate code, file-size governance, and import-cycle detection, then wire them into WorkClaw's repo hygiene entrypoint and CI lane.

**Architecture:** Keep `pnpm review:repo-hygiene` as the human-facing umbrella command, but extend it with focused collectors that each own one narrow signal surface. Follow OpenClaw's pattern of "small named checks + report artifacts + non-blocking CI visibility" instead of one monolithic linter.

**Tech Stack:** Node.js scripts, `jscpd`, `madge`, existing repo hygiene report writer, GitHub Actions

---

### Task 1: Extend repo hygiene modes and command surface

**Files:**
- Modify: `package.json`
- Modify: `scripts/review-repo-hygiene.mjs`
- Test: `scripts/review-repo-hygiene.test.mjs`

- [ ] Add `review:repo-hygiene:dup`, `review:repo-hygiene:loc`, and `review:repo-hygiene:cycles` scripts in `package.json`.
- [ ] Extend `SUPPORTED_MODES` in `scripts/review-repo-hygiene.mjs` to include `dup`, `loc`, and `cycles`.
- [ ] Add new collector slots to `runRepoHygieneReview()` so mode routing stays deterministic.
- [ ] Update route tests in `scripts/review-repo-hygiene.test.mjs` to prove each new mode only invokes its own collector.

### Task 2: Add duplicate-code collector

**Files:**
- Create: `scripts/lib/repo-hygiene/collect-duplicate-signals.mjs`
- Modify: `scripts/review-repo-hygiene.mjs`
- Test: `scripts/review-repo-hygiene.test.mjs`

- [ ] Implement a `jscpd`-backed collector that scans `apps/runtime`, `packages`, and `scripts`, writes findings as `duplicate-implementation`, and degrades safely when the tool is unavailable.
- [ ] Follow the current repo hygiene collector style: injectable command runner, stable finding shape, and conservative parsing.
- [ ] Add tests for success parsing and unavailable-tool fallback.

### Task 3: Add file-size governance collector

**Files:**
- Create: `scripts/lib/repo-hygiene/collect-loc-signals.mjs`
- Modify: `scripts/review-repo-hygiene.mjs`
- Test: `scripts/review-repo-hygiene.test.mjs`

- [ ] Implement a lightweight LOC collector using repo file traversal instead of a new heavy dependency.
- [ ] Encode WorkClaw's existing governance thresholds:
- [ ] Frontend warning at `301+`, action at `501+`
- [ ] Rust warning at `501+`, action at `801+`
- [ ] Emit findings as `oversized-file` with enough detail to show actual line count and threshold crossed.
- [ ] Add fixture-based tests for frontend, Rust, and ignored file paths.

### Task 4: Add import-cycle collector

**Files:**
- Create: `scripts/lib/repo-hygiene/collect-import-cycle-signals.mjs`
- Modify: `scripts/review-repo-hygiene.mjs`
- Test: `scripts/review-repo-hygiene.test.mjs`

- [ ] Implement a `madge`-backed collector targeting `apps/runtime/src` and `apps/runtime/sidecar/src`.
- [ ] Parse cycle output into `import-cycle` findings with path detail and safe fallback when no cycles exist or the tool is unavailable.
- [ ] Add tests for cycle parsing and zero-cycle output.

### Task 5: Update docs and CI

**Files:**
- Modify: `docs/maintenance/repo-hygiene.md`
- Modify: `AGENTS.md`
- Modify: `.github/workflows/repo-hygiene.yml`

- [ ] Document the new command set and explain which checks are "report-first" versus cleanup candidates.
- [ ] Update AGENTS guidance so maintainers know `review:repo-hygiene` now covers deadcode, drift, artifacts, dup, loc, and cycles.
- [ ] Extend the existing GitHub Actions lane to run the umbrella command and upload the richer `.artifacts/repo-hygiene/` output without turning the lane into a hard merge gate.

### Task 6: Verify with smallest honest command set

**Files:**
- Modify: `scripts/review-repo-hygiene.test.mjs`
- Verify: `.artifacts/repo-hygiene/report.json`

- [ ] Run `node --test scripts/review-repo-hygiene.test.mjs`.
- [ ] Run `pnpm review:repo-hygiene:dup`.
- [ ] Run `pnpm review:repo-hygiene:loc`.
- [ ] Run `pnpm review:repo-hygiene:cycles`.
- [ ] Run `pnpm review:repo-hygiene`.
- [ ] If runtime-affecting package metadata changes broaden Rust coverage, also run `pnpm test:rust-fast`.
- [ ] Commit the phase as one or two focused commits so detector infrastructure stays separable from docs-only edits.
