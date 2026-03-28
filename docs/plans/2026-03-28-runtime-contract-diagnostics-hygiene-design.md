# Runtime Contract Diagnostics Hygiene Design

**Date:** 2026-03-28

**Goal:** Align WorkClaw's hidden runtime observability and contract-style regression coverage more closely with OpenClaw while keeping diagnostics deep enough for developers and invisible enough for ordinary users.

## Context

WorkClaw already has three important building blocks:

- hidden runtime observability snapshots and recent event buffers in the Tauri runtime
- a fixture-driven runtime contract harness with six stable cases
- a desktop diagnostics export flow that already packages raw observability JSON

That gives us the raw ingredients, but not yet the final OpenClaw-style shape.

Today the gaps are:

- contract coverage is still too narrow for kernel-grade refactors
- diagnostics export contains raw JSON, but not a concise human-readable summary
- the frontend exposes diagnostics status, but not the new observability signal in a developer-focused hidden entry point
- build output still includes stable warning noise that makes future regressions harder to spot

## OpenClaw Alignment Principles

This work follows the same high-level pattern visible in `references/openclaw`:

- keep detailed event data in an internal bounded event store rather than the normal chat surface
- expose diagnostics through operator and developer entry points, not through ordinary end-user chat flows
- treat stable regression fixtures as a contract that protects runtime behavior during refactors

Relevant references:

- `references/openclaw/apps/macos/Sources/OpenClaw/AgentEventStore.swift`
- `references/openclaw/apps/shared/OpenClawKit/Sources/OpenClawChatUI/ChatView.swift`

## Scope

This design intentionally limits the work to three tracks.

### 1. Contract Fixture Expansion

Extend the runtime contract fixture suite from six to nine cases by adding:

- `compaction_overflow`
- `failover_recovery`
- `approval_reject`

Each fixture continues to assert two layers:

- normalized session run trace output
- observability snapshot and recent-event signal

This keeps the contract suite useful even if a refactor preserves the final trace shape but accidentally drops an important runtime signal.

### 2. Hidden Diagnostics Summary

Add a separate runtime diagnostics summary command instead of bloating the existing desktop diagnostics status response.

The new backend summary should:

- reuse the existing observability snapshot and recent events as the source of truth
- add lightweight, human-readable derived hints
- remain safe for diagnostics export and default-hidden frontend display

The new summary is intended for:

- a collapsed developer-only diagnostics section inside desktop settings
- diagnostics export bundle files

It is not intended for:

- the main chat view
- ordinary session UI
- a persistent always-visible debug panel

### 3. Engineering Hygiene

Reduce the most stable warning noise without turning this work into a broad cleanup refactor.

This includes:

- shrinking obvious Rust `unused` and `dead_code` warning sources
- reducing the frontend production chunk warning by lazily loading non-primary scenes

This does not include:

- eliminating every warning in the repository
- re-architecting runtime modules solely for aesthetics
- large bundler strategy changes

## Architecture

## Backend Summary Flow

Introduce a summary builder in desktop lifecycle diagnostics services that derives a compact developer-oriented summary from:

- `RuntimeObservabilitySnapshot`
- recent `RuntimeObservedEvent` entries

The builder should produce a stable serializable payload such as:

- turn totals and active count
- admission conflict total
- top guard warning kinds
- approval request total
- child-session link total
- compaction run total
- failover errors by kind
- recent event preview
- derived hints for common operator questions

Examples of derived hints:

- recent dominant error kind is `network`
- loop guard fired in the most recent run window
- compaction has run but failure count remains elevated

This keeps the UI and export flow dependent on one shared summary shape rather than duplicating summary logic in multiple places.

## Desktop Diagnostics Export

The diagnostics export should continue to ship the raw files:

- `runtime-observability-snapshot.json`
- `runtime-recent-events.json`

In addition, it should export:

- `runtime-diagnostics-summary.json`
- `runtime-diagnostics-summary.md`

The JSON file is for tooling and future automation.
The Markdown file is for humans opening the zip and trying to understand the machine state quickly.

## Frontend Entry Point

The existing desktop settings diagnostics section remains the single user-facing entry point.

Enhance it with a new collapsed block such as:

- title: `开发者诊断摘要`
- default state: collapsed
- location: inside the existing desktop diagnostics area

The collapsed block should:

- stay out of the normal day-to-day workflow
- only fetch its data when shown, or fetch once alongside diagnostics status if the payload remains modest
- render concise summary cards and a small recent-events preview

It should not render raw trace JSON directly.

## Frontend Bundle Hygiene

Reduce the Vite chunk warning by lazy-loading scenes that are not part of the primary chat path:

- `SettingsView`
- `ExpertsView`
- `ExpertCreateView`
- `EmployeeHubScene`
- `PackagingView`

`ChatView` remains eager because it is the core flow and the highest-risk target for a first lazy-loading pass.

## Rust Warning Hygiene

Prioritize warning cleanup in three buckets:

- overly broad re-exports that pull unused types into production modules
- helpers only used by tests but compiled into non-test builds
- fields and private functions left behind by refactors but no longer wired into live code

Avoid papering over these with blanket `#[allow(...)]` unless a warning is truly intentional and documented.

## Data Contracts

Add a new desktop diagnostics summary payload with stable fields that are easy to consume in both Tauri and React.

Suggested shape:

```json
{
  "turns": {
    "active": 0,
    "completed": 12,
    "failed": 2,
    "cancelled": 1
  },
  "admissions": {
    "conflicts": 3
  },
  "guard": {
    "top_warning_kinds": [
      { "kind": "loop_detected", "count": 2 }
    ]
  },
  "failover": {
    "top_error_kinds": [
      { "kind": "network", "count": 4 }
    ]
  },
  "recent_events": [
    {
      "kind": "session_run",
      "event_type": "run_failed",
      "run_id": "run-123"
    }
  ],
  "hints": [
    "Most recent failures were network-related."
  ]
}
```

Exact naming can be adjusted during implementation, but the shape should stay compact and summary-oriented.

## Testing Strategy

### Contract Tests

Expand `test_runtime_contract.rs` to nine fixtures and keep every fixture checked through the existing support harness.

### Backend Diagnostics Tests

Add focused tests for:

- summary builder output
- export bundle inclusion of the new summary files
- derived hint behavior for representative observability snapshots

### Frontend Tests

Add or extend desktop settings tests to verify:

- developer diagnostics summary is hidden by default
- expanding the section renders the returned summary
- diagnostics export actions continue to work

### Build and Verification

Expected verification set after implementation:

- `cargo test --lib --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_runtime_contract -- --nocapture`
- `pnpm test:rust-fast`
- `pnpm --dir apps/runtime exec vitest run src/components/__tests__/SettingsView.data-retention.test.tsx --pool forks --poolOptions.forks.singleFork`
- `pnpm --dir apps/runtime build`
- `pnpm build:runtime`

## Non-Goals

- building a normal-user trace screen
- adding a dedicated diagnostics window in this round
- redesigning the chat UX around observability
- fully zero-warninging the entire repository

## Rollout Notes

This design is safe to ship incrementally because:

- the new diagnostics summary is additive
- raw diagnostics export files remain unchanged
- contract fixtures only strengthen regression coverage
- lazy-loading targets non-primary scenes first

That gives us better hidden diagnostics and stronger runtime contracts without changing the everyday user-facing chat flow.
