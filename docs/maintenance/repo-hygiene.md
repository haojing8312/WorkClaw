# Repo Hygiene

## Why This Exists

WorkClaw uses long-running AI-assisted development across runtime, sidecar, Rust, skills, and docs. Repo hygiene review exists to stop temporary artifacts, dead code, duplicated implementations, and stale references from silently becoming part of the long-term source of truth.

## Finding Categories

- temporary-artifacts
- orphan-files
- dead-code
- duplicate-implementations
- stale-docs
- stale-skill-or-command-references
- generated-or-runtime-owned-candidates

## Confidence Levels

- confirmed: multiple signals or direct evidence show the candidate is safe to remove or replace.
- likely: the candidate looks removable, but one more check is needed before deletion.
- uncertain: the candidate needs owner knowledge, runtime evidence, or compatibility review.
- high-risk: the candidate is generated, runtime-owned, dynamically discovered, or config-driven unless a rule explicitly marks it safe.

## Allowed Actions

- Report findings without deleting anything when the task is only review or triage.
- Group candidates into small cleanup batches after repo hygiene review.
- Delete only confirmed candidates in a reviewed cleanup batch.
- Prefer deprecation, relocation, or compatibility fallbacks when removal could affect runtime behavior or older worktrees.
- Keep suspicious files and code in place until they are classified.

## Default Workflow

1. Run `pnpm review:repo-hygiene`.
2. Review the report and triage candidates by finding category and confidence.
3. Use `workclaw-repo-hygiene-review` to classify findings before any destructive edit.
4. Choose the smallest cleanup batch that is still well supported.
5. Use `workclaw-cleanup-execution` only for the reviewed batch.
6. Run `workclaw-change-verification` when the cleanup changes code, tests, docs, or skill files.

Reports are written to `.artifacts/repo-hygiene/` for local review and should stay untracked.
The GitHub Actions `Repo Hygiene` workflow also runs this command in a non-blocking lane and uploads the generated report as a workflow artifact for review.

## High-Risk Surfaces

- Generated files and generated directories.
- Runtime-owned artifacts and state.
- Dynamically discovered files, commands, or skills.
- Config-driven outputs and paths.
- Legacy compatibility surfaces that may still be read by old worktrees or older data.
- Any candidate that appears unused in only one static signal.

## Repo-Local Skills

- `workclaw-repo-hygiene-review` - review and classify findings only.
- `workclaw-cleanup-execution` - execute only after a reviewed batch is selected.
