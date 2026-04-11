# WorkClaw Repo Hygiene Governance Design

**Date:** 2026-04-11
**Status:** Draft for review
**Owner:** Codex + maintainer

## Summary

WorkClaw needs a repeatable way to control repository entropy caused by long-running AI-assisted development. The problem is broader than "delete orphan files": it includes dead code, duplicated implementations, temporary artifacts, stale docs and skill references, and misplaced files that distort future agent behavior.

The recommended design is a governance loop with five layers:

1. repo rules in `AGENTS.md` and dedicated hygiene docs
2. workflow skills for review and cleanup execution
3. deterministic scanners for dead code and drift detection
4. recurring review cadence
5. verification gates through existing WorkClaw verification skills

This design intentionally separates "identify suspicious items" from "delete or rewrite them" so the project can gain signal without introducing risky autonomous cleanup.

## Problem Statement

As WorkClaw grows under AI-assisted development, the repository can accumulate:

- temporary scripts and experimental files that were useful once but no longer belong in the tree
- dead code and unused exports that continue to consume attention and context
- parallel implementations created when an agent chose the wrong insertion point
- stale docs, skill references, and command examples that no longer match the real repo
- generated or runtime-owned files that are mistaken for hand-maintained code

These issues reduce trust in the codebase and make later AI work worse. Once the tree becomes noisy, agents are more likely to:

- read the wrong file as the source of truth
- add new changes in the wrong directory
- preserve outdated logic because it still "looks active"
- hesitate to remove suspicious code because deletion is high risk

The result is a repo that gets slower to understand and harder to maintain.

## Goals

- Reduce the number of new orphan files, dead code paths, and stale references entering the main development flow.
- Make repository hygiene review a normal maintenance workflow rather than an occasional cleanup sprint.
- Provide agent-friendly skills that can classify and prepare cleanup work safely.
- Reuse existing WorkClaw verification and release-readiness workflows instead of creating a parallel process.
- Start with non-blocking signal collection, then tighten the process only after false positives are understood.

## Non-Goals

- Fully autonomous deletion of all suspicious files.
- Early CI hard-fail behavior for subjective hygiene findings.
- Repo-wide refactoring for style consistency under the banner of cleanup.
- Replacing feature review, release review, or correctness testing with hygiene review.

## Current WorkClaw Constraints

WorkClaw is not a simple static TypeScript app. Repo hygiene logic must account for:

- desktop runtime surfaces across React, Tauri, sidecar, and shared packages
- dynamic loading and generated or discovered assets
- built-in skills and prompt assets
- docs, release flows, and examples that can drift independently of runtime code
- SQLite, runtime configuration, and startup-sensitive surfaces where false cleanup is dangerous

That means "unused" is not enough by itself. Hygiene findings need risk classes and human-review thresholds.

## Design Principles

### 1. Separate detection from execution

The review workflow should classify suspicious items first. Actual deletion or deprecation work should only happen in a separate execution workflow after confirmation.

### 2. Prefer deterministic evidence before model judgment

When a scanner can prove unused dependencies, invalid file references, or missing targets, prefer that evidence. Use model reasoning for categorization, batching, and human-readable review summaries.

### 3. Preserve WorkClaw's existing workflow model

Repo hygiene should extend existing WorkClaw skills instead of replacing them. Cleanup changes still need the normal `workclaw-change-verification` path. Release-sensitive cleanup still needs `workclaw-release-readiness`.

### 4. Default to small, reversible batches

Cleanup should land as small scoped changes. Avoid giant "clean everything" passes that mix deletes, refactors, docs, and release-sensitive changes.

### 5. Treat dynamic or generated surfaces carefully

Any path that is generated, runtime-owned, dynamically resolved, or intentionally unreferenced from static import graphs should be explicitly excluded or downgraded in risk.

## Governance Model

The recommended governance loop has five layers.

### Layer 1: Repo Rules

Add a repo hygiene section to root governance docs and a dedicated repo hygiene reference doc.

These rules should define:

- what counts as a temporary artifact
- what counts as a cleanup candidate
- which directories contain generated or runtime-owned files
- when to mark code or docs as deprecated instead of deleting them
- expectations for new files: clear owner, clear purpose, correct directory, and discoverable linkage

This layer exists to reduce new entropy from future AI edits.

### Layer 2: Workflow Skills

Add two repo-local skills.

#### `workclaw-repo-hygiene-review`

Purpose:

- inspect hygiene signals
- classify suspicious files and code
- produce a structured review report
- recommend cleanup batches
- avoid direct destructive edits

Expected output categories:

- `safe-to-delete`
- `likely-dead-needs-confirmation`
- `duplicate-or-misplaced-needs-review`
- `stale-doc-or-skill-reference`
- `generated-or-runtime-owned-ignore`

#### `workclaw-cleanup-execution`

Purpose:

- apply a pre-approved cleanup batch
- keep edits small and scoped
- run the required verification commands
- report residual uncertainty

This skill should not discover broad new scope on its own. It should execute from a reviewed candidate set.

### Layer 3: Deterministic Scanners

Create a repo hygiene command family that aggregates deterministic checks.

Recommended groups:

#### A. Dead code and unused dependency scanning

For JS and TS surfaces:

- unused dependencies
- unused exports
- suspicious unused files

Candidate tooling:

- `knip`
- `ts-prune` or equivalent

#### B. Repository structure anomaly scanning

Custom checks should identify:

- suspicious root-level files
- temp or debug filenames such as `tmp`, `debug`, `copy`, `bak`, `draft`, `old`
- abandoned analysis artifacts committed in source trees
- files placed outside expected module directories

#### C. Docs and skill drift scanning

Custom checks should identify:

- docs command examples that no longer exist in `package.json`
- skill references pointing to missing files
- stale path references in repo docs
- built-in skill assets that no longer match actual workflow entrypoints

#### D. Unified report output

All scanners should emit machine-readable and human-readable output under a single artifact area, for example:

- `.artifacts/repo-hygiene/*.json`
- `.artifacts/repo-hygiene/*.md`

### Layer 4: Review Cadence

Make hygiene review recurrent.

Recommended cadence:

- after large feature work: scoped hygiene review for touched areas
- weekly: lightweight report generation for the repo
- monthly: dedicated cleanup batch
- before release-sensitive landings: stale docs and stale skill review when relevant

### Layer 5: Verification and Release Gates

Hygiene review itself should not imply completion. Cleanup changes still follow existing WorkClaw workflows.

- use `workclaw-change-verification` for any cleanup that changes tracked code, docs, tests, or skills
- use `workclaw-release-readiness` when cleanup touches packaging, versioning, release docs, installer branding, or vendor lanes

## Finding Classification Model

To avoid unsafe deletion, all findings should be classified by confidence and action type.

### Confidence Classes

- `confirmed`
  deterministic evidence strongly indicates a removable item
- `probable`
  likely unused or stale, but dynamic behavior or weak linkage needs confirmation
- `uncertain`
  suspicious but not safe to act on without human judgment

### Action Classes

- `delete`
- `deprecate`
- `relocate`
- `merge-duplicate`
- `ignore-with-rationale`

### Combined Outcome Examples

- `confirmed + delete`
- `probable + deprecate`
- `uncertain + ignore-with-rationale`

This allows the review skill to be useful without pretending every result is equally reliable.

## Proposed WorkClaw Additions

### New Docs

- `docs/maintenance/repo-hygiene.md`
  canonical maintenance guide for hygiene review and cleanup

### New Skills

- `.agents/skills/workclaw-repo-hygiene-review/SKILL.md`
- `.agents/skills/workclaw-cleanup-execution/SKILL.md`

### New Commands

Suggested command family:

- `pnpm review:repo-hygiene`
- `pnpm review:repo-hygiene:deadcode`
- `pnpm review:repo-hygiene:drift`
- `pnpm review:repo-hygiene:artifacts`

The top-level command should aggregate lower-level reports rather than hiding them.

### New Scripts

Likely script responsibilities:

- dead code scanner wrapper
- stale command and path reference validator
- suspicious file naming and placement checker
- report merger for artifact output

## Rollout Strategy

### Phase 1: Signal Collection

Objective:

- define the rules
- expose likely hygiene issues
- avoid breaking development flow

Deliverables:

- root hygiene guidance
- maintenance doc
- aggregate repo hygiene command
- non-blocking report artifacts
- first-pass scanner integration

CI behavior:

- non-blocking only
- report findings, do not fail the lane

### Phase 2: Workflow Skill Integration

Objective:

- let maintainers and agents run structured hygiene review and structured cleanup

Deliverables:

- `workclaw-repo-hygiene-review`
- `workclaw-cleanup-execution`
- risk classification schema
- scoped cleanup workflow using existing verification skills

### Phase 3: Selective Tightening

Objective:

- convert the lowest-noise, highest-confidence hygiene checks into stronger enforcement

Candidates for future gating:

- missing file references
- clearly invalid docs command examples
- clearly unused dependencies after exclusions stabilize
- obviously forbidden temp artifact patterns

Do not early-gate subjective findings such as duplicate implementations or semantic stale docs.

## Scope Prioritization

Start with the highest-value and lowest-ambiguity areas.

Recommended first-pass scope:

- `.agents/skills/`
- `docs/`
- `apps/runtime/`
- `packages/`

Reasons:

- skills and docs are highly visible and easy to drift
- runtime code is the highest-value behavior surface
- packages often accumulate helper exports and dependency drift

Delay broad cleanup in release-sensitive or highly dynamic areas until exclusions and confidence thresholds are stable.

## Risks and Mitigations

### Risk: false positives on dynamic or runtime-discovered files

Mitigation:

- explicit allowlists and exclusions
- confidence classes
- non-blocking rollout

### Risk: cleanup work expands into unrelated refactors

Mitigation:

- small cleanup batches
- separate review and execution skills
- scoped verification

### Risk: maintainers ignore reports because noise is too high

Mitigation:

- prioritize report quality over completeness
- classify findings
- add repo-specific exclusions early

### Risk: AI agents start deleting aggressively

Mitigation:

- review skill must not directly delete
- execution skill operates only from a reviewed candidate set
- existing verification remains mandatory

## Success Metrics

Track the following over time:

- number of weekly confirmed hygiene findings
- number of new suspicious root-level or misplaced files
- number of stale docs and missing skill references found before merge
- average cleanup batch size
- number of cleanup-caused regressions

Success is not "zero findings." Success is a stable system where findings stay understandable, bounded, and easy to resolve.

## Recommended Immediate Next Steps

1. Add the maintenance doc and root hygiene rules.
2. Add a first-pass aggregate hygiene command with non-blocking output.
3. Integrate one deterministic dead-code scanner and one custom drift checker.
4. Add the review skill.
5. Pilot the workflow on `docs/`, `.agents/skills/`, and one runtime slice.

## Open Questions Resolved For This Design

- Should cleanup be fully autonomous?
  No. Detection and execution stay separate.

- Should CI fail on all hygiene findings immediately?
  No. Start non-blocking and tighten later.

- Should WorkClaw create one giant cleanup skill?
  No. Use a review skill and a separate execution skill.

- Should hygiene review replace existing verification skills?
  No. It should feed into them.

## Decision

Proceed with a phased repo hygiene governance system for WorkClaw built from:

- repo rules
- dedicated review and execution skills
- deterministic scanners
- recurring review cadence
- existing verification and release workflows

This is the smallest safe path that matches the project's current scale and AI-heavy development style without overcommitting to brittle automation.
